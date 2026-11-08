# Validation

Run `cargo test --workspace` and `cargo clippy --workspace --all-targets`.
The workspace is intentional: SDK feature unification differs for a single
package invocation. `cargo fmt --all --check` checks formatting.

Backend tests cover mTLS login/sync against disposable TLS servers, invalid and
expired certificates, origin permissions, encrypted SQLite snapshot commit and
restart, revocation/leases, durable logout, staged exports, one-shot protected
operations, clipboard ownership/TTL, timeout calculations, SSH signing/session
binding, socket permissions and tray protocol behavior. Two OS-dependent tests
are ignored by default (a real OpenSSH server and a desktop StatusNotifier host).

`core/tests.rs` covers presentation/session revocation, late workers, plaintext
cleanup, protected prompt persistence/cancellation/retry and all eight SDK draft
schemas. `ui/tests.rs` runs actual production egui frames with a test-only font:
list focus, arrows/page keys, Escape, text-input isolation, password prompt focus
and Enter, and secret state cleanup. Tests clear unconsumed texture deltas.

Normal application startup never loads mock vaults. For manual native testing,
generate a new disposable fixture with `fake-data --output` and launch it through
`--demo`. Use a distinct SSH socket override for parallel test instances. Never
copy credentials, certificates or a real account database into tests; capture
screenshots only of the exact synthetic test process/window.

For a local Linux build/preview, register its desktop launcher from the normal
desktop environment (before the launcher overrides XDG profile directories):
`python3 tools/install-linux-desktop.py /absolute/path/to/launcher-or-binary`.
This installs a user-local desktop entry and the existing blue shield icon.
The Wayland app ID matches cargo-packager's `bitwarden-desktop-next.desktop`
filename. Without a matching desktop entry KDE may display its generic yellow
Wayland W instead of the embedded window icon. SSH and quick-search windows use
the same app ID and shield. Installed packages provide their own desktop entry.

The independent comparison demo can be tested separately:
`cargo test --manifest-path tools/egui-demo/Cargo.toml`.
