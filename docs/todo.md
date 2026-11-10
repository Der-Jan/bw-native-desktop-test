# TODO

> The production frontend migrated to egui on 2026-11-08. Historical Iced
> implementation notes below describe the preserved checkpoint `0945673`.
> Current architecture and supported behavior: [architecture](architecture.md)
> and [migration](egui-migration.md).


Open work, organized by **what it asks of the picker** rather than by importance. Pick by
what unblocks you or what's interesting — order within each section is not a queue.

**Keep this file in sync.** Finish (or partially finish) a task → update or remove its
entry in the same change. Reduced scope is fine — shrink the entry to what's still left.

Completed work isn't tracked here; check `git log` or [docs/decisions.md](./decisions.md)
for context.

## Security and server integration prerequisites

The proposed sequence and acceptance gates are in the
[security remediation concept](security-concept.md). These tasks are open;
the first runtime/session and demo-storage slice is implemented; remaining work is listed here.

- **Production storage platform validation** `[M]` — validate Windows ACLs and
  macOS/Windows credential-store behavior. AEAD credential persistence with OS
  keys, private Unix paths, pending-login recovery and orphan cleanup are implemented.
- **Platform durability** `[L]` — validate Windows durability and credential-store
  behavior. Targeted server token/session revocation is blocked by the audited
  Vaultwarden 1.37.5 API; `clear-token` is push-only and the security-stamp reset
  affects all devices. Keep offline-safe local logout and do not substitute a
  global reset. Revisit when a supported targeted API exists; see the audit in
  [login/sync requirements](login-sync-requirements.md).
- **Protected action authorization parity** `[M]` — one-use account/revision/action
  grants, detail/copy/edit/save/delete gates and fresh export authorization are wired.
  Add policy-aware selective field display (current `view_password=false` fails closed
  for full details) and platform acceptance. Vaultwarden generator, item-type, personal
  ownership and URI-default policies are enforced from the encrypted snapshot;
  master-password requirements are checked at login. See the policy scope below.
- **Secret-safe diagnostics** `[M]` — continue the general redaction audit.
  Profile-scoped favicon requests are implemented with account TLS/mTLS, no OAuth
  headers, no redirects/public fallback and no raw domain/error logging.
- **Remaining login challenges and encrypted sync** `[L]` — password/TOTP/email
  login, OS-protected tokens and full read-sync snapshots are wired; unsupported
  challenges fail explicitly. Remaining sync work includes complete key migrations, item
  permanent deletion, ownership/sharing transfers and Send.
  Incoming WebSocket push with focus/reconnect revision checks (no periodic polling), new-item
  creation with server-ID reconciliation and SDK-encrypted attachment transfers
  now work through the account-scoped TLS/mTLS transport. Archive/unarchive also
  synchronizes immediately with durable intent and read-only outcome confirmation.
  Trash/Restore are also synchronized with fresh permissions, a durable lifecycle
  intent and safe lost-reply recovery. Same-account refresh preserves list position. Uploads are bounded
  in-memory operations; automatic offline file resumption is deferred.
  Folder create/rename/delete now use encrypted, account-scoped requests with
  durable intent, read-after-write reconciliation and preserved entries/drafts.
  Existing-entry update sync is implemented: automatic uploads after Save, manual
  retry, revision conflicts,
  encrypted draft retention, read-after-write reconciliation and per-item durable
  acknowledgments. Deleted/inaccessible draft recovery into new items remains open. Exclude Bitwarden-only passwordless passkey login and unsupported Enterprise
  policies. Follow
  [login and sync requirements](login-sync-requirements.md), including SDK proof,
  challenge handling, token persistence, transactional snapshots and online writes.
- **Native mTLS completion** `[L]` — scoped login/probe HTTP transport, import,
  expiry checks and offline recovery settings are wired. Complete the remaining gates in [mTLS requirements](mtls-requirements.md), including
  account-scoped bindings from the iOS work, endpoint isolation, secret storage,
  HTTP/WSS coverage, expiry recovery and certificate replacement.
- **Remaining iOS sorting parity** `[M]` — persistent Name / Last changed / displayed Base URL column sorting is implemented. Add locale collation and the distinct raw-URI / registrable-domain modes; follow
  [iOS sorting requirements](vault-sorting-requirements.md), including four modes,
  stable selection, domain fixtures and clearing derived keys on lock.

## Conventions

Each entry carries two inline tags so you can size it at a glance:

