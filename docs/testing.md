# Validation

Run `cargo test --workspace` and `cargo clippy --workspace --all-targets`.
The workspace is intentional: SDK feature unification differs for a single
package invocation. `cargo fmt --all --check` checks formatting.

Backend tests cover mTLS login/sync against disposable TLS servers, invalid and
expired certificates, origin permissions, encrypted SQLite snapshot commit and
restart, revocation/leases, durable logout, staged exports, one-shot protected
operations, clipboard ownership/TTL, timeout calculations, SSH signing/session
binding, socket permissions and tray protocol behavior. Three OS-dependent tests
are ignored by default (a real OpenSSH server, a desktop StatusNotifier host and
a native Wayland/OpenGL settings-size test).

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

Local-edit validation uses disposable mTLS accounts and real SDK SQLite storage:
encrypted title/username/password persistence, lock/restart, stale revision
rejection, metadata ownership checks, permission denial, one-shot password
authorization, organization-key reload, read-sync pause and per-item discard.
Request counts assert that saves and discards never contact the server.

The 2026-11-08 follow-up passes 165 desktop tests and one fixture test (two
existing OS tests ignored at that checkpoint). Native synthetic-profile checks cover settings
without a scrollbar at 1080px monitor height and with a scrollbar at 700px,
independent tool windows, and editing a real-account cache followed by process
restart with the modified name/username still present.

The settings-size regression additionally runs on the real Wayland compositor:
`cargo test --workspace native_wayland_settings_fit_the_available_screen -- --ignored --nocapture`.
It uses only a synthetic in-memory account and closes its native windows after
two seconds. Before the fix the native window stayed at 760px for 869px of
content; after the fix it reaches 893px on the same desktop. This catches the
Wayland behavior that the earlier X11/Xvfb checks did not reproduce.

The browser integration checkpoint passes 178 desktop tests and one fixture
test, with three OS-dependent tests ignored. It adds SDK Noise and legacy native
messaging interoperation, approval/revocation, account identity and registration
backup checks. The separate Helium/EGL synthetic native test passes approval,
denial, until-lock reuse and disconnect on desktop lock. See
[browser integration](browser-integration.md) for setup, acceptance scope and the
unresolved raw-Xvfb GLX child-window limitation.
The expanded Settings also passes the native Wayland sizing test: 1170px of
actual height for 1146px of content on the 1878px logical monitor.

Permanent browser pairing passes 181 desktop tests and one fixture test. Added
coverage includes restart/reconnect reuse, lock and wrong-account rejection,
all browser/extension/account/server identity fields, durable revocation,
storage failure and invalidated approvals. Actual egui frames verify the default
Always action, alternate durations, Enter focus and Escape denial. A synthetic
Helium run confirms that Enter pairs once and the next request needs no prompt;
Light and Dark native views and expanded details were inspected. The expanded
Settings passes the Wayland sizing test (1206px window, 1182px content).
