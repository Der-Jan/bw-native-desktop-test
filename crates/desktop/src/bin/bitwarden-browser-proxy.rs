//! Browser-launched, bounded stdin/stdout ↔ local-socket relay.
//! Never starts a GUI, logs messages, or handles plaintext vault keys.
#[cfg(target_os = "linux")]
#[path = "../services/browser_integration/wire.rs"]
mod wire;
#[cfg(target_os = "linux")]
fn main() {
    if run().is_err() {
        eprintln!("Bitwarden browser connection unavailable");
        std::process::exit(1);
    }
}
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Native browser registration currently supports Linux");
}
#[cfg(target_os = "linux")]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use std::{io, os::unix::fs::MetadataExt, path::PathBuf};
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 5 || args[0] != "--socket" || args[2] != "--app" {
        return Err("Invalid native host invocation".into());
    }
    let socket = PathBuf::from(&args[1]);
    let expected_app = PathBuf::from(&args[3]).canonicalize()?;
    let origin = wire::extension_origin(&args[4..]).ok_or("Unrecognized browser extension")?;
    let browser = wire::browser_process(std::process::id()).ok_or("Unrecognized browser parent")?;
    if browser.label.is_empty() || !browser.executable.is_absolute() {
        return Err("Invalid browser identity".into());
    }
    let uid = unsafe { libc::geteuid() };
    let parent = std::fs::symlink_metadata(socket.parent().ok_or("Invalid socket")?)?;
    if !parent.is_dir() || parent.uid() != uid || parent.mode() & 0o077 != 0 {
        return Err("Unsafe socket directory".into());
    }
    let metadata = std::fs::symlink_metadata(&socket)?;
    if metadata.file_type().is_symlink() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err("Unsafe socket".into());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(async {
        let mut conn = tokio::net::UnixStream::connect(&socket).await?;
        let peer = conn.peer_cred()?;
        let pid = peer.pid().ok_or("Missing peer identity")?;
        if peer.uid() != uid
            || std::fs::read_link(format!("/proc/{pid}/exe"))?.canonicalize()? != expected_app
        {
            return Err("Unrecognized desktop process".into());
        }
        wire::write_frame(
            &mut conn,
            &serde_json::to_vec(&serde_json::json!({"origin":origin}))?,
        )
        .await?;
        let (mut read, mut write) = conn.into_split();
        let to_browser = async {
            let mut stdout = tokio::io::stdout();
            while let Some(data) = wire::read_frame(&mut read).await? {
                wire::write_frame(&mut stdout, &data).await?;
            }
            Ok::<_, io::Error>(())
        };
        let to_app = async {
            let mut stdin = tokio::io::stdin();
            while let Some(data) = wire::read_frame(&mut stdin).await? {
                wire::write_frame(&mut write, &data).await?;
            }
            Ok::<_, io::Error>(())
        };
        tokio::select! { r=to_browser => r?, r=to_app => r? }
        Ok::<_, Box<dyn std::error::Error>>(())
    });
    // Tokio's stdin uses a blocking reader that cannot be canceled. Do not
    // wait for browser stdin when the desktop disconnects; closing this helper
    // must notify the extension immediately.
    runtime.shutdown_background();
    result
}