| Tag            | Meaning                                                                       |
| -------------- | ----------------------------------------------------------------------------- |
| `[S]`          | A few hours; one or two files; spec is unambiguous.                           |
| `[M]`          | One or two days; spans a feature; a couple of small judgement calls.          |
| `[L]`          | Multi-PR, multi-day; meaningful surface area.                                 |
| `[plan]`       | Needs a written plan before code.                                             |
| `[defer: …]`   | Spec is clear, but landing it is wasted work until the named condition holds. |
| `[blocked: …]` | Can't progress until the named external thing changes.                        |

## Contents

- [Security and server integration prerequisites](#security-and-server-integration-prerequisites)
- [Quick wins](#quick-wins)
- [Auth & onboarding](#auth--onboarding)
- [Vault & Send](#vault--send)
- [Magnify launcher](#magnify-launcher)
- [Settings — runtime wiring](#settings--runtime-wiring)
- [Polish & refactors](#polish--refactors)
- [Plan-first](#plan-first)
- [Blocked on Bitwarden SDK](#blocked-on-bitwarden-sdk)
- [Blocked / waiting on other upstream](#blocked--waiting-on-other-upstream)

---

## Quick wins

Each is small enough to land in one focused session.

- **Master-password hint request** `[S]` — "Get master password hint" link on the
  login-password screen sends a hint request to the server. One SDK call + one toast.
- **"Always show dock" (macOS)** `[S]` — `NSApplication.setActivationPolicy`. Single API
  call; macOS-only build branch.
- **Edit menu — Undo / Redo / Cut / Copy / Paste / Select all** `[plan]` — currently
  rendered as **disabled placeholders** (`EnabledWhen::Never`) so the menu still mirrors
  the official client visually. Iced text widgets already handle all six via the keyboard;
  menu wiring would need a focused-widget dispatcher. Decide whether to (a) build that
  dispatcher, or (b) remove the entries entirely and live on keyboard shortcuts only — the
  official client gets these "for free" via `role: "undo"` etc., a luxury we don't have.
- **Magnify footer "More" expander** `[S]` — the launcher binds
  `Ctrl+C / Ctrl+Shift+C / Ctrl+T / Ctrl+U / Ctrl+Shift+N` but the hint bar still surfaces
  the first three only. Replace the trailing slots with a "More" disclosure
  (cursor-positioned popover or expand-on-hover row).
- **Magnify decrypt-failure feedback** `[S]`
  `[blocked: tray-balloon / OS-notification path]` — `Err` from `full_cipher` only logs at
  `warn` and the launcher silently dismisses
  ([handlers/magnify.rs](../crates/desktop/src/app/handlers/magnify.rs)
  `FieldDecryptCompleted`). Hook into a notification path once one exists.
- **Tray icon Linux test-VM verification** `[S]` `[blocked: Linux test machine]` —
  packaging-side wiring is in (`.deb` / `.pacman` declare `libayatana-appindicator3-1` /
  `libayatana-appindicator`; README notes the SNI-host requirement). Verify GNOME +
  AppIndicator extension, KDE Plasma, and sway + waybar on a VM.
- **Per-type item-list icons** `[S]` — non-login ciphers fall back to
  `favicon::globe_handle()` in
  [views/vault/widgets/item_list.rs](../crates/desktop/src/views/vault/widgets/item_list.rs).
  Card / Identity / Note / SSH-key entries each have a BWI glyph already used in the
  magnify launcher and detail headers; mirror that mapping here so the list shows a
  type-specific icon.

---

## Auth & onboarding

The login command is the keystone — most of this section flows from it.

### Implement the login command `[L]`

[crates/desktop/src/views/login/login_email.rs](../crates/desktop/src/views/login/login_email.rs)
stubs after `ContinueWithEmail` — no network call, no session creation. Wire
`bitwarden_auth`'s password-login API through `ClientManager` so a user with an email +
master password can authenticate (not just unlock an existing `fake-data`-seeded SQLite).
Handle CAPTCHA / device-verification responses (toast + retry); sanitise SDK errors per
the toast rule. 2FA screens are out of scope — see "Two-factor authentication" below.

### Remaining Vaultwarden authentication `[L]`

TOTP and email 2FA are implemented. Remaining supported server methods are
WebAuthn/FIDO2 2FA, YubiKey OTP and Duo, subject to server configuration. Device
approval is a separate supported login flow. Passwordless passkey vault login is
not supported by audited Vaultwarden and is outside scope. Keep unsupported
challenges explicit and fail closed; see the compatibility audit in
[login/sync requirements](login-sync-requirements.md).

### Registration view `[M]`

"Create account" link on login email screen. Email, password, hint fields. New sub-view
under `views/register/` plus a new `Register` screen variant or inlined into `LoginView`.

### Vaultwarden OIDC SSO login flow `[L]`

Implement only when supported/enabled by the configured Vaultwarden server.
Browser authorization redirect with bound state/nonce, PKCE and callback handling;
no direct SAML flow or Bitwarden-only SSO organization-policy requirement.

### Website icons for server accounts — implemented

The native client uses the account server's `/icons/<hostname>/icon.png`, including
base subpaths, through its revocable TLS/mTLS transport. The global icon-service
resolver and US-cloud fallback were removed. Lazy memory caching and an encrypted,
account/profile-bound SDK-SQLite cache reuse icons after sync/restart (seven-day
expiry, 512 entries, 2 MiB image-payload limit). Request/decoder limits,
profile/session revocation and texture invalidation are implemented.
See [login/sync requirements](login-sync-requirements.md). Automatically adopting a
separate cloud or server-advertised icon origin remains deferred; it must have an
explicit transport permission instead of silently receiving website domains.

### `LoginView` per-`AuthPage` field grouping `[M]` `[defer: Registration view]`

`AuthPage` carries all per-page state as inline fields; 15 match arms in
`LoginView::update` start with `if let AuthPage::Unlock { ... } = &mut self.auth_page`.
Extract into `UnlockState` / `LoginEmailState` / `LoginPasswordState` sub-structs. Cheaper
to do alongside the Registration touch.

---

## Vault & Send

### Right-click context menu for text inputs `[M]`

Cut / copy / paste / select-all on `TextInput` and the notes `TextEditor` in
`cipher_edit`. Iced 0.15 doesn't ship this — text widgets silently swallow right-clicks.
Shape: a `components::context_menu` wrapper that stacks `MouseArea::on_right_press` over
the child and shows our `DropDown` with the four actions. `TextEditor` already accepts
`Action::{Copy,Cut,Paste,SelectAll}` via `on_action` (direct wire-up). `TextInput` needs a
`widget::Id` per field + `widget::operation::text_input::{select_all, …}` dispatched as
`Task`s; paste reuses iced's clipboard shell. First pass anchors the menu to the field
(our `DropDown` is widget-anchored); cursor-anchored variant would need a small `DropDown`
extension for absolute-offset placement.

### Send — wire to real SDK `[L]`

Sends live as decrypted `SendView`s in an in-memory `HashMap` on
[`ClientManager`](../crates/desktop/src/services/sdk/mod.rs) (`list_sends` / `full_send` /
`save_send` / `delete_send`). Empty at startup; mutations never leave the process. Replace
the in-memory map with the SDK's `Repository<Send>` +
`SendClient::{encrypt, decrypt, decrypt_list}`. `ClientManager`'s methods are already
async + fallible so call sites don't change. Drop the `uuid::Uuid::new_v4()` id
fabrication in `save_send`; the placeholder `access_id` should come from
`CreateSendResponse`. Once the SDK returns a real `access_id` + key fragment,
`SendForm::send_link`
([state.rs](../crates/desktop/src/views/send/widgets/send_edit/state.rs)) can drop its
`http://vault.bitwarden.test/#/send/{access_id}` stub and use the user's configured
`server_url` with the actual URL shape.

### Send — file creation flow `[M]`

The "Choose file" button in the new-file-send branch
([widgets/send_edit/view.rs](../crates/desktop/src/views/send/widgets/send_edit/view.rs)
`file_section`) is a placeholder — message fires, handler is a no-op. Needs
`rfd::AsyncFileDialog::new().pick_file().await` (already a dep — see
[app/handlers/export.rs](../crates/desktop/src/app/handlers/export.rs) for the
`Task::perform` recipe) to populate `file_name` + `file_size_name`, plus
`SendClient::encrypt_file` / `encrypt_buffer` wiring.

### Send form — embedded password generator panel `[M]`

The password regenerate button currently fires `ClientManager::generate_password` with a
fixed request (14-char, all charsets, min 1 digit + 1 symbol — see
[send/handler.rs](../crates/desktop/src/views/send/handler.rs)
`regenerate_send_password`). The official client opens a contextual side-panel generator
instead: Password / Passphrase tabs, live preview with its own regenerate, length /
charset / min-number / min-special / avoid-ambiguous options, "Use this password" /
"Cancel" footer. On apply the value populates the form field; on cancel the form is
untouched. Open question: share [`GeneratorView`](../crates/desktop/src/views/generator/)
in an embedded "picker" mode (drop the Username tab, expose options + value, fire an apply
event) or build a new lightweight view — the embedded path is fewer LOC but adds a mode
switch to the existing modal.

### In-form validation surface `[M]` `[defer: 2nd required field on either form]`

Both `CipherForm` and `SendForm` use a single `is_valid()` method + the
`toast-required-fields` toast. Once required-field rules grow past one field, replace
`is_valid() -> bool` with `validate() -> HashMap<FieldId, &'static str>` returning
per-field error messages. Add `show_validation: bool` flipped on the first failed Save —
keeps first-view UX clean. Add `inputs::validated_text_field(...)` in
[components/inputs/](../crates/desktop/src/components/inputs/mod.rs) that paints a red
border via the `text_input::Style` closure when `Some(error)`. Toast stays as the global
"can't save yet" nudge but demoted to title only.

### Precompute lowercase search keys `[M]`

`VaultView::filter_items` re-lowercases `name`, `subtitle`, and URI per item per
keystroke. On the 20k loadtest account that's ~60k allocations per keystroke. Wrap
`CipherListView` in a
`CipherRow { inner: Arc<CipherListView>, name_lc: String, subtitle_lc: String, uri_lc: Option<String> }`
populated once on `ListLoaded`; filter against pre-lowered strings. Pairs with the
`Arc<[CipherListView]>` micro-opt below.

### `Arc<[CipherListView]>` micro-optimization `[M]` `[blocked: bench]`

`VaultView.items.all` and `.cached` hold `Vec<Arc<CipherListView>>`, paying a refcount
bump per item per clone/filter. Switch to one shared `Arc<[CipherListView]>`, with
filter/search operating on indices. `VaultMessage::ListLoaded` simplifies to
`Result<Arc<[CipherListView]>, String>`. Bench before/after on the loadtest account; pair
with the lowercase-key cache for a bigger win.

---

## Magnify launcher

V1 ships behind `Ctrl+Shift+Space`
([views/magnify/](../crates/desktop/src/views/magnify/) +
[services/global_hotkey/](../crates/desktop/src/services/global_hotkey/mod.rs)).

- **Master enable/disable toggle** `[M]` — no off-switch today; the global hotkey
  registers unconditionally at startup. Add `Settings::magnify_enabled: bool` (default
  `true`), gate `services::global_hotkey::install_event_handler` on it, expose a switch in
  Integrations / Autotype settings. Toggling at runtime should re-register without restart
  (`GlobalHotKeyManager::unregister` on disable, re-call `install_event_handler` on enable
  — the `OnceLock<Sender>` must outlive the disable so re-enable doesn't leak a second
  manager).
- **Hotkey settings UI** `[M]` — V1 hardcodes `Ctrl+Shift+Space` (`Cmd+Shift+Space` on
  macOS). Add a binder; persist `Settings::magnify_hotkey` as a parsed
  `(Modifiers, Code)`; re-register on change via fresh `GlobalHotKeyManager::register`.
  The hardcoded `"Ctrl+C"` / `"Ctrl+⇧C"` strings in
  [chip helpers](../crates/desktop/src/views/magnify/widgets/chip.rs) and
  [row action pills](../crates/desktop/src/views/magnify/widgets/row.rs) should drive off
  the bound config so the displayed shortcut matches the binding.
- **Cursor-monitor centering — Linux** `[M]` `[blocked: Wayland needs DE-specific shim]` —
  Windows + macOS shipped via
  [services/cursor_monitor/](../crates/desktop/src/services/cursor_monitor/mod.rs)
  (`GetCursorPos` + `MonitorFromPoint` + `GetDpiForMonitor` on Windows;
  `NSEvent::mouseLocation` + `NSScreen` on macOS). Linux returns `None`. X11 is doable via
  `XQueryPointer` + Xinerama (new dep). Wayland has no standard global-cursor protocol —
  `wlr-layer-shell` or DE-specific.
- **Action menu per result** `[M]` — Tab cycles through copy options, or `Cmd+1/2/3`. Lets
  users pick non-default fields without leaving the keyboard.
- **Recent / smart ranking** `[M]` — sort by last-used; pin manually-favourited items to
  the top. Per-user usage tracking (small side-table in `data/`).
- **Multi-user search** `[M]` — currently single-user (active vault only). User badge per
  row + cross-user merge unlocks "search any unlocked account" UX.
- **Encapsulate `MagnifyView` state** `[M]`
  `[defer: next refactor that touches the struct]` — all fields are `pub(crate)` and the
  App-level handler reaches in directly. Move every field private, expose `set_query` /
  `set_selected` / `update_scroll_offset` / `clamp_selection`. ~10 fields all read+write —
  verbose for what it buys in isolation.
- **Autotype handoff** `[blocked: autotype framework]` — Enter on the highlighted result
  triggers autotype. V1 leaves Enter as a no-op for exactly this reason.

---

## Settings — runtime wiring

24 settings persist to `data/settings.json`; most are UI-only stubs that emit
`settings-toast-not-supported`.

### OS-glue (no SDK work)

- **Open at device login** `[M]` — platform autostart: registry on Windows, LaunchAgent on
  macOS, `.desktop` file on Linux.
- **Allow screenshots — branded screenshot decoy** `[M]` `[defer]` — V1 ships
  protection-only via
  [services/screenshot_protection/](../crates/desktop/src/services/screenshot_protection/mod.rs)
  with the "confirm window still visible" auto-revert dialog. Open follow-up: render a
  custom branded panel (flat color + logo + "Screen capture is disabled" text) inside a
  second window stacked just below the main window, so screenshots / RDP captures show
  that panel instead of the OS's blank exclusion. See chat 2026-05-10 for the design
  (two-window stack, z-order management). Magnify launcher excluded by design — too
  short-lived to matter.

### SDK integration

- **PIN unlock** `[M]` — per-user PIN state via `bitwarden_auth` + keystore wrapping.
- **Browser integration (+ fingerprint)** `[L]` — native-messaging host registration.
- **Autotype engine** `[L]` — Windows Premium feature, whole subsystem.

### Biometrics `[L]` `[plan: desktop_native study]`

Touch ID / Windows Hello and Linux authentication remain deferred. The
[Linux wallet and biometric unlock concept](security-concept.md#linux-wallet-and-biometric-unlock-concept-deferred)
distinguishes optional Secret Service wallet unlock from fresh biometric key
release. Acceptance testing with the actual biometric hardware is outstanding;
do not enable a biometric path based only on simulated authentication. Linux
wallet unlock can be evaluated separately without a sensor. Other platforms also
require the desktop_native study (under [Plan-first](#plan-first)).

---

## Polish & refactors

### Icon audit — migrate bootstrap-icons to bwi `[M]`

[`components::icons`](../crates/desktop/src/components/icons.rs) hosts two parallel fonts:
the Bootstrap Icons set (`FONT`, codepoints auto-generated at build time from
`assets/bootstrap-icons-*.css` via `build.rs::generate_bootstrap_icons`) and the
Bitwarden-branded `bwi-font` (`BWI_FONT`, codepoints transcribed by hand from
`clients/libs/angular/src/scss/bwicons/styles/style.scss`). The official client uses bwi
glyphs everywhere; mixing in Bootstrap icons drifts us off-brand. Sweep call sites and
prefer `BwiIcon` for anything bwi has a parity glyph for, downgrading Bootstrap to a
last-resort fallback for icons bwi genuinely lacks. Stretch: if the audit ends with zero
Bootstrap usages, drop the dep + font file + `FONT` / `Icon` types entirely. If a handful
remain, leave Bootstrap in place but document the residual cases.

Stretch follow-up — **automate bwi codepoint generation.** Once bwi is the canonical font,
extend `build.rs` to parse `clients/libs/angular/src/scss/bwicons/styles/style.scss` (or
the bwi `.css`, whichever is more stable) and emit a `bwi_icons_generated.rs` next to
`bootstrap_icons_generated.rs`. Mirror `FONT_VERSION` bookkeeping so the regen step is the
same recipe. Removes the manual codepoint table at
[icons.rs:78](../crates/desktop/src/components/icons.rs#L78) and prevents drift when
upstream adds glyphs.

### Replace `system_theme` crate with iced's built-in `theme_changes()` `[M]`

[`iced::system::theme_changes()`](../crates/desktop/src/theme/mod.rs) returns a
`Subscription<theme::Mode>` against the same OS signal we get from the
[`system_theme`](https://crates.io/crates/system-theme) crate today. Switching drops a
dep + a tokio observer thread and lets the message carry the new mode directly (current
`SystemMessage::ThemeChanged` is no-arg, re-reads via `system.get_scheme()`).

| Source  | `system_theme`                                            | iced via winit                               |
| ------- | --------------------------------------------------------- | -------------------------------------------- |
| Windows | WinRT `UISettings.ColorValuesChanged` (background thread) | Win32 `WM_SETTINGCHANGE` / `WM_THEMECHANGED` |
| macOS   | NSDistributedNotificationCenter                           | winit's NSApp observation                    |
| Linux   | XDG portal `org.freedesktop.appearance/color-scheme`      | winit's per-windowing-system reading         |

Verify before swapping:

1. **Synchronous initial value.** `ThemeState::new()` calls `system.get_scheme()`
   synchronously inside `App::new` so first paint renders right. iced's path needs
   `iced::system::theme()` (a `Task<theme::Mode>`) that resolves on the next message
   cycle. Test the dark-OS startup flicker is invisible.
2. **Drop `theme_contrast()` / `theme_accent()`.** winit's event is Light/Dark only; we
   don't use either today, so OK to drop — note in `decisions.md` for a future a11y pass.
3. **Linux Wayland coverage.** XDG portals work consistently. winit's Wayland reading has
   been spottier; test on at least one Wayland compositor.

Sketch: drop `system-theme` from `Cargo.toml`; replace
`ThemeState.system: Rc<system_theme::SystemTheme>` with the resolved mode; dispatch
`iced::system::theme()` from `App::new` (handle as
`SystemMessage::SystemThemeResolved(Mode)`); replace `theme_sub` with
`iced::system::theme_changes().map(...)` carrying the mode payload directly. Net: −1
crate, −1 thread, ~30 lines simpler.

### `UpdateCtx` filter leak `[M]` `[defer: N=3 sidebar filter]`

[`app/ctx.rs`](../crates/desktop/src/app/ctx.rs) carries `active_vault_filter` +
`active_send_filter` as separate fields; every view's `update()` receives both. Generator
landed without a sidebar filter (history is internal to the modal), so we're at N=2.
Whichever next view introduces a sidebar-driven filter tips this to N=3 and the right move
becomes obvious — either (a) collapse to a single `sidebar: &SidebarState` field, or (b)
push-based: App dispatches `FilterChanged { filter }` to the affected view on sidebar
clicks instead of threading the current filter through every cycle.

### Shared list-view state primitive `[L]` `[defer: 3rd list-style screen]`

View-side chrome (the header row, the wide-mode pane split + rounded-top container) now
lives in [`components::list_pane`](../crates/desktop/src/components/list_pane.rs); the
remaining duplication is state-side. `VaultView` and `SendView` both carry
`Selection { item, id, form, confirm_delete }` and `ItemCache { all, cached }` keyed by
`UserId`, plus `recompute_filtered` / `filter_items` / search query and reimplement
`apply_filter` / `reset` / `remove_user_items` / `focus_search_task`. Two implementations
is still a coincidence; three is a pattern. When the third lands, evaluate extracting
`ListView<T, Form>` generic state + a `ListViewController` trait with `load_list_task` /
`full_item` / `save_item` / `delete_item`. Premature here — the shape would lock against
an unknown use case.

### Vault + Send event-handler dedup `[S]` `[defer: shared list-view primitive]`

[`vault/handler.rs`](../crates/desktop/src/views/vault/handler.rs) and
[`send/handler.rs`](../crates/desktop/src/views/send/handler.rs) dispatch four overlapping
event arms (`AccountSwitcher` / `ItemSaved` / `ItemDeleted` / `ClipboardCopyRequested`),
differing only in toast strings and which list-reload task they call. Folds for free into
the list-view primitive above via a shared `ListEvent<V>`; don't fix in isolation.

---

## Plan-first

These need a written plan and a recommendation before code lands.

### Integrate `desktop_native` from the old clients `[plan]`

The `clients/` git submodule contains `apps/desktop/desktop_native/` — pure-Rust crates
the Electron app calls via NAPI. Two modules are directly useful:

- **`desktop_core::biometric_v2`**
  ([clients/apps/desktop/desktop_native/core/src/biometric_v2/](../clients/apps/desktop/desktop_native/core/src/biometric_v2))
  — per-platform biometric unlock (Windows Hello, Touch ID, polkit); modules `windows.rs`,
  `windows_focus.rs`, `linux.rs`, `unimplemented.rs` (macOS TBD). Backs the "Unlock with
  Windows Hello" button currently stubbed to a "not yet supported" toast.
- **`desktop_core::ssh_agent`**
  ([clients/apps/desktop/desktop_native/core/src/ssh_agent/](../clients/apps/desktop/desktop_native/core/src/ssh_agent))
  — serves keys from the unlocked vault via `russh`. Depends on `bitwarden-russh` (same
  pin as the SDK). See also the standalone `ssh_agent` binary at
  `clients/apps/desktop/desktop_native/ssh_agent/`.

Investigation questions:

1. **Consumable as a direct Cargo dep** from our workspace, or tangled with the NAPI
   layer? `lib.rs` has module guards but `#[global_allocator] ZeroAlloc` is set at crate
   root and may conflict.
2. **License compatibility.** Is `desktop_core` under the GPLv3 that applies to
   `apps/desktop/`, or split out under Bitwarden SDK's Apache/GPL dual? GPL-only would
   make our binary GPL too.
3. **Integration path:** (a) direct path dep on the submodule, (b) git-pinned rev, (c)
   fork and vendor, (d) extract modules into our workspace.
4. **Biometric v2 API surface.** Windows Hello requirements (hwnd? UWP runtime?). Does it
   block the UI thread? Async?
5. **SSH agent lifecycle.** In-process tokio task or out-of-process (Electron-style)?
   Lock/unlock behaviour? Key plumbing from the SDK?
6. **Platform coverage gaps.** macOS Touch ID is `unimplemented.rs`.

Deliverable: a plan document recommending (integration path) × (biometrics in scope?) ×
(ssh_agent in scope?).

### Toast API review `[plan]`

Before more call sites accumulate (unlock failure, copy-to-clipboard, sync errors),
validate the API:

- **`Toast::info/success/warning/error(body, title)` surface** — the `title: Option<&str>`
  slot is a pain point; default callers pass `None`. Alternatives: builder
  (`Toast::warning("body").with_title("Title")`), default+titled overloads, or drop title
  entirely.
- **`Toast` schema** — today `{ title, body, status }`. Future callers may want action
  button (Undo), icon override, sticky flag, custom timeout. Plan the schema before ten
  call sites exist.
- Document the final design in `decisions.md`.

"Toast emission from anywhere" is already solved: views call `Outcome::toast(Toast::*)`
and the dispatch helper routes it directly to `push_toast()` — no per-view
`ToastRequested` event variant needed.

---

## Blocked on Bitwarden SDK

Park; revisit when the named SDK API lands. Each entry names the upstream function or
crate it's waiting on so you can grep `sdk-internal` for status.

### Export — wire org vault export when SDK lands

The Export modal ([views/export/mod.rs](../crates/desktop/src/views/export/mod.rs)) lets
the user pick "My vault" or one of their orgs from the source-vault dropdown; personal
exports go through `ExporterClient::export_vault`, but the org branch short-circuits at
[`ClientManager::export_organization_vault`](../crates/desktop/src/services/sdk/mod.rs)
with `Err("Organization vault export isn't implemented in the SDK yet")`. Surfaces as the
generic "Couldn't export the vault" toast.

The pinned `bitwarden-exporters` rev's `export_organization_vault` in
[`crates/bitwarden-exporters/src/export.rs`](https://github.com/bitwarden/sdk-internal/blob/main/crates/bitwarden-exporters/src/export.rs)
is `todo!()` (would panic if called). When it lands: replace the early-return in
`ClientManager::export_organization_vault` with a real call. The signature already matches
what the SDK needs (uid + org id + format) — body needs to load the org's encrypted
ciphers (filter the cipher repo by `organization_id`) and the SDK-shape
`bitwarden_collections::Collection` for the org, then hand both to
`ExporterClient::export_organization_vault`. We don't have a `Repository<Collection>`
today; the org-collection load may need a parallel SDK addition.

### Import — wire to SDK importers when the crate exists

The Import modal ([views/import/mod.rs](../crates/desktop/src/views/import/mod.rs)) is
fully chrome — vault/folder/collection dropdowns seeded from the active user, format
picker populated from the upstream `featuredImportOptions` + `regularImportOptions` lists,
paste textarea. Submit fires `ImportEvent::Unimplemented` and a "coming soon" toast;
nothing is parsed, nothing is written.

The SDK ships
[`bitwarden-exporters`](https://github.com/bitwarden/sdk-internal/tree/main/crates/bitwarden-exporters)
(which we use for export) but no parallel `bitwarden-importers` crate yet. The only
import-side function is `ExporterClient::import_cxf` — Apple-only Credential Exchange
Format. Every other parser still lives in upstream Electron's TypeScript
([clients/libs/importer/](../clients/libs/importer/)); a Rust port isn't on a published
roadmap.

When a `bitwarden-importers` crate (or equivalent SDK API) lands: replace the
`Unimplemented` path in
[app/handlers/import.rs](../crates/desktop/src/app/handlers/import.rs) with a real call
that takes the chosen `ImportFormat` + file bytes (or paste contents) + destination vault
/ folder / collection. Handler shape mirrors the export wiring (Task::perform → completion
message → success/error toast → close on success). The "Choose file" button — currently
also stubbed via `Unimplemented` — should use
`rfd::AsyncFileDialog::new().pick_file().await` (already a dep — see
[app/handlers/export.rs](../crates/desktop/src/app/handlers/export.rs) for the save-side
recipe).

### Sync — wire to real SDK

`ClientManager::sync(uid)`
([services/sdk/mod.rs](../crates/desktop/src/services/sdk/mod.rs)) returns `Ok(())`
without doing anything — File → Sync now toasts "Vault synced" but no network round-trip
happens. The whole vault loads from SQLite at startup (`fake-data` seeds it), so there's
no remote to sync against today. SDK 4 exposes sync entry points, but full vault handler coverage needs verification.
Integrate those APIs to pull ciphers / folders /
collections / orgs from the server, persist into the SDK's repos, then return so the toast
reflects reality. Subscription-side: the official client also runs sync on a timer
(default ~5 min) and on app focus — both can layer on once the manual call works.

---

## Blocked / waiting on other upstream

Park; revisit when the gate lifts. Bitwarden SDK gates have their own section above; this
bucket is everything else (iced, Bitwarden's Electron desktop_native, third-party crates).

### SSH agent — Unix implementation available

The opt-in agent now uses the active unlocked SDK session and OpenSSH's Unix
socket protocol, with bounded transport, per-signature approval, verified session
binding and revocation on lock/account/sync transitions. It does not wait for the
Electron V2 runtime. See [SSH agent setup, security boundary and remaining work](ssh-agent-requirements.md).

Remaining: Windows named pipes, protected-item master-password reprompt, broader
macOS validation and independently reviewed generic/Git-signing support if needed.

### Drop `ShellScope` if iced fixes `Stack`'s capture leak

`iced::widget::Stack::update` short-circuits between its children on
`shell.is_event_captured()`, but the shell is shared across the whole event pass — sibling
captures leak. We work around it by wrapping `field_frame`'s output in
[`components::shell_scope::ShellScope`](../crates/desktop/src/components/shell_scope.rs)
(full write-up:
[architecture.md → Shell Capture Isolation](./architecture.md#shell-capture-isolation-shellscope)).
The minimal upstream patch is `was_captured_before = shell.is_event_captured()` once at
the top of `Stack::update` and short-circuit only on captures that happen during the loop.
If accepted upstream, drop `ShellScope` + the wrap call in `field_frame`.

### Replace `virtual_list` with upstream when iced lands native virtualization

iced has an open issue for first-class virtualized list support:
<https://github.com/iced-rs/iced/issues/160>, targeted at iced 1.0. When a native widget
lands, [components/virtual_list.rs](../crates/desktop/src/components/virtual_list.rs) can
become a thin adapter — or be deleted outright if the upstream API fits our usage
directly.

### Scrollbar minimum thumb height

With 20k+ items in a virtualized list, iced's scroll thumb shrinks to a hardcoded 2px
minimum and becomes ungrabbable. The constant is in `iced_widget-0.15/src/scrollable.rs`:

```rust
let scroller_height = (scrollbar_bounds.height * ratio).max(2.0);
```

Upstream PR shape: add `min_scroller_length: f32` (default `2.0`) on `Scrollbar`, builder
method, replace `.max(2.0)` with `.max(self.min_scroller_length)` in vertical + horizontal
branches. Workaround would be vendoring `scrollable.rs` (~2400 LOC) or an overlay thumb
beside `Scrollbar::hidden()` (~150–250 LOC). Wait on upstream.

### Tray support — iced PR #3021

iced [PR #3021](https://github.com/iced-rs/iced/pull/3021) adds native tray support,
targeting 1.0. If it lands on our pin, re-evaluate swapping our `tray-icon` dep for the
native path.

### Hot reloading

iced [PR #3000](https://github.com/iced-rs/iced/pull/3000) merged into master (June 2025).
`hot` feature flag + `subsecond` / `cargo-hot`. Available on 0.15. Worth trying on a
feature branch; no core architecture changes required.

### SVG logo antialiasing

iced's `resvg` rasterizer doesn't match browser quality. Workaround: pre-rasterized PNG
with 2× / 3× variants. Otherwise wait for upstream resvg improvements.
