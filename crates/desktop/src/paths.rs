//! Explicit demo profiles and stable OS application storage.

use std::{ffi::OsString, io, path::PathBuf, sync::OnceLock};

#[derive(Debug)]
struct Profile {
    directory: PathBuf,
    demo: bool,
}

static PROFILE: OnceLock<Profile> = OnceLock::new();

fn parse_profile(
    args: impl IntoIterator<Item = OsString>,
    local_data: Option<PathBuf>,
) -> Result<Profile, String> {
    let mut args = args.into_iter();
    match args.next() {
        None => Ok(Profile {
            directory: local_data
                .ok_or("Cannot resolve the OS application data directory")?
                .join("bitwarden-desktop-next"),
            demo: false,
        }),
        Some(flag) if flag == "--demo" => {
            let directory = args
                .next()
                .ok_or("Usage: bitwarden-desktop-next --demo <directory>")?;
            if directory.is_empty() || args.next().is_some() {
                return Err("Usage: bitwarden-desktop-next --demo <directory>".into());
            }
            Ok(Profile {
                directory: PathBuf::from(directory),
                demo: true,
            })
        }
        Some(_) => Err("Usage: bitwarden-desktop-next [--demo <directory>]".into()),
    }
}

pub fn initialize() -> Result<(), String> {
    let mut profile = parse_profile(std::env::args_os().skip(1), dirs::data_local_dir())?;
    if profile.demo {
        // Demo startup must never create or seed an implicit directory.
        profile.directory = profile
            .directory
            .canonicalize()
            .map_err(|e| format!("Cannot open demo directory: {e}"))?;
        if !profile.directory.join("mock.json").is_file() {
            return Err(
                "Demo directory has no mock.json; generate it with fake-data --output <directory>"
                    .into(),
            );
        }
        tracing::warn!("Explicit demo profile: synthetic accounts only");
    } else {
        prepare_private_directory(&profile.directory)
            .map_err(|e| format!("Cannot prepare application data directory: {e}"))?;
    }
    PROFILE
        .set(profile)
        .map_err(|_| "Profile already initialized".into())
}

fn prepare_private_directory(path: &std::path::Path) -> io::Result<()> {
    if let Ok(meta) = path.symlink_metadata()
        && (meta.file_type().is_symlink() || !meta.is_dir())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Application data path must be a directory, not a link",
        ));
    }
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn initialized_data_dir() -> Option<PathBuf> {
    PROFILE.get().map(|p| p.directory.clone())
}

pub fn data_dir() -> PathBuf {
    PROFILE
        .get()
        .expect("Profile initialized before application startup")
        .directory
        .clone()
}

pub fn is_demo() -> bool {
    PROFILE.get().is_some_and(|p| p.demo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_startup_uses_only_os_storage() {
        let p = parse_profile([], Some(PathBuf::from("/os-private"))).unwrap();
        assert!(!p.demo);
        assert_eq!(
            p.directory,
            PathBuf::from("/os-private/bitwarden-desktop-next")
        );
        assert!(parse_profile([], None).is_err(), "never fall back to cwd");
    }

    #[test]
    fn demo_requires_an_explicit_directory_and_rejects_extra_arguments() {
        assert!(parse_profile(["--demo".into()], None).is_err());
        assert!(parse_profile(["--demo".into(), "".into()], None).is_err());
        assert!(parse_profile(["--demo".into(), "demo".into(), "extra".into()], None).is_err());
        let p = parse_profile(["--demo".into(), "explicit-demo".into()], None).unwrap();
        assert!(p.demo);
        assert_eq!(p.directory, PathBuf::from("explicit-demo"));
    }

    #[cfg(unix)]
    #[test]
    fn storage_is_private_and_refuses_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let root = std::env::temp_dir().join(format!("bw-profile-test-{}", uuid::Uuid::new_v4()));
        let private = root.join("private");
        prepare_private_directory(&private).unwrap();
        assert_eq!(
            private.metadata().unwrap().permissions().mode() & 0o777,
            0o700
        );
        let link = root.join("link");
        symlink(&private, &link).unwrap();
        assert!(prepare_private_directory(&link).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
