# Ubuntu 24.04 packages

Build a native **amd64** `.deb` on an amd64 Linux host with Podman (preferred) or
Docker and a recent Rust toolchain compatible with `Cargo.lock`. The package is
compiled inside Ubuntu 24.04, rather than wrapping a host binary that may require
newer glibc. The first validated build used Rust 1.99.1.

```sh
python3 tools/build-ubuntu-deb.py
# Optional: seed dependency downloads from an existing Cargo home.
python3 tools/build-ubuntu-deb.py --cargo-cache "$HOME/.cargo"
```

Use `--toolchain /path/to/toolchain` to select a directory containing `bin/cargo`
and `bin/rustc`, or `--engine docker` to select Docker. The host toolchain must
itself run on Ubuntu 24.04. Build output/caches live in `target/ubuntu-24.04`;
packages and SHA-256 checksums are written to `dist/ubuntu-24.04`.
Container user mapping preserves developer ownership of caches and artifacts
with rootless Podman or rootful/rootless Docker.
`--package-only` repackages an existing successful build recorded in
`target/ubuntu-24.04/build-info.json`.

Only tracked source files from the working tree are copied into the container.
Vault data, `.git`, preview profiles and local logs are not mounted. The lockfile
is mandatory. This is a repeatable build recipe, not a claim of byte-for-byte
reproducibility; Ubuntu package updates and the chosen Rust toolchain can change
output.

The `.deb` installs:

- `/usr/bin/bitwarden-desktop-next` and the browser native-messaging helper beside it;
- an application-menu entry and the shield icon;
- license notices and `README.Debian` under `/usr/share/doc/bitwarden-desktop-next`.

ELF runtime dependencies are determined by Ubuntu's `dpkg-shlibdeps`; dynamically
loaded graphics libraries are declared separately. A Secret Service wallet is
required. The package recommends GNOME Keyring or the KDE wallet runtime.
Ubuntu 24.04/KWallet 5 additionally needs `libqca-qt5-2-plugins` for encrypted
Secret Service sessions; GPG wallets need a graphical pinentry such as
`pinentry-qt`. These are suggested, not installed for every desktop. Restart
KWallet or log out/in after installing its QCA plugin. See the
[real credential-store acceptance tests](../../docs/testing.md#real-credential-store-smoke-test). Tray
support on GNOME needs an AppIndicator extension; the main window remains usable
without it. No autostart entries or browser registrations are installed automatically.

Install the downloaded package on Ubuntu 24.04 using APT, which also installs its
runtime dependencies:

```sh
sudo apt install ./bitwarden-desktop-next_*.deb
```

Use the applications menu or run `bitwarden-desktop-next`. Remove with
`sudo apt remove bitwarden-desktop-next`; user vault data is retained.

Validation should include package metadata/contents, `ldd` and GLIBC symbol
requirements on Ubuntu 24.04, plus an installed-app smoke test in a separate
Ubuntu 24.04 environment with a fresh empty profile. A container smoke test does
not substitute for KDE/GNOME login, wallet, hardware graphics and browser testing.

## Validated build

Source `8cf10baca6f1` was built with Rust 1.99.1 on Ubuntu 24.04.5 amd64.
The resulting package is 13,439,112 bytes, with 45,799 KiB installed size and
a `libc6 (>= 2.39)` dependency. Validation in a separate Ubuntu 24.04 container
passed APT installation, ELF library resolution for both binaries, visible GUI
startup with an empty profile under Xvfb/Mesa software rendering, and removal.
The login screen was also visually inspected. No real vault data was used.
