//! Per-user XDG desktop autostart. Only an executable path is stored; existing unlock preferences still apply.
pub const SUPPORTED: bool = cfg!(target_os = "linux");
#[cfg(target_os = "linux")]
pub use linux::{configure, enabled};
#[cfg(not(target_os = "linux"))]
pub fn enabled() -> Result<bool, String> {
    Ok(false)
}
#[cfg(not(target_os = "linux"))]
pub fn configure(_: bool) -> Result<bool, String> {
    Err("Autostart currently supports Linux".into())
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        fs,
        io::{Read, Write},
        os::unix::fs::OpenOptionsExt,
        path::{Path, PathBuf},
    };
    const FILE: &str = "bitwarden-desktop-next.desktop";
    const OWNER: &str = "X-Bitwarden-Native-Autostart=true";
    #[cfg(not(test))]
    fn config_home() -> Result<PathBuf, String> {
        dirs::config_dir()
            .filter(|p| p.is_absolute())
            .ok_or("Cannot resolve the desktop configuration directory".into())
    }
    #[cfg(test)]
    fn config_home() -> Result<PathBuf, String> {
        Err("Autostart tests require an isolated configuration directory".into())
    }
    fn read_entry(path: &Path) -> Result<Option<String>, String> {
        let meta = match path.symlink_metadata() {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("Cannot read the desktop autostart entry".into()),
        };
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 16 * 1024 {
            return Err("The desktop autostart entry must be a regular file".into());
        }
        let mut text = String::new();
        fs::File::open(path)
            .and_then(|f| f.take(16 * 1024 + 1).read_to_string(&mut text))
            .map_err(|_| "Cannot read the desktop autostart entry")?;
        if text.len() > 16 * 1024 {
            return Err("Desktop autostart entry is too large".into());
        }
        Ok(Some(text))
    }
    fn desktop_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
        let mut in_group = false;
        let mut result = None;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                in_group = line == "[Desktop Entry]";
            }
            if in_group
                && let Some((name, value)) = line.split_once('=')
                && name.trim() == key
            {
                result = Some(value.trim());
            }
        }
        result
    }
    fn entry_enabled(text: &str) -> bool {
        desktop_value(text, "Type") == Some("Application")
            && desktop_value(text, "Hidden") != Some("true")
            && desktop_value(text, "X-GNOME-Autostart-enabled") != Some("false")
    }
    fn enabled_at(root: &Path) -> Result<Option<bool>, String> {
        Ok(read_entry(&root.join("autostart").join(FILE))?.map(|s| entry_enabled(&s)))
    }
    pub fn enabled() -> Result<bool, String> {
        if let Some(enabled) = enabled_at(&config_home()?)? {
            return Ok(enabled);
        }
        // User Hidden=true overrides a system entry with the same filename.
        let system = std::env::var_os("XDG_CONFIG_DIRS")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/etc/xdg".into());
        for root in std::env::split_paths(&system).filter(|p| p.is_absolute()) {
            if let Some(enabled) = enabled_at(&root)? {
                return Ok(enabled);
            }
        }
        Ok(false)
    }
    fn desktop_string(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }
    fn entry(executable: &Path, enabled: bool) -> Result<String, String> {
        let text = executable
            .to_str()
            .ok_or("Autostart requires a UTF-8 executable path")?;
        if !executable.is_absolute() || text.chars().any(char::is_control) {
            return Err(
                "Autostart requires an absolute executable path without control characters".into(),
            );
        }
        // GLib cannot resolve a program path containing an escaped percent.
        if text.contains('%') {
            return Err(
                "Install or move the app to a path without '%' before enabling autostart".into(),
            );
        }
        let mut quoted = String::from("\"");
        for c in text.chars() {
            if matches!(c, '\\' | '"' | '`' | '$') {
                quoted.push('\\');
            }
            quoted.push(c);
        }
        quoted.push('"');
        Ok(format!(
            "[Desktop Entry]\nType=Application\nName=Bitwarden [Next]\nComment=Open the desktop vault at login\nExec={}\nTryExec={}\nIcon=bitwarden-desktop-next\nTerminal=false\nHidden={}\n{}\n",
            desktop_string(&quoted),
            desktop_string(text),
            !enabled,
            OWNER
        ))
    }
    fn configure_at(root: &Path, executable: &Path, enabled: bool) -> Result<bool, String> {
        if enabled {
            use std::os::unix::fs::PermissionsExt;
            let meta = executable.metadata().map_err(
                |_| "The app executable is unavailable; restart the updated app and retry",
            )?;
            if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
                return Err("The app executable is not executable".into());
            }
        }
        let directory = root.join("autostart");
        let target = directory.join(FILE);
        if let Some(previous) = read_entry(&target)?
            && desktop_value(&previous, "X-Bitwarden-Native-Autostart") != Some("true")
        {
            return Err("A custom Bitwarden autostart entry already exists. Manage it in your desktop's autostart settings.".into());
        }
        let text = entry(executable, enabled)?;
        fs::create_dir_all(&directory)
            .map_err(|_| "Cannot create the desktop autostart directory")?;
        let temporary =
            directory.join(format!(".bitwarden-autostart-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> std::io::Result<()> {
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
            drop(f);
            fs::rename(&temporary, &target)?;
            fs::File::open(&directory)?.sync_all()
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(
                "Could not save the desktop autostart entry; check directory permissions".into(),
            );
        }
        Ok(enabled)
    }
    pub fn configure(enabled: bool) -> Result<bool, String> {
        let root = config_home()?;
        let executable =
            std::env::current_exe().map_err(|_| "Cannot resolve the app executable")?;
        configure_at(&root, &executable, enabled)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::fs::{PermissionsExt, symlink};
        #[test]
        fn desktop_entry_enables_disables_and_launches_literal_special_paths() {
            let root = std::env::temp_dir().join(format!("bw-autostart-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            let executable = root.join("app with $ ` and \\\" quotes");
            let marker = root.join("launched");
            fs::write(
                &executable,
                format!("#!/bin/sh\nprintf ok > '{}'\n", marker.display()),
            )
            .unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
            assert_eq!(enabled_at(&root).unwrap(), None);
            configure_at(&root, &executable, true).unwrap();
            let file = root.join("autostart").join(FILE);
            assert_eq!(enabled_at(&root).unwrap(), Some(true));
            assert_eq!(
                fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
            if let Ok(validation) = std::process::Command::new("desktop-file-validate")
                .arg(&file)
                .output()
            {
                assert!(
                    validation.status.success(),
                    "{}",
                    String::from_utf8_lossy(&validation.stdout)
                );
                let result = std::process::Command::new("gio")
                    .arg("launch")
                    .arg(&file)
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                for _ in 0..100 {
                    if marker.exists() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                assert_eq!(fs::read_to_string(&marker).unwrap(), "ok");
            }
            configure_at(&root, &executable, false).unwrap();
            assert_eq!(enabled_at(&root).unwrap(), Some(false));
            // GNOME's external disable flag must also be reflected in the app.
            fs::write(
                &file,
                format!(
                    "{}X-GNOME-Autostart-enabled=false\n",
                    entry(&executable, true).unwrap()
                ),
            )
            .unwrap();
            assert_eq!(enabled_at(&root).unwrap(), Some(false));
            fs::remove_dir_all(root).unwrap();
        }
        #[test]
        fn custom_entries_symlinks_and_missing_executables_are_preserved() {
            let root = std::env::temp_dir().join(format!("bw-autostart-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(root.join("autostart")).unwrap();
            let file = root.join("autostart").join(FILE);
            let executable = Path::new("/bin/sh");
            assert!(entry(Path::new("/tmp/app%percent"), true).is_err());
            assert!(entry(Path::new("/tmp/app\nName=injected"), true).is_err());
            fs::write(&file, "custom entry").unwrap();
            assert!(configure_at(&root, executable, true).is_err());
            assert_eq!(fs::read_to_string(&file).unwrap(), "custom entry");
            fs::remove_file(&file).unwrap();
            assert!(configure_at(&root, &root.join("missing"), true).is_err());
            symlink(root.join("elsewhere"), &file).unwrap();
            assert!(configure_at(&root, executable, false).is_err());
            assert!(!root.join("elsewhere").exists());
            fs::remove_dir_all(root).unwrap();
        }
    }
}
