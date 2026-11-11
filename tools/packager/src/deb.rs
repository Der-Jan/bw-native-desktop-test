//! Resolve ELF dependencies using the target distribution's package metadata.
use std::{env, fs, io, path::Path, process::Command};

pub fn prepare() -> io::Result<()> {
    let formats = env::var("CARGO_PACKAGER_FORMATS").unwrap_or_default();
    if !formats.split(',').any(|format| format == "deb") {
        return Ok(());
    }
    // Run after the release build, before cargo-packager reads the dependency file.
    let binaries = ["bitwarden-desktop-next", "bitwarden-browser-proxy"]
        .map(|name| Path::new("target/release").join(name).canonicalize())
        .into_iter()
        .collect::<io::Result<Vec<_>>>()?;
    let root = Path::new("target/deb-shlibdeps");
    fs::create_dir_all(root.join("debian"))?;
    fs::write(
        root.join("debian/control"),
        "Source: bitwarden-desktop-next\nSection: utils\nPriority: optional\nMaintainer: Bitwarden Inc.\nStandards-Version: 4.7.0\n\nPackage: bitwarden-desktop-next\nArchitecture: any\nDescription: Bitwarden desktop password manager\n",
    )?;
    let mut command = Command::new("dpkg-shlibdeps");
    command.current_dir(root).arg("-O");
    for binary in binaries {
        command.arg("-e").arg(binary);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "dpkg-shlibdeps failed (build on the target Debian/Ubuntu distribution with dpkg-dev installed): {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let output = String::from_utf8(output.stdout).map_err(io::Error::other)?;
    let dependencies = output
        .lines()
        .find_map(|line| line.strip_prefix("shlibs:Depends="))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| io::Error::other("dpkg-shlibdeps returned no dependencies"))?;
    let mut lines = dependencies.split(',').map(str::trim).collect::<Vec<_>>();
    // Runtime-loaded libraries do not appear in ELF DT_NEEDED. Keep these in
    // sync with tools/packaging/make-ubuntu-deb.py.
    lines.extend([
        "ca-certificates",
        "libegl1",
        "libgl1",
        "libxkbcommon0",
        "libxkbcommon-x11-0",
        "libxcursor1",
        "libxi6",
        "libxrandr2",
    ]);
    // Provider and QCA/pinentry requirements depend on the desktop; see README.
    lines.sort_unstable();
    lines.dedup();
    fs::write("target/deb-dependencies.txt", lines.join("\n") + "\n")
}
