//! Bounded browser Native Messaging framing. No tracing or stdout diagnostics.
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
pub const MAX_FRAME: usize = 1024 * 1024;
pub const FIREFOX_ID: &str = "{446900e4-71c2-419f-a6a7-df9c091e268b}";
pub const CHROMIUM_IDS: &[&str] = &[
    "nngceckbapebfimnlniiiahkandclblb",
    "hccnnhgbibccigepcmlgppchkpfdophk",
    "jbkfoedolllekgbhcbcoahefnbanhhlh",
    "ccnckbpmaceehanjmeomladnmlffdjgn",
];
pub async fn read_frame(reader: &mut (impl AsyncRead + Unpin)) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0; 4];
    if reader.read(&mut header[..1]).await? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..]).await?;
    let size = u32::from_ne_bytes(header) as usize;
    if size == 0 || size > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid native message size",
        ));
    }
    let mut data = vec![0; size];
    reader.read_exact(&mut data).await?;
    Ok(Some(data))
}
pub async fn write_frame(writer: &mut (impl AsyncWrite + Unpin), data: &[u8]) -> io::Result<()> {
    if data.is_empty() || data.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid native message size",
        ));
    }
    writer.write_all(&(data.len() as u32).to_ne_bytes()).await?;
    writer.write_all(data).await?;
    writer.flush().await
}
pub fn extension_origin(args: &[String]) -> Option<String> {
    match args {
        [origin]
            if CHROMIUM_IDS.iter().any(|id| {
                origin == &format!("chrome-extension://{id}/")
                    || origin == &format!("chrome-extension://{id}")
            }) =>
        {
            Some(origin.trim_end_matches('/').into())
        }
        [manifest, id]
            if id == FIREFOX_ID
                && std::path::Path::new(manifest)
                    .file_name()
                    .is_some_and(|n| n == "com.8bit.bitwarden.json") =>
        {
            Some(FIREFOX_ID.into())
        }
        _ => None,
    }
}
#[cfg(target_os = "linux")]
pub fn browser_process(pid: u32) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let proc = std::path::PathBuf::from(format!("/proc/{pid}"));
    let stat = std::fs::read_to_string(proc.join("stat")).ok()?;
    let parent: u32 = stat
        .rsplit_once(") ")?
        .1
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    let exe = std::fs::read_link(format!("/proc/{parent}/exe"))
        .ok()?
        .canonicalize()
        .ok()?;
    let metadata = std::fs::metadata(&exe).ok()?;
    if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return None;
    }
    match exe.file_name()?.to_str()? {
        "firefox" | "firefox-bin" | "firefox-esr" => Some("Firefox".into()),
        "chrome" | "chromium" | "chromium-browser" => Some("Chrome / Chromium".into()),
        "brave" | "brave-browser" => Some("Brave".into()),
        "vivaldi-bin" => Some("Vivaldi".into()),
        "msedge" => Some("Microsoft Edge".into()),
        "helium" => Some("Helium".into()),
        "opera" => Some("Opera".into()),
        _ => None,
    }
}
