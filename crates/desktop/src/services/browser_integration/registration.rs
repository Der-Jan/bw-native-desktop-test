use super::wire;
use crate::services::accounts::storage::{atomic_write, read_bounded};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Browser {
    Helium,
    Chrome,
    Chromium,
    Firefox,
}
impl Browser {
    pub const ALL: [Self; 4] = [Self::Helium, Self::Chrome, Self::Chromium, Self::Firefox];
    pub fn label(self) -> &'static str {
        match self {
            Self::Helium => "Helium",
            Self::Chrome => "Google Chrome",
            Self::Chromium => "Chromium",
            Self::Firefox => "Firefox",
        }
    }
}
#[derive(Serialize, Deserialize)]
struct Previous {
    launcher: String,
    manifest: Option<Vec<u8>>,
}
type Registrations = BTreeMap<String, Previous>;
fn root() -> PathBuf {
    crate::paths::data_dir().join("browser-host")
}
fn target(browser: Browser) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Home directory unavailable")?;
    if browser == Browser::Firefox {
        return Ok(home.join(".mozilla/native-messaging-hosts/com.8bit.bitwarden.json"));
    }
    let config = std::env::var_os("BITWARDEN_BROWSER_CONFIG_HOME")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    Ok(config
        .join(match browser {
            Browser::Helium => "net.imput.helium",
            Browser::Chrome => "google-chrome",
            Browser::Chromium => "chromium",
            Browser::Firefox => unreachable!(),
        })
        .join("NativeMessagingHosts/com.8bit.bitwarden.json"))
}
fn load(root: &Path) -> Result<Registrations, String> {
    let path = root.join("registrations.json");
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    serde_json::from_slice(&read_bounded(&path, 8 * 1024 * 1024)?)
        .map_err(|_| "Invalid browser registration backup".into())
}
fn sh(value: &Path) -> Result<String, String> {
    let value = value.to_str().ok_or("Non-UTF8 browser host path")?;
    Ok(format!("'{}'", value.replace('\'', "'\\''")))
}
pub fn install(browser: Browser, socket: &Path) -> Result<String, String> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (browser, socket);
        return Err("Browser registration currently supports Linux".into());
    }
    let app = std::env::current_exe()
        .map_err(|_| "Application path unavailable")?
        .canonicalize()
        .map_err(|_| "Application path unavailable")?;
    let proxy = app
        .with_file_name("bitwarden-browser-proxy")
        .canonicalize()
        .map_err(|_| "The bitwarden-browser-proxy helper has not been installed")?;
    install_at(browser, socket, &app, &proxy, &root(), &target(browser)?)
}
fn install_at(
    browser: Browser,
    socket: &Path,
    app: &Path,
    proxy: &Path,
    root: &Path,
    target: &Path,
) -> Result<String, String> {
    let launcher = root.join("native-host");
    let script = format!(
        "#!/bin/sh\nexec {} --socket {} --app {} \"$@\"\n",
        sh(proxy)?,
        sh(socket)?,
        sh(app)?
    );
    atomic_write(root, "native-host", script.as_bytes())
        .map_err(|_| "Could not install browser launcher")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "Could not set browser launcher permissions")?;
    }
    let mut manifest = serde_json::json!({"name":"com.8bit.bitwarden","description":"Bitwarden native desktop unlock","path":launcher,"type":"stdio"});
    if browser == Browser::Firefox {
        manifest["allowed_extensions"] = serde_json::json!([wire::FIREFOX_ID]);
    } else {
        manifest["allowed_origins"] = serde_json::json!(
            wire::CHROMIUM_IDS
                .iter()
                .map(|id| format!("chrome-extension://{id}/"))
                .collect::<Vec<_>>()
        );
    }
    let mut records = load(root)?;
    let previous = if target.exists() {
        Some(read_bounded(target, 64 * 1024)?)
    } else {
        None
    };
    let owned = previous
        .as_ref()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(b).ok())
        .is_some_and(|m| m["path"].as_str() == launcher.to_str());
    if !owned || !records.contains_key(&target.to_string_lossy().to_string()) {
        records.insert(
            target.to_string_lossy().into_owned(),
            Previous {
                launcher: launcher.to_string_lossy().into_owned(),
                manifest: previous,
            },
        );
        atomic_write(
            root,
            "registrations.json",
            &serde_json::to_vec(&records).map_err(|_| "Could not encode browser backup")?,
        )
        .map_err(|_| "Could not save previous browser registration")?;
    }
    atomic_write(
        target.parent().ok_or("Invalid browser path")?,
        "com.8bit.bitwarden.json",
        &serde_json::to_vec_pretty(&manifest)
            .map_err(|_| "Could not encode browser registration")?,
    )
    .map_err(|_| "Could not register browser native host")?;
    Ok(format!(
        "{} registered. Any previous Bitwarden host registration was backed up.",
        browser.label()
    ))
}
pub fn restore() -> Result<String, String> {
    restore_at(&root())
}
fn restore_at(root: &Path) -> Result<String, String> {
    let root = root.to_path_buf();
    let mut records = load(&root)?;
    let mut restored = Vec::new();
    for (path, previous) in &records {
        let target = Path::new(path);
        let current = read_bounded(target, 64 * 1024)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
        if !current.is_some_and(|m| m["path"].as_str() == Some(&previous.launcher)) {
            continue;
        }
        if let Some(bytes) = &previous.manifest {
            atomic_write(
                target.parent().ok_or("Invalid browser path")?,
                "com.8bit.bitwarden.json",
                bytes,
            )
            .map_err(|_| "Could not restore previous browser host")?;
        } else {
            std::fs::remove_file(target).map_err(|_| "Could not remove browser host")?;
        }
        restored.push(path.clone());
    }
    for path in &restored {
        records.remove(path);
    }
    atomic_write(
        &root,
        "registrations.json",
        &serde_json::to_vec(&records)
            .map_err(|_| "Could not encode browser registration backup")?,
    )
    .map_err(|_| "Could not update browser registration backup")?;
    Ok(format!("Restored {} browser registrations", restored.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helium_registration_restricts_extensions_and_preserves_previous_host() {
        let tmp =
            std::env::temp_dir().join(format!("bw-browser-registration-{}", uuid::Uuid::new_v4()));
        let root = tmp.join("app");
        let target =
            tmp.join("config/net.imput.helium/NativeMessagingHosts/com.8bit.bitwarden.json");
        let original = br#"{"name":"com.8bit.bitwarden","path":"/original/bitwarden"}"#;
        atomic_write(
            target.parent().unwrap(),
            "com.8bit.bitwarden.json",
            original,
        )
        .unwrap();
        for _ in 0..2 {
            install_at(
                Browser::Helium,
                Path::new("/private/socket"),
                Path::new("/app/native"),
                Path::new("/app/proxy with ' quote"),
                &root,
                &target,
            )
            .unwrap();
        }
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
        assert_eq!(
            manifest["allowed_origins"].as_array().unwrap().len(),
            wire::CHROMIUM_IDS.len()
        );
        assert!(manifest.get("allowed_extensions").is_none());
        let script = std::fs::read_to_string(root.join("native-host")).unwrap();
        assert!(script.contains("'\\''"));
        restore_at(&root).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), original);
        std::fs::remove_dir_all(tmp).unwrap();
    }
    #[test]
    fn restore_does_not_overwrite_a_host_changed_by_another_application() {
        let tmp =
            std::env::temp_dir().join(format!("bw-browser-registration-{}", uuid::Uuid::new_v4()));
        let root = tmp.join("app");
        let target = tmp.join("hosts/com.8bit.bitwarden.json");
        install_at(
            Browser::Firefox,
            Path::new("/private/socket"),
            Path::new("/app/native"),
            Path::new("/app/proxy"),
            &root,
            &target,
        )
        .unwrap();
        let foreign = br#"{"path":"/new/other/host"}"#;
        std::fs::write(&target, foreign).unwrap();
        restore_at(&root).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), foreign);
        assert_eq!(load(&root).unwrap().len(), 1);
        std::fs::remove_dir_all(tmp).unwrap();
    }
}
