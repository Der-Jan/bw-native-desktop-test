use super::{
    Shared,
    protocol::{Bindings, MAX_PACKET},
};
use std::{
    os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    sync::{Semaphore, mpsc, watch},
    task::JoinSet,
};

pub(super) struct SocketGuard {
    path: PathBuf,
    device: u64,
    inode: u64,
}
impl Drop for SocketGuard {
    fn drop(&mut self) {
        if let Ok(meta) = self.path.symlink_metadata()
            && meta.file_type().is_socket()
            && meta.dev() == self.device
            && meta.ino() == self.inode
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
pub(super) fn bind(path: &Path) -> Result<(UnixListener, SocketGuard), &'static str> {
    if !path.is_absolute() {
        return Err("SSH-Socketpfad muss absolut sein (BITWARDEN_SSH_AUTH_SOCK)");
    }
    if path.as_os_str().as_encoded_bytes().len() > 100 {
        return Err("SSH-Socketpfad ist zu lang");
    }
    let base = path.parent().ok_or("Ungültiger SSH-Socketpfad")?;
    let uid = unsafe { libc::geteuid() };
    match base.symlink_metadata() {
        Ok(meta)
            if meta.is_dir()
                && !meta.file_type().is_symlink()
                && meta.uid() == uid
                && meta.mode() & 0o077 == 0 => {}
        Ok(_) => {
            return Err(
                "SSH-Socket benötigt ein privates Laufzeitverzeichnis (0700, eigener Benutzer)",
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(base)
                .map_err(|_| "SSH-Laufzeitverzeichnis konnte nicht erstellt werden")?;
        }
        Err(_) => return Err("SSH-Laufzeitverzeichnis ist nicht verfügbar"),
    }
    // Never replace an existing socket, file or symlink, including stale paths.
    if path.symlink_metadata().is_ok() {
        return Err("SSH-Socketpfad ist bereits belegt; vorhandener Eintrag bleibt unverändert");
    }
    let listener =
        UnixListener::bind(path).map_err(|_| "SSH-Socket konnte nicht gebunden werden")?;
    let meta = path
        .symlink_metadata()
        .map_err(|_| "SSH-Socket konnte nicht geprüft werden")?;
    let guard = SocketGuard {
        path: path.into(),
        device: meta.dev(),
        inode: meta.ino(),
    };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| "SSH-Socketrechte konnten nicht gesetzt werden")?;
    Ok((listener, guard))
}
pub(super) async fn serve(listener: UnixListener, shared: Arc<Shared>) {
    let slots = Arc::new(Semaphore::new(4));
    let mut tasks = JoinSet::new();
    loop {
        tokio::select! {
            connection=listener.accept()=>{
                let Ok((stream,_))=connection else {break};
                let Ok(permit)=slots.clone().try_acquire_owned() else {continue};
                let Ok(credentials)=stream.peer_cred() else {continue};
                if credentials.uid()!=unsafe{libc::geteuid()} {continue;}
                let pid=credentials.pid();
                let process=pid.map(|pid| {
                    let name=std::fs::read_to_string(format!("/proc/{pid}/comm")).ok().map(|n|crate::services::sdk::ssh_agent::clean_label(n.trim(),128)).unwrap_or_else(||"unbekannt".into());
                    format!("{name} (PID {pid})")
                }).unwrap_or_else(||"unbekannter lokaler Prozess".into());
                let shared=shared.clone();
                tasks.spawn(async move {let _permit=permit;connection_loop(stream,shared,process).await;});
            },
            _=tasks.join_next(),if !tasks.is_empty()=>{},
        }
    }
}
struct AbortTask(tokio::task::JoinHandle<()>);
impl Drop for AbortTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn connection_loop(stream: UnixStream, shared: Arc<Shared>, process: String) {
    let mut changed = shared.changes.subscribe();
    let epoch = *changed.borrow_and_update();
    let (mut read, mut write) = stream.into_split();
    let (frames, mut pending) = mpsc::channel(4);
    let (closed, mut disconnected) = watch::channel(false);
    let _reader = AbortTask(tokio::spawn(async move {
        for _ in 0..256 {
            // Header/body share one absolute deadline. Occasional bytes do not renew it.
            let packet = tokio::time::timeout(Duration::from_secs(60), async {
                let length = read.read_u32().await? as usize;
                if length == 0 || length > MAX_PACKET {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid SSH frame",
                    ));
                }
                let mut bytes = vec![0; length];
                read.read_exact(&mut bytes).await?;
                Ok(bytes)
            })
            .await;
            let Ok(Ok(packet)) = packet else { break };
            if frames.try_send(packet).is_err() {
                break;
            }
        }
        closed.send_replace(true);
    }));
    let mut bindings = Bindings::default();
    loop {
        let packet = tokio::select! { _=changed.changed()=>break,_=disconnected.changed()=>break,packet=pending.recv()=>packet };
        let Some(packet) = packet else { break };
        if *disconnected.borrow() || *changed.borrow() != epoch {
            break;
        }
        let reply = tokio::select! { _=changed.changed()=>break,_=disconnected.changed()=>break,reply=shared.handle(packet,&mut bindings,epoch,&process)=>reply };
        if *disconnected.borrow() || *changed.borrow() != epoch {
            break;
        }
        if reply.first() == Some(&14) && !shared.current(epoch) {
            break;
        }
        let mut frame = Vec::with_capacity(4 + reply.len());
        frame.extend_from_slice(&(reply.len() as u32).to_be_bytes());
        frame.extend_from_slice(&reply);
        if !matches!(
            tokio::time::timeout(Duration::from_secs(5), write.write_all(&frame)).await,
            Ok(Ok(()))
        ) {
            break;
        }
    }
}
