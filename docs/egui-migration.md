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
| Forms | Existing local item/folder create/edit/delete; encrypted local edits of server entries | Implemented and tested; no HTTP writes |
| Generator | Password/passphrase/username, account-scoped history and clipboard rules | Implemented and tested |
| Import/export | JSON/CSV/encrypted JSON exports retain scoped authorization; Import is disabled in the menu and dispatcher pending implementation | Export implemented; Import grayed out |
| Sends | Existing local/demo code retained; navigation disabled pending server support | Grayed out |
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

### Accepted review decisions

- Details open on single selection and follow the list's arrow-key cursor.
  Escape closes details while preserving selection. No double-click/Enter gate.
  List labels do not intercept row clicks; a rendered-label pointer test covers
  clicking the item name itself, alongside the existing keyboard tests.
- System mode follows the complete KDE window/view/button/text palette and
  its light/dark mode, in addition to selection, focus and link colors.
  Explicit Light and Dark modes remain available in Settings, including while
  locked or before login. KDE role colors take precedence over a conflicting
  generic GTK/native color-scheme hint. Appearance options and typography survive
  secret-widget cleanup on lock/unlock; secret inputs and undo state still clear.
  Selection, focus, links and button interaction states follow the desktop
  accent. On Linux, the appearance portal supplies startup values and live change
  notifications; KDE's selection color is the fallback when no portal accent is
  available. Config/portal access runs off the UI thread. The Bitwarden wordmark
  retains its existing blue/white branding. Selection text and focus/link colors
  adapt for contrast with bright and dark accents.

### Remaining review question

- Prioritization of the already unsupported backend features is still open:
  remote writes, Send server publication, import, autostart, PIN and biometrics.
  Approval of the interaction and appearance decisions does not start this work.

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

Existing backend limits remain visible: server writes remain disabled;
import and Send server publication are not implemented; PIN, biometrics and
launch-at-login remain unsupported. This migration does not claim those as
newly working features.

The existing `device-id` file is reused for password login. mTLS/credential-store
transport preparation runs on blocking workers, so certificate/keyring access
cannot stall the UI event thread. Certificate settings for an existing account
edit its bound profile rather than the prelogin default.


## Approved review follow-up

The single-selection interaction is retained and now also covered by a rendered
item-name pointer test (previously the label's text selection intercepted it).
Desktop accent parsing and selection/focus contrast add four focused tests.
The workspace suite passes 158 desktop tests and one fixture-generator test;
the same two environment-dependent tests remain ignored. Release build,
formatting and clippy succeed with the retained helper warnings. A native
synthetic-vault run confirms KDE-colored selections, readable selected labels,
working item-name clicks and the unchanged SVG wordmark.

## Full system palette and desktop icon follow-up

The full KDE palette (window, view, button and text roles) now comes from the
host settings portal, even when the preview uses isolated XDG profile paths.
System mode follows KDE's light/dark palette; explicit Light and Dark remain
available in Settings before and after unlock. Secret-widget cleanup retains
only appearance/font options while resetting widget memory. The workspace suite
passes 161 desktop tests and one fixture-generator test, with the same two
environment-dependent tests ignored. Synthetic native screenshots verify the
window/view colors before and after unlock in System, Light and Dark modes.

Window app IDs now match the packager's desktop entry basename. The local-build
desktop registration helper supplies the existing blue Bitwarden shield; the
yellow/white W was KDE's Wayland fallback for an unresolved desktop entry.
Auxiliary windows share the app ID and cached shield image.

Settings and Generator now use separate resizable native windows, sharing the
desktop palette, fonts and app icon. Escape closes the focused tool window.
Their scrollbars reserve space and show a solid, contrasting handle. Generator
uses four framed tabs with a colored active underline: Password, Passphrase,
Username and History. History has its own content page; the menu's Generator
history action selects it directly. Clipboard copies retain sensitive-data TTL
handling, child-window interaction records activity, and screen-capture
protection follows the configured preference on supported platforms.

Settings now size themselves to the measured content height, capped below the
monitor edges for desktop panels and window decorations. A scrollbar appears
only if the content cannot fit (or the user shrinks the window). Generator
history and zoom entries were removed from the menu; history remains a generator
tab and zoom controls remain in Settings. Local Linux launchers use an absolute
shield icon path so discovery does not depend on an icon-theme size directory.

Synchronized entries now allow guarded local content edits, saved as encrypted
snapshot overlays. They survive lock/restart and can be discarded individually.
No server write occurs; downloads pause while changes are pending. The UI marks
unsent changes and explains that logout deletes the local account data. New
backend tests cover encrypted persistence, stale drafts, ownership, permissions,
master-password grants, discard, organization-key reload and no HTTP writes.
