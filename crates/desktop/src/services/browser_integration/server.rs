use super::*;
#[cfg(target_os = "linux")]
pub(super) async fn run(shared: Arc<Shared>, path: PathBuf) {
    let mut changes = shared.changes.subscribe();
    loop {
        while !shared.state.lock().unwrap().enabled {
            if changes.changed().await.is_err() {
                return;
            }
        }
        let result = bind(&path);
        match result {
            Ok((listener, _guard)) => {
                status(&shared, "Browser connection ready");
                let limit = Arc::new(Semaphore::new(8));
                let mut peers = tokio::task::JoinSet::new();
                loop {
                    tokio::select! {
                        change=changes.changed()=>{if change.is_err() || !shared.state.lock().unwrap().enabled {break;}},
                        accept=listener.accept()=>{
                            let Ok((stream,_))=accept else {break;};
                            let Ok(slot)=limit.clone().try_acquire_owned() else {continue;};
                            let shared=shared.clone();
                            peers.spawn(async move {let _slot=slot;let _=serve(shared,stream).await;});
                        },
                        _=peers.join_next(),if !peers.is_empty()=>{}
                    }
                }
            }
            Err(_) => {
                status(
                    &shared,
                    "Browser socket unavailable; another instance or unsafe path may be present",
                );
                if changes.changed().await.is_err() {
                    return;
                }
            }
        }
    }
}
#[cfg(not(target_os = "linux"))]
pub(super) async fn run(shared: Arc<Shared>, _: PathBuf) {
    status(&shared, "Browser integration currently supports Linux");
}
fn status(shared: &Shared, message: &str) {
    shared.state.lock().unwrap().status = message.into();
    let _ = shared.events.send(Event::Changed);
}
#[cfg(target_os = "linux")]
struct SocketGuard {
    path: PathBuf,
    inode: u64,
}
#[cfg(target_os = "linux")]
impl Drop for SocketGuard {
    fn drop(&mut self) {
        use std::os::unix::fs::MetadataExt;
        if std::fs::symlink_metadata(&self.path)
            .is_ok_and(|m| m.ino() == self.inode && m.uid() == unsafe { libc::geteuid() })
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
#[cfg(target_os = "linux")]
fn bind(path: &std::path::Path) -> std::io::Result<(tokio::net::UnixListener, SocketGuard)> {
    use std::{
        io,
        os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
    };
    let uid = unsafe { libc::geteuid() };
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing socket parent"))?;
    match std::fs::symlink_metadata(parent) {
        Ok(m) if !m.is_dir() || m.uid() != uid || m.mode() & 0o077 != 0 => {
            return Err(io::Error::other("Unsafe socket directory"));
        }
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)?;
        }
        Err(e) => return Err(e),
    }
    if let Ok(m) = std::fs::symlink_metadata(path) {
        if !m.file_type().is_socket() || m.uid() != uid || m.mode() & 0o077 != 0 {
            return Err(io::Error::other("Unsafe existing socket"));
        }
        match std::os::unix::net::UnixStream::connect(path) {
            Ok(_) => return Err(io::Error::other("Browser socket already active")),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                ) => {}
            Err(e) => return Err(e),
        }
        if std::fs::symlink_metadata(path)?.ino() != m.ino() {
            return Err(io::Error::other("Socket changed"));
        }
        std::fs::remove_file(path)?;
    }
    let listener = tokio::net::UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    let guard = SocketGuard {
        path: path.into(),
        inode: std::fs::symlink_metadata(path)?.ino(),
    };
    Ok((listener, guard))
}
#[cfg(target_os = "linux")]
async fn serve(shared: Arc<Shared>, mut stream: tokio::net::UnixStream) -> Result<(), String> {
    use std::time::Duration;
    let peer = stream.peer_cred().map_err(|_| "Missing browser peer")?;
    if peer.uid() != unsafe { libc::geteuid() } {
        return Err("Wrong browser user".into());
    }
    let pid = peer.pid().ok_or("Missing browser process")? as u32;
    let actual = std::fs::read_link(format!("/proc/{pid}/exe"))
        .and_then(|p| p.canonicalize())
        .map_err(|_| "Cannot verify browser helper")?;
    let expected = std::env::current_exe()
        .map_err(|_| "Missing desktop executable")?
        .with_file_name("bitwarden-browser-proxy")
        .canonicalize()
        .map_err(|_| "Missing browser helper")?;
    if actual != expected {
        return Err("Unexpected browser helper process".into());
    }
    let browser = wire::browser_process(pid).ok_or("Unrecognized browser parent")?;
    let hello = tokio::time::timeout(Duration::from_secs(3), wire::read_frame(&mut stream))
        .await
        .map_err(|_| "Browser handshake timed out")?
        .map_err(|_| "Invalid browser handshake")?
        .ok_or("Empty browser handshake")?;
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Hello {
        origin: String,
    }
    let hello: Hello = serde_json::from_slice(&hello).map_err(|_| "Invalid browser identity")?;
    let valid = hello.origin == wire::FIREFOX_ID
        || wire::extension_origin(std::slice::from_ref(&hello.origin)).is_some();
    if !valid || ((browser.label == "Firefox") != (hello.origin == wire::FIREFOX_ID)) {
        return Err("Browser extension does not match its browser".into());
    }
    let connection = {
        let state = shared.state.lock().unwrap();
        let session = state.session.clone().ok_or("No selected desktop account")?;
        Arc::new(Connection {
            shared: shared.clone(),
            session,
            epoch: state.epoch,
            browser: browser.label,
            browser_executable: browser.executable,
            origin: hello.origin,
            alive: AtomicBool::new(true),
            remembered: AtomicBool::new(false),
        })
    };
    struct Alive(Arc<Connection>);
    impl Drop for Alive {
        fn drop(&mut self) {
            self.0.alive.store(false, Ordering::SeqCst);
        }
    }
    let _alive = Alive(connection.clone());
    let mut changes = shared.changes.subscribe();
    wire::write_frame(&mut stream, b"{\"command\":\"connected\"}")
        .await
        .map_err(|_| "Browser disconnected")?;
    let (mut read, mut write) = stream.into_split();
    let (outgoing, mut output) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
    let sdk = sdk_ipc::Peer::new(connection.clone(), outgoing.clone()).await?;
    let mut legacy = None::<Arc<legacy::Channel>>;
    let mut handlers = tokio::task::JoinSet::new();
    let mut frames = 0u32;
    let mut started = Instant::now();
    loop {
        if !connection.current() {
            break;
        }
        tokio::select! {
            _=changes.changed()=>break,
            data=output.recv()=>{
                let Some(data)=data else {break;};
                if !connection.current(){break;}
                tokio::select! {
                    _=changes.changed()=>break,
                    result=tokio::time::timeout(Duration::from_secs(5),wire::write_frame(&mut write,&data))=>{if !matches!(result,Ok(Ok(()))) {break;}}
                }
            },
            frame=tokio::time::timeout(Duration::from_secs(300),wire::read_frame(&mut read))=>{
                let Ok(Ok(Some(data)))=frame else {break;};
                if started.elapsed()>Duration::from_secs(1){started=Instant::now();frames=0;}
                frames+=1;if frames>64 || handlers.len()>=32 {break;}
                let value:serde_json::Value=serde_json::from_slice(&data).map_err(|_|"Malformed browser request")?;
                if value["type"]=="bitwarden-ipc-message" {sdk.receive(value)?;continue;}
                if value["message"]["command"]=="setupEncryption" {
                    if legacy.is_some(){break;}
                    let (channel,response)=legacy::Channel::setup(&value,&connection)?;
                    legacy=Some(channel);outgoing.send(response).await.map_err(|_|"Browser disconnected")?;
                } else {
                    let channel=legacy.clone().ok_or("Browser encryption not established")?;
                    let message=channel.request(&value)?;
                    let connection=connection.clone();let outgoing=outgoing.clone();
                    handlers.spawn(async move {if let Ok(response)=channel.respond(message,connection.clone()).await && connection.current(){let _=outgoing.send(response).await;}});
                }
            },
            _=handlers.join_next(),if !handlers.is_empty()=>{}
        }
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    #[tokio::test]
    async fn socket_is_private_and_an_active_listener_is_never_removed() {
        let root = std::env::temp_dir().join(format!("bw-browser-socket-{}", Uuid::new_v4()));
        let path = root.join("browser.sock");
        let (listener, guard) = bind(&path).unwrap();
        assert_eq!(std::fs::metadata(&root).unwrap().mode() & 0o777, 0o700);
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        let inode = std::fs::metadata(&path).unwrap().ino();
        assert!(bind(&path).is_err());
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
        drop(listener);
        drop(guard);
        assert!(!path.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn unsafe_directory_and_symlink_socket_are_rejected_without_removing_targets() {
        let root = std::env::temp_dir().join(format!("bw-browser-socket-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(bind(&root.join("browser.sock")).is_err());
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let file = root.join("keep");
        std::fs::write(&file, b"keep").unwrap();
        symlink(&file, root.join("browser.sock")).unwrap();
        assert!(bind(&root.join("browser.sock")).is_err());
        assert_eq!(std::fs::read(file).unwrap(), b"keep");
        std::fs::remove_dir_all(root).unwrap();
    }
}
