# Complete egui migration

Status: frontend replacement implemented on `egui-migration`; Linux acceptance checks passed. Iced checkpoint: `0945673`.
Interaction-demo checkpoint: `4316be8`.

## Completion criteria

The production binary must use egui with no Iced runtime/dependency. Existing
supported behavior must remain available; previously unsupported server writes,
SSO/WebAuthn and biometric unlock are not implicitly added by the UI migration.
No real credentials or certificates are copied into demo/test fixtures.

| Area | Required behavior | Status |
|---|---|---|
| Backend | Preserve SDK, mTLS transport, encrypted SQLite snapshots, scoped sessions | Preserved and tested |
| Runtime | Bounded asynchronous results, session/presentation/request fences | Implemented and tested |
| Login | Stored accounts, unlock, password login, TOTP/email, cancellation | Implemented and tested |
| Server settings | Persistent profiles, CA/client certificate import/removal and expiry errors | Implemented and tested |
| Vault | Real data, combined navigation/search, sorting, virtualized list, all detail types | Implemented and tested |
| Keyboard | List-scoped arrows, Tab, page keys, Escape, text-field/dialog isolation | Implemented and tested |
| Protected entries | Scoped password confirmation and retry, notes/custom fields | Implemented and tested |
| Forms | Existing supported item/folder create/edit/delete flows; remote read-only gates | Implemented and tested |
| Generator | Password/passphrase/username, account-scoped history and clipboard rules | Implemented and tested |
| Import/export | JSON/CSV/encrypted JSON, fresh scoped authorization and guarded staged exports; import remains the pre-existing backend stub | Implemented |
| Sends | Existing local/demo Send workflows | Implemented and tested |
| Preferences | Settings, per-account filters, theme/language/zoom/system fonts | Implemented; existing Fluent catalogues retained, new strings fall back to English |
| Desktop | Menus, tray hide/restore, single-instance wake, global hotkey/Magnify | Implemented and tested |
| SSH | Fixed socket, verified host binding, until-lock grants, focused approval viewport | Implemented and tested |
| Lifecycle | Immediate plaintext cleanup, stale-result rejection, durable logout, timeouts | Implemented and tested |
| Validation | Core security tests, egui interaction tests, real synthetic GUI run | Implemented and tested |

## Decisions

- Reuse the existing security/services code rather than substitute demo authentication.
- Use egui/eframe 0.37 and the proven OpenGL backend from the interaction demo.
- Read desktop fonts from the system; retain the original outlined wordmark.
- Keep UI transitions immediate and shadow-free. Use egui scrolling/virtualization.
- Keep operational secrets outside persisted egui widget memory.
- Preserve the normal application profile and opt-in synthetic `--demo` storage.

## Questions for the final joint review

Collect product questions here as they arise. Do not interrupt the migration for
routine technical choices. No questions blocked implementation.

### Collected review questions

- Open details on a single selection, as implemented now, or require double-click
  / Enter? The current implementation preserves selection-driven details on the
  right and closes them with Escape.
- Should a later pass make the theme follow the KDE accent palette as well as
  the desktop font and light/dark preference? The migration uses Bitwarden blue.
- Previously unsupported features remain explicit: remote writes, Send server
  publication, import, autostart, PIN and biometric unlock. Prioritize these
  together after reviewing the migrated UI.


## Validation record (2026-11-08)

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, and the
workspace release build succeed. Clippy still reports retained unused helper and
platform-specific API warnings.

- `cargo test --workspace`: 153 desktop tests plus one fixture-generator test
  pass. Two existing OS-dependent tests are ignored by default.
- Production dependency tree contains no Iced, wgpu, OpenEXR or TIFF. Rendering
  uses eframe/egui Glow/OpenGL. The former vendored Iced renderer patch is gone.
- Native KDE/Wayland acceptance with an isolated synthetic profile: tray hide
  destroys the surface; tray activation recreates it; native close hides;
  a second process activates the primary; tray exit completes cleanly.
- Native X11/Xvfb acceptance with SDK-encrypted synthetic SSH keys and a separate
  socket: identities appear after unlock; an authentication request while hidden
  opens only the approval surface; Enter returns a signature; the popup closes
  while the main window stays hidden; locking removes identities; exit removes
  the socket. Xvfb has no window manager, so this test explicitly grants native
  focus; egui tests verify initial button focus and the emitted activation request.
- System fonts resolve through fontconfig on Linux and are cached across native
  surface recreation. No production text font is embedded. The original SVG
  wordmark remains independent of the desktop text font.
- Windows/macOS conditional integration code is retained but has not been run
  on those operating systems during this migration.

Existing backend limits remain visible: synchronized vaults are read-only;
import and Send server publication are not implemented; PIN, biometrics and
launch-at-login remain unsupported. This migration does not claim those as
newly working features.

The existing `device-id` file is reused for password login. mTLS/credential-store
transport preparation runs on blocking workers, so certificate/keyring access
cannot stall the UI event thread. Certificate settings for an existing account
edit its bound profile rather than the prelogin default.
