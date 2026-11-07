# Decisions

## Framework: Iced 0.15

**Decision**: Use Iced as the GUI framework, tracking its pre-release 0.15 via git pin.

**Alternatives considered**: Dioxus (webview-based desktop mode, custom renderer still
maturing), Slint (GPL/commercial license concern).

**Rationale**: Pure Rust, no webview, Elm architecture ensures repaints only on state
change, ~25k GitHub stars, actively maintained, cross-platform. 0.15 brings a cleaner
window/daemon API and builder methods on `Border` / `container::Style` that remove much of
the `..Default::default()` boilerplate.

## Renderer and image formats

**Decision**: Use tiny-skia CPU rendering exclusively. Remove the GPU Cargo
feature, graphics-backend discovery/cache, and hardware-acceleration controls.

**Why**: The current form and virtualized-list UI works with CPU rendering.
The additional GPU stack increases binary size without a demonstrated benefit
for this application. Existing settings files remain readable; obsolete GPU
fields are ignored and omitted on the next save.

**Image formats**: Disable image defaults and select the existing codecs explicitly,
excluding OpenEXR and TIFF. Use Iced's `image-without-codecs` feature so it cannot
restore the default codecs indirectly. SVG remains available for the embedded
vector assets. Other existing image formats remain supported.

## Surface separation without painted shadows

**Decision**: Use borders, background colors and modal backdrops instead of painted
shadows on dialogs, menus, cards, generator history, toast notifications and picker
panels. Dialog and menu transitions were subsequently removed (see below).

**Why**: The pinned tiny-skia renderer regenerates the full shadow pixel buffer
when a shadowed quad is redrawn, including for small intersecting damage regions.
A release-mode headless control at 2× scale took about 5.6 ms without a shadow
and 68.9 ms with one. These are offscreen full-frame render timings, not end-to-end
input latency. Removing shadows avoids that work without requiring GPU rendering.
The settings dialog subsequently measured about 18.6 ms per warm full frame,
down from 58.8 ms under the same 1024×768 logical, 2× raster test. Input latency
and the existing animation/focus delays require separate measurement.

## Crate Name: bitwarden-desktop-next

**Decision**: Named `bitwarden-desktop-next` to avoid conflict with existing
"bitwarden-lite" project.

## Workspace Structure

**Decision**: Organize as a Cargo workspace with the desktop app at `crates/desktop/`,
plus `bitwarden_license/*` and `tools/*` as additional members.
`default-members = ["crates/*", "bitwarden_license/*"]`.

**Rationale**: Mirrors the pattern used in other Bitwarden Rust projects. Separates the
main application from licensed code and developer tooling. `default-members` excludes
`tools/*` so `cargo run` / `cargo build` target only the app and licensed code, not the
developer tooling; `tools/*` are built explicitly via `cargo run -p fake-data` /
`cargo run --bin packager`.

## UI Only (No Business Logic)

**Decision**: This app is stub UI only. All crypto, API, authentication, and vault
operations will come from a separate SDK (`PasswordManagerClient`).

**Rationale**: Separation of concerns. The SDK handles security-sensitive operations; this
project handles presentation.

## Multi-User State Model

**Decision**: App state uses per-user keyed data (active_user + items keyed by UserId +
per-user `PasswordManagerClient` in `ClientManager`).

**Rationale**: Makes future SDK integration a clean swap. Multiple accounts can be
available simultaneously with one active at a time.

## Startup: Lazy Data Load + Spinner

**Decision**: `App::new` returns fast with
`client_manager: Arc::new(ClientManager::empty())` and `screen: Screen::Loading`. The 24
MB JSON parse runs on tokio's blocking pool via `spawn_blocking`; when it resolves,
`SystemMessage::ClientManagerLoaded(Arc<ClientManager>)` swaps the Arc and transitions to
`Screen::Login`.

**Previous approach**: `ClientManager::load()` ran synchronously in `App::new`, blocking
the window's first frame for hundreds of milliseconds (up to ~1-2 s on the loadtest
account).

**Why changed**: Iced renders the first frame only after `App::new` returns. Blocking
inside it means a blank screen while the vault JSON parses. Deferring to a background task
with a spinner gives immediate window + spinner feedback.

**Implementation**:

- `ClientManager::empty()` returns a zero-user manager. Existing accessors (`user_ids`,
  `is_unlocked`, `has_users`, `email`, etc.) all behave correctly on the empty `HashMap`,
  so no `Option<Arc<ClientManager>>` plumbing is needed across call sites.
- `Task::perform` wraps `tokio::task::spawn_blocking(ClientManager::load)` so the JSON
  parse is scheduled on tokio's dedicated blocking thread pool, not on an async worker.
  Iced's `tokio` feature already initializes a multi-threaded runtime with
  `rt-multi-thread` + `time`, so our crate only needs the `rt` feature of tokio for
  `spawn_blocking`.
- `SystemMessage::ClientManagerLoaded` arrives, handler swaps the Arc, computes
  `active_user` from `user_ids().next()`, calls
  `login_view.show_unlock_for(active_user, &client_manager)`, sets
  `screen = Screen::Login`.

**Spinner component** (`components/spinner.rs`): self-animating 8-dot ring widget.
Intercepts `Event::Window(window::Event::RedrawRequested(_))` in `Widget::update` and
calls `shell.request_redraw()` to ask for the display's next frame. No app-level
subscription, no dummy animation message. Same pattern as the toast overlay, which uses
the explicit-instant variant `request_redraw_at` because its fade animation has a known
cadence.

**Per-user lazy cipher parsing (landed via SQLite)**: ciphers now live in per-user
`<user_id>.sqlite` files under `data/` and are read through the SDK state registry only
when `list_ciphers` / `full_cipher` runs. The original "parse all ciphers at startup"
bottleneck is gone.

## Unlock in-flight indicator

**Decision**: During the unlock task, the primary button shows a spinner (same padded
height as the text), the password/PIN input is read-only, and the alternate-method + Log
out buttons stay visible but inert (no `on_press`). `LoginView.unlock_in_progress: bool`
drives the state.

**Rationale**: Nothing disappears between "Unlock clicked" and "unlock completed" — the
buttons stay in place but are clearly unavailable. The spinner sits inside the primary
button so there's no layout shift. Re-entry guards inside `LoginMessage::Unlock` prevent
Enter-spam from queuing multiple unlocks.

**Flag lifecycle**: set true in `LoginMessage::Unlock` just before `Task::perform`;
cleared in `UnlockCompleted` (both Ok and Err), `SwitchUnlockMethod`,
`reset_to_email_entry`, `show_unlock_for` (covers account switches while a task is in
flight).

## Compositional MVU

**Decision**: Each view owns its local state and async work, and returns an
[`Outcome<V>`](./architecture.md#update-outcome) (mutually-exclusive `None` / `Task(t)` /
`Event(e)`) from its `update()` method. App is a thin router that calls
`Outcome::dispatch` to lift the view's local message type and route events through
`handle_{view}_event`. See [architecture.md](./architecture.md) → "Compositional MVU" /
"Update Outcome" for the implementation shape, message flow, and "How to Add a New View"
recipe.

**Previous approach**: Sub-views returned `Vec<Action>` with imperative requests; App
owned every `Task::perform` call; async completions were orphan top-level `Message`
variants.

**Why changed**: top-level `Message` had grown to 13 variants; `App::update()` was 220
lines mixing routing, async dispatch, toast pushes, theme sync, and window chrome. Every
new feature touched shared files. `app.rs` became the single-team bottleneck. After the
refactor: `Message` shrank to 6, adding a new screen costs a handful of mechanical lines
in `app.rs` (see [architecture.md](./architecture.md) → "What this recipe costs in
`app.rs`").

**Prior art**: Halloy —
[investigation/halloy/src/buffer.rs:247](../investigation/halloy/src/buffer.rs);
cosmic-settings —
[investigation/cosmic-settings/cosmic-settings/src/pages/mod.rs:113](../investigation/cosmic-settings/cosmic-settings/src/pages/mod.rs);
iced-guide —
[investigation/iced-guide/src/app_structure/composition.md](../investigation/iced-guide/src/app_structure/composition.md).

**Kill-switch criteria**: revisit this pattern if (a) sub-views need to nest multiple
levels deep — the `Task::map` chain gets awkward past two levels, or (b) we need a
middleware router (e.g. undo/redo).

## Colors: Picker Over SCSS

**Decision**: Use color-picked values from the actual running Bitwarden app, not SCSS/CSS
variable values.

**Rationale**: The SCSS values in `variables.scss` and Tailwind CSS vars don't match what
the app actually renders. For example, `backgroundColor` is `#1f242e` in SCSS but the
actual window background is `#070b18`. The SCSS has three background levels and it's
unclear which applies where. Always verify with a color picker (Win+Shift+C with
PowerToys).

## Menus: Native on macOS, Custom-Drawn Elsewhere

**Decision**: Use `muda` for native menus on macOS only. Draw the menu bar inside iced on
Windows and Linux.

**Previous approach**: Used `muda::init_for_hwnd()` on Windows for native Win32 menus.

**Why changed**: Native Win32 menus create a 1px transparent gap between the menu bar and
the iced client area (DWM compositor border). This is a known muda issue (#123, fixed for
Tauri's webview but not for raw tiny-skia rendering). Drawing the menu ourselves
eliminates the gap entirely.

**macOS**: Keeps native menus via `init_for_nsapp()` because macOS users expect the system
menu bar at the top of the screen.

**Linux**: Custom-drawn (same as Windows) to avoid GTK dependencies.

## Icons: Bootstrap Icons (Self-Bundled)

**Decision**: Bundle Bootstrap Icons TTF font directly and auto-generate Rust constants
via `build.rs`.

**Alternatives considered**: `iced_fonts` crate (adds a dependency for something trivial),
`iced_aw` icons (claimed Icon enum but didn't have one), Font Awesome, Nerd Fonts,
individual SVGs per icon.

**Rationale**: Bootstrap Icons is what the official Bitwarden app uses (`bwi-*` prefixed
classes). Doing it ourselves means zero extra dependencies and full control over the
version.

**WOFF→TTF**: The Bootstrap Icons release only ships WOFF/WOFF2, but iced's `fontdb` only
supports TTF/OTF. We used the `wuff` crate (pure Rust WOFF decoder) once to convert, saved
the result in `assets/`, then removed `wuff` from dependencies.

## Font: Inter 18pt Static

**Decision**: Bundle static Inter 18pt font files (`Inter_18pt-Medium.ttf` +
`Inter_18pt-Bold.ttf`), default weight Medium (500). Family name: `"Inter 18pt"`.
Constants `APP_FONT` / `APP_FONT_BOLD` in `main.rs`.

**Rationale**: The "18pt" optical size variant has an open single-storey "g" matching the
Bitwarden app better than variable Inter. Medium (500) matches the app's visual density.
Static files avoid the optical-size axis complexity.

## Font Sizes: Consolidated to 5

**Decision**: Use exactly 5 font sizes: 12, 14, 16, 18, 28.

**Rationale**: Fewer sizes create a more consistent visual hierarchy. 12 captions, 14
body/buttons, 16 emphasis, 18 section headers, 28 page titles.

## Button Style: Pill-Shaped (RADIUS_PILL = 20)

**Decision**: Use 20px border-radius (`RADIUS_PILL` constant) for buttons.

**Rationale**: Visual inspection of the actual rendered app shows more rounded buttons
than the Tailwind `tw-rounded-xl` (12px) suggests.

## Theme: Custom `AppTheme`

**Decision**: Custom `AppTheme` struct implementing `iced::theme::Base` + widget `Catalog`
traits. Dark/light palettes in separate files.

**Status**: Complete. Light theme is the default. `ThemePreference::Light` hardcoded at
startup (no settings persistence layer yet). Light palette colors picked from the 2025
Figma mockup. `AppTheme.name: &'static str` (not `String`) so clones are pointer copies,
since iced calls `App::theme()` frequently.

**Rationale**: iced's `application()` / `daemon()` is generic over Theme. Custom theme
gives `&AppTheme` in all `.style()` closures with direct access to 25 semantic color
tokens. No thread_local or registry needed. All colors (including `scrollbar_thumb` used
by the default `scrollable` catalog impl) live in `AppColors`.

**Per-window theme variants**: iced has no per-window `Theme` slot —
`App::theme(window_id)` returns one `AppTheme` for _every_ widget tree in that window.
When one window needs a different `iced::theme::Base::base()` output (e.g. the Magnify
launcher's transparent OS-window background so pixels outside its rounded container stay
see-through, while the main window keeps its opaque surface), the dispatch lives at the
App level: `App::theme()` matches on `WindowKind` and returns a tweaked clone via a small
builder method on `AppTheme`. The current escape hatch is
[`AppTheme::with_transparent_background()`](../crates/desktop/src/theme/mod.rs) — it flips
a `transparent_background: bool` field that `Base::base()` reads. Add further variants
(each its own `with_*()` builder + matching `bool` field) the same way; keep the
per-window switch in `App::theme()` so the _colours_ stay shared but window-scoped chrome
can diverge. `AppTheme` is cloned every time iced calls `App::theme(window_id)`, so any
new field must stay cheap (`Copy` or `&'static`) — no `String`s, no `Vec`s.

## Overlays: DropDown Widget (replaces `stack!` pattern)

**Decision**: Use a local fork of iced_aw's `DropDown` widget (`components/drop_down.rs`)
with custom alignment variants (`BelowLeft`, `BelowRight`, `AboveRight`).

**Previous approach**: `iced::widget::stack!` with manual backdrops for
click-outside-to-close.

**Why changed**: Stack-based overlays couldn't cover the title bar. DropDown uses iced's
native overlay system which renders at the window level, handles click-outside
automatically, and positions relative to the trigger.

## Window-Level Overlays Composed at App Root

**Decision**: Overlays meant to cover the whole window (bottom sheet, sub-modals,
in-view confirmations) are composed as siblings of the root column at the App level via
`iced::widget::stack`, not nested inside the screen they belong to. Sub-views push them
through `View::overlays(&RenderCtx) -> Vec<Element>`; `App::collect_overlays` extends
the window-level stack with each view's contribution.

**Why**: `iced::widget::stack` doesn't cull or clip — every child fully lays out at the
same bounds. Stacking a heavy detail subtree inside a content area meant iced walked both
the list AND the detail tree on every frame, even when one was visually covered. Hoisting
to the App level let the underlying screen take a cheaper exclusive branch (e.g. vault
narrow mode renders only the list, no `pane_grid`), so the total per-frame layout work
dropped despite the sheet being conceptually "more on top".

**How to apply**: Any future overlay that conceptually covers the entire window — modals,
sheets, full-screen confirmations — should expose an `Option<Element>` accessor on the
owning view and be stacked at the App level. If the overlay only covers a sub-region,
prefer an exclusive branch in the view's own `view()` over stacking.

## Responsive Layout Pattern

**Decision**: For layouts that change based on window size: subscribe to
`window::Event::Resized` via `iced::event::listen_with`, store `iced::Size` per
`WindowInfo` in `App.windows`, thread the width into the view function, and branch on a
`pub const ..._BREAKPOINT_PX: f32` constant near the view module.

**Rationale**: Iced provides no other way to access window size from a view. `view()`
re-runs every frame, so the layout swaps automatically on resize without any extra
plumbing. Constants keep the breakpoint editable in one place.

**First consumer**: vault detail. `SHEET_BREAKPOINT_PX` in `views/vault/mod.rs` switches
between a side `pane_grid` split (wide) and an App-level bottom sheet (narrow).

## Self-Animating Widgets via RedrawRequested

**Decision**: Widgets that need continuous animation (spinner, toast fade) drive their own
redraws by intercepting `Event::Window(window::Event::RedrawRequested(now))` in
`Widget::update` and calling `shell.request_redraw()` or
`shell.request_redraw_at(now + delta)`. No app-level subscription or dummy animation
message.

- `request_redraw()` — next display frame; simplest; used by the spinner.
- `request_redraw_at(instant)` — explicit schedule, caps the rate regardless of display
  Hz. Used by the toast overlay because its fade animation has a known cadence and
  hover-pause needs precise `Instant::now()` timing.

**Prior art**: The toast overlay predates the spinner and established the pattern.

**Rationale**: Subscriptions create every-tick updates that run the whole app `update`
cycle even when nothing changed; the redraw-request path only redraws the widget tree
without running `update`, so the cost stays bounded to the widget that needs the
animation.

## Immediate dialog and menu interaction

**Decision**: Open and close dialogs, sheets and dropdown menus immediately. The
generator tab indicator also switches directly. Remove the 180 ms dialog transitions,
80 ms menu transitions, 160 ms tab transition, 220 ms deferred focus and the global
animation watermark/frame subscription. Keep the historical `FadeInOut` API as a
boolean visibility adapter; its cleanup task completes without sleeping.

**Why**: These decorative transitions imposed waiting and repeated layout/redraws
on interactive controls. Immediate changes improve responsiveness with the existing
CPU renderer and require no GPU dependency. Status indicators such as loading
spinners and toast expiry keep their local redraw behavior.

## Component Library

**Decision**: Button convenience constructors (`buttons::primary(content)`) that take
`impl Into<Element>` and return `Button` with style pre-applied. Style functions are
internal. Signature aligns with iced's `button()` for familiarity.

**`buttons::ghost` signature**: takes `(content, is_active, active_bg, hover_bg, radius)`.
`ghost_icon(content, hover_bg)` is a shorthand for non-active icon-only ghost buttons with
`RADIUS_SM`.

## Native Menu Unification

**Decision**: A single `MenuTree`, built by `services::menu::menu_tree()` and cached on
`App::menu`, drives both the custom title-bar dropdown and the native muda menu.
`NativeMenuHandle` bridges muda events.

**Rationale**: Avoids duplicating menu structure. `Shortcut::to_accelerator()` reuses
existing `display()` for muda compatibility. `DEV_BOTH_MENUS=1` shows both simultaneously
for comparison.

**Why runtime, not static**: an earlier `MENUS: &'static [(&str, &[MenuEntry])]` const
stored Fluent keys as bare `&str` and resolved them at render time via `i18n::lookup()`.
That bypassed `i18n-embed-fl`'s compile-time validation (a typo in a menu key compiled
fine and rendered the raw key string) and forced `tools/i18n-unused` to carry a second
needle (`E(`) plus an allowlist of top-level menu titles. Building the tree at runtime
lets every label flow through `fl!()` directly; the linter is back to a single needle and
the OS menu actually picks up language changes via `App::rebuild_menu()` instead of
freezing labels at startup.

**Dynamic submenus**: File → Lock vault and File → Log out gain per-account entries via
`MenuChildren::Dynamic(DynamicSubmenu)`. `DynamicSubmenu::resolve(accounts)` expands the
parent into one entry per unlocked / known account at render (and click-dispatch) time.
Renderers walk `MenuEntry::effective_children(accounts)` so the static vs. dynamic split
stays out of their loops; native muda additionally tracks `(Submenu, DynamicSubmenu)`
pairs so `NativeMenuHandle::sync_dynamic` can refill them on lock / unlock / login /
logout.

**MenuState shape**: 3 bools (`is_locked`, `has_accounts`, `has_lockable_accounts`). An
earlier `has_authenticated_accounts` field was indistinguishable from `has_accounts` in
practice — collapsed.

## Widget ID Constants for Cross-File Focus Operations

**Decision**: Widget IDs referenced from more than one file live as `pub const` in the
module that owns the widget. Example:
`SEARCH_ID: widget::Id = widget::Id::new("vault-search")` in
`views/vault/widgets/search_bar.rs`, consumed by `handle_menu_action` for the Cmd/Ctrl-F
focus operation.

**Rationale**: String literals repeated across files are a silent-breakage failure mode —
one gets updated, the others don't, the focus silently stops working. A single `pub const`
forces all callers to import the same identifier.

## SDK Integration: Generator Binary + Per-User SQLite

**Decision**: Build a standalone `tools/fake-data` binary that drives the real Bitwarden
SDK (`make_register_keys`, `initialize_user_crypto`, `vault().ciphers().encrypt`,
`vault().folders().encrypt`) to populate `<workspace-root>/data/` with one SQLite database
per user (filename = user UUID) plus a `mock.json` file holding non-cipher user metadata
(email, KDF, encrypted user key, unlock methods). The desktop app discovers users by
listing `data/*.sqlite`, pairs each with its `mock.json` entry, and opens the DB via
`StateRegistry::new_with_db(Sqlite { db_name, folder_path }, bitwarden_pm::migrations::get_sdk_managed_migrations()).await`.
`ClientManager` then owns one `PasswordManagerClient` per user whose `Cipher`/`Folder`
repositories read from SQLite.

**Alternatives considered**: (1) Hand-roll fake `Cipher` structs and skip the SDK —
rejected: the app would never exercise real crypto paths. (2) Keep the old single
`mock-vault.json` + `MemoryRepo` design — rejected once the SDK grew SQLite support;
per-user DBs match production and avoid the 24 MB parse at startup. (3) Embed the DB files
in the binary — rejected: `include_bytes!`ing SQLite files bloats the binary and forbids
writes.

**Rationale**: Unlock is a real `crypto().initialize_user_crypto(...)` call against real
encrypted data; `list_ciphers` is a real `decrypt_list` round-trip. Data lives in real
SDK-managed storage, not a hand-rolled `MemoryRepo`, so any future change to the SDK's
persistence layer is automatically exercised in dev.

**Caveats**: `LocalUserDataKeyState` isn't in `get_sdk_managed_migrations()` but
`initialize_user_crypto` writes to it, so a narrow `MemoryRepo<LocalUserDataKeyState>` is
registered client-managed per user. The registry is initialized with the SDK's
SQLite configuration before passing it to `PasswordManagerClient::load_from_state`.
SDK 4 replaces the old two-step registry initialization with `new_with_db`.
The generator also persists the email required by SDK 4's `SaveStateData`.

**Dev passwords** (documented in `services/sdk/loader.rs`): `alice@example.com` /
`password`, `alice@acmecorp.com` / `123456`, `loadtest@example.com` / `loadtest` (20 k
ciphers).

**Shared schema synchronization**: `MockVaultFile` / `MockUser` / `UnlockMethodsCfg`
structs are defined independently in `crates/desktop/src/services/sdk/loader.rs` and
`tools/fake-data/src/main.rs`, with comments telling the reader to keep the field sets in
sync. A shared schema crate (`tools/mock-schema`) is a future refactor — current
synchronization is manual but easy to spot in review.

## Cipher UI Types: SDK Directly

**Decision**: The vault view, item list, and detail pane consume
`bitwarden_vault::CipherListView` and `CipherView` directly. No intermediate
`state::CipherItem` DTO. `VaultView.items.cached` is `Vec<Arc<CipherListView>>`.

**Rationale**: Once the SDK pipeline is in place, keeping a mirror DTO is pure overhead —
every new SDK field would need duplicating.

**`Arc<CipherListView>` wrapping**: `CipherListView` doesn't derive `Clone`, and iced
`Message`s must be `Clone`. Wrapping in `Arc` gives cheap clones for message dispatch and
filter recomputation. `CipherView` is smaller and does derive `Clone`, so it's passed as
`Box<CipherView>` in messages and stored as `Option<CipherView>` on `VaultView`.

**Stale-load guard**: When the user clicks item A, then item B before A's decrypt
completes, both `DetailLoaded` messages arrive but only B should apply. The guard is
`self.selection.id == view.id` in `VaultView::update` for the `DetailLoaded` arm.

## Toast Notifications: Custom Widget + Unified Cells

**Decision**: `components::toast::Manager` is a custom `Widget` that wraps the app's main
content and overlays a vertical toast stack via iced's native `Overlay` trait. Animation
state (fade in/out, countdown progress, hover-pause) lives in two places:

1. **Persistent timer state** in the widget tree (`Vec<Option<ToastTimer>>`): creation
   timestamp, optional dismissal-start timestamp, hovered bool. Survives across `view()`
   rebuilds because iced's tree state outlives `Manager` instances.
2. **Per-frame animation values** in `Vec<Rc<Cell<ToastVisuals>>>` on `Manager`, paired
   1:1 with the toast list. `ToastVisuals { alpha, progress }` is `Copy`. Refreshed on
   every `window::Event::RedrawRequested` tick from the timer state.

Style closures on the row widgets capture an `Rc<Cell<ToastVisuals>>` at construction time
and read it at draw time. Updating the cell from inside the overlay's `update()` drives a
re-render without layout invalidation.

**Auto-dismiss is two-phase**: timer expires → `dismissing = Some(now)` → fade-out runs
over 150ms → once complete, `on_close(idx)` is published and App removes the toast. Manual
close bypasses the fade. Hovering pins `alpha = MAX_ALPHA` + `progress = 1.0`.

**Progress bar rounding**: the fill quad's bottom-left corner rounds to `RADIUS_MD`
(capped to half the fill width for narrow slivers), bottom-right rounds only when the fill
reaches the toast's right edge — so the fill follows the toast's rounded corners without
hard-clipping. The fill is inset 1 px on left/right/bottom so it floats inside the
corners.

## Error Messages in Toasts: Sanitize SDK Output

**Decision**: Don't echo `e.to_string()` from SDK errors directly into toast bodies. Log
the raw error at `warn!`/`error!`, show the user a short sanitized message.

**Rationale**: SDK `Display` impls can surface internal crypto state, key-derivation
iteration counts, or module paths in error text — not appropriate for a UI toast. Users
see "Check your master password and try again" for unlock failures; the raw SDK error
lives in the tracing log for developers.

## `#[expect(dead_code)]` Over `#[allow(dead_code)]`

**Decision**: Use `#[expect(lint)]` rather than `#[allow(lint)]` for deliberate
suppressions. The `expect` form emits a warning when the suppression becomes unnecessary,
catching stale allowances automatically.

**Exception**: `components/icons.rs` uses `#![allow(dead_code)]` at file scope because the
`bootstrap_icons_generated.rs` include generates hundreds of `Icon` constants, many
unused. `expect` at file scope would not fire correctly since _some_ constants are used.
Alternative: `#[expect(dead_code)]` on each unused constant is not feasible in generated
code.

## Workspace-Level Shared Dependencies

**Decision**: Dependencies used by more than one workspace member (`async-trait`, `serde`,
`serde_json`, `tokio`, `tracing`, `tracing-subscriber`, `uuid`, and all five `bitwarden-*`
git-pinned crates) are declared once under `[workspace.dependencies]` in the root
`Cargo.toml` and consumed as `{ workspace = true }`.

**Rationale**: The `bitwarden-*` git revision is especially important to keep in lockstep
— if `tools/fake-data` encrypts with one revision and `crates/desktop` decrypts with
another, the JSON won't round-trip.

**Crate-local deps stay local**: iced, iced_aw, image, muda, system-theme (desktop only),
chrono (fake-data only), cargo-packager (packager only).

## Packaging: cargo-packager

**Decision**: Use `cargo-packager` for distribution packaging (.app, .dmg, .msi).
Configuration in `Packager.toml` at workspace root. A thin wrapper crate at
`tools/packager/` invokes `cargo_packager::cli::run`.

**Previous approach**: `winresource` build script for Windows exe icon, `embed_plist` for
macOS bundle metadata.

**Why changed**: `cargo-packager` handles all platform packaging concerns in one place —
bundle structure, installer creation, icon embedding, code signing — removing the
per-platform build-script hacks.

## Cipher Save: Local-Only (Skips Server PUT)

**Decision**: The edit form's Save path encrypts via `CiphersClient::encrypt(CipherView)`
and writes directly to the local `Repository<Cipher>`, skipping `CiphersClient::edit()`
(which also PUTs to the server API).

**Rationale**: The app is a UI stub with no backend — the configured `api_url` points at
an unused `localhost:8080`. Using `.edit()` would require a running mock server on every
Save. The encrypt + repo.set path keeps the round-trip entirely local, matching how
`list_ciphers` and `full_cipher` already work.

**Tradeoff**: Password-history tracking that `CipherEditRequestInternal` normally performs
inside `.edit()` is skipped — those types are `pub(super)` and can't be reused from
outside the SDK. Acceptable until a backend is wired up.

## Localization: `i18n-embed` + Fluent

**Decision**: Use `i18n-embed` (+ `i18n-embed-fl` + `rust-embed`) with Mozilla's Fluent
`.ftl` file format. Translation assets live under `assets/i18n/{lang}/{crate}.ftl`
alongside the other binary resources (fonts, SVGs, icons) and are embedded into the binary
at compile time. A `fl!` convenience macro at the crate root wraps `i18n_embed_fl::fl!` so
call sites don't repeat the static loader argument.

**Alternatives considered**:

- **`rust-i18n`** (~190k downloads/month, v4.0.0 released 2026-04-11): the smallest
  dependency, `t!("key")` macro with minimal boilerplate, loads YAML/TOML/JSON at compile
  time with runtime locale switching via `set_locale`. Ruled out because its formats
  aren't supported natively by Poedit (translators' default tool), and its flat key→string
  model can't express Fluent's plurals / gender / select-expressions — migrating later
  would mean rewriting every translation.
- **`gettext-rs`** (~137k/month): gold-standard `.po` / `.mo` format with ubiquitous
  external tooling (Poedit, Crowdin, Weblate, Transifex, Lokalise). Ruled out because it's
  an FFI to libintl, which complicates the Windows MSI packaging in `tools/packager/` —
  our target platform is Windows-first.
- **`rosetta-i18n`**: last released June 2023 (~34 months stale). Effectively abandoned.
  Skipped.
- **Own keyed-lookup system**: not considered seriously. Handling plural rules, gender,
  and locale-specific formatting correctly is what the libraries exist for.

**Rationale**:

- **Fluent format chosen for future-proofing**. `.ftl` is the only format here that
  natively expresses plurals, gender, and CLDR-style select-expressions without
  escape-hatching to code. Starting on a limited format (key→string) and migrating later
  would cost more than the small upfront complexity.
- **Tooling**: Fluent is supported by Crowdin, Weblate, Pontoon, and a Poedit plugin —
  enough for a real translation workflow.
- **`i18n-embed-fl` is the differentiator** over raw `fluent-bundle`: the `fl!` macro
  validates message IDs and argument names at **compile time** against the `.ftl` files. A
  typo is a build error, not a silent blank string on a screen no one opens in CI.
- **Runtime locale switch** is trivial: `DesktopLanguageRequester` auto-detects the OS
  locale at startup via `i18n::init()`. A future settings dropdown that calls
  `i18n_embed::select(...)` re-renders all views on iced's next frame with no extra
  plumbing — this works because `view()` reads the live loader each frame, not a cached
  snapshot.

**Structure**:

- Translation files under `assets/i18n/{lang}/bitwarden_desktop_next.ftl` — colocated with
  other binary resources (fonts, SVGs) because they're assets, not source code.
- `crates/desktop/i18n.toml` points at `assets_dir = "../../assets/i18n"`; the
  `#[derive(RustEmbed)] #[folder = "../../assets/i18n"]` struct mirrors the same path.
  Both macros resolve relative to `CARGO_MANIFEST_DIR`. The compile-time `fl!` check reads
  `i18n.toml` + the `.ftl` files via the same resolution, verified in
  `i18n-embed-fl/src/lib.rs`.
- File stem must match the crate identifier with underscores
  (`bitwarden_desktop_next.ftl`, not `bitwarden-desktop-next.ftl`) — `i18n-embed-fl`
  converts hyphens in the crate name but reads the file by the converted name.
- Static loader at `crate::services::i18n::LANGUAGE_LOADER`
  (`std::sync::LazyLock<FluentLanguageLoader>`), initialized once from `main()` via
  `services::i18n::init()` before `iced::daemon` starts.

**Convenience `fl!` macro** at the crate root (`main.rs`):

```rust
#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::services::i18n::LANGUAGE_LOADER, $message_id)
    }};
    ($message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::services::i18n::LANGUAGE_LOADER, $message_id, $($args),*)
    }};
}
```

Import per-module: `use crate::fl;`. Call: `fl!("login-unlock-title")` or
`fl!("login-server-accessing", server = name)`. Returns `String`.

**API ripple: `impl Into<String>` on label parameters**. Label parameters in
`components::inputs` (`field_frame`, `text_field`, `reveal_text_field_with_submit`, etc.)
previously took `&'a str` — which works fine for `'static` literals but fails when the
caller passes `&fl!(...)` because the `String` temporary drops before the returned
`Element<'a>` does. Widened to `impl Into<String>` so both forms pass transparently; the
widget owns the label internally instead of borrowing it.

**Pilot**: the login view (`views/login/*`) was converted first — the smallest string set
in the app (~25 keys), and the one new users see before any vault state exists. Other
views will migrate opportunistically as they're touched.

**Kill-switch criteria**: revisit if (a) translation teams push back on `.ftl` tooling
(unlikely given Pontoon / Crowdin / Weblate support), or (b) compile times grow
meaningfully from the per-file compile-time validation — currently negligible for one
`.ftl` file, but worth re-measuring past ~10 languages.

## Organizations & Collections: Seeded from `mock.json`

**Decision**: Orgs and collections are stored in `mock.json` per-user (not in SQLite) and
exposed via app-level `ClientManager::list_organizations` / `list_collections`. Org
symmetric keys are generated by `fake-data`, wrapped with the user's public key into an
`UnsignedSharedKey`, stored in `mock.json`, and replayed on `unlock` via
`initialize_org_crypto`.

**Rationale**: The SDK has no `Repository<Organization>` available via the managed
migrations, so these lists would otherwise have nowhere to live. Seeding from `mock.json`
mirrors how the user profile itself is persisted (`mock.json` holds KDF + wrapped user
key). Replaying the wrapped org key on unlock is what makes `CiphersClient::encrypt` /
`decrypt` succeed for org-owned ciphers — without it, encryption fails with
`MissingKeyId("Organization(...)")`.

## System-Tray Icon: `tray-icon` crate + shared `muda` receiver

**Decision**: Use the `tray-icon` crate (sister crate to `muda`) for the system tray. Tray
menu events flow through `muda::MenuEvent::receiver()` — the same global receiver the
native app menu already uses — so a single 16 ms poll drains both. Tray-icon click events
(left-click toggle) come through `tray_icon::TrayIconEvent::receiver()` on the same poll
tick.

**Version coupling**: `tray-icon` 0.22 pins `muda` 0.17. Our direct `muda` dep is held at
`0.17` to match — attempting to mix `muda = "0.18"` (which we had briefly) splits the
global event channel between the two versions and tray menu clicks are never seen by the
app.

**Linux runtime requirement**: `tray-icon` on Linux needs `libayatana-appindicator3-1`
(apt/.deb) or a `StatusNotifierItem` host in the DE (KDE ships one; GNOME needs the
AppIndicator extension). When neither is present, `crate::tray::build()` returns `None`
and the tray-gated settings become no-ops. Document this dep in the Linux installer
scripts before shipping.

**macOS template image**: the tray icon is set via `.with_icon_as_template(true)` so
NSStatusBar recolours it for light and dark menubar modes. The PNG asset is monochrome
(black) with alpha — vendored from the Electron client's `icon-template.png`.

**macOS native minimize limitation**: the yellow traffic-light button is handled by AppKit
and doesn't route through our `WindowAction::Minimize` handler, so `minimize_to_tray`
doesn't apply to it. Accepted limitation; would need platform-specific NSWindow
subclassing to override.

## Tray Lifecycle Settings: `data/settings.json`

**Decision**: The three tray-adjacent user settings (`show_tray_icon`, `minimize_to_tray`,
`close_to_tray`) live in `data/settings.json` and are loaded once at startup into a `Copy`
struct on `App`. No UI yet — the user edits by hand. Starting hidden is not a setting:
it's a `--autostart` CLI flag the future "open at login" path will pass when registering
the auto-launch entry, so the choice belongs to the launcher rather than the persisted
user prefs.

**Live-update readiness**: the struct lives as a plain `App` field and every behavioural
branch reads `self.settings.<field>` at event time (never cached). When a future settings
view lands, mutating the field directly will apply immediately; tray creation is
idempotent via `App::ensure_tray` (and a future `remove_tray` on the same pattern).

**Why not include a live file watcher**: the file is expected to be edited before launch
as an early-days workaround, not during a run. A `notify` watcher is a small follow-up if
needed; excluded to keep this change tight.

## Single-Instance Lock: stdlib probe + tokio listener

**Decision**: A second launch of the app detects the running primary via a local-IPC
probe, sends `"show\n"`, and exits. The primary has a listener that yields a message into
the iced update loop, which surfaces the window (gain-focus / unhide / or open if
`--autostart` skipped it).

**Transport** (tokio, already a dep via iced):

- **Unix**: Unix domain socket at `/tmp/bitwarden-desktop-next.sock`.
  `tokio::net::UnixListener` on the primary; `std::os::unix::net::UnixStream` for the sync
  probe in `main()` (no tokio runtime yet).
- **Windows**: named pipe at `\\.\pipe\bitwarden-desktop-next`.
  `tokio::net::windows::named_pipe::ServerOptions` on the primary (fresh pipe instance per
  accepted client); `std::fs::OpenOptions::open(...)` on the probe side (CreateFileW under
  the hood).

**Stale-socket self-healing (Unix)**: if the primary was hard-killed, the socket file
lingers. On probe failure we know nobody is listening, so the file is definitionally stale
and `cleanup_stale_socket` removes it before `bind`. Windows needs no cleanup — named
pipes are kernel objects that vanish with the process.

**Listener lifetime**: bound inside an `iced::stream::channel`-backed
`Subscription::run(instance_lock::wake_stream)`. The `fn` pointer is the subscription
identity, so iced keeps the same stream instance across `update()` cycles instead of
rebinding each tick.

**Why not the `single-instance` crate**: that crate only detects a duplicate — it doesn't
deliver a wake signal to the primary, so a second launch would just exit silently and the
user would think the app was broken. Writing it ourselves (~100 lines) delivers real UX
and avoids pulling in dbus-rs (its Linux dep).


## Bitwarden SDK 4 upgrade

**Decision (2026-11-07):** Pin every workspace Bitwarden dependency to upstream
`main` commit `a13b4c4e1fa3685280241f417920fd19d7ef24a6` (4.0.0), replacing
`10af2ea2f20b6d17ec1e4654892184e37fc4626e` (3.0.0). Keep exact Git revisions and
the lockfile for reproducible builds. This is a tested development snapshot,
not a claim that upstream `main` is a stable published release.

The SDK's crypto dependency graph is updated with it; existing prerelease pins
in the old lockfile conflict with the new SSH/PKCS8 requirements. Use current
public key-generation and registry constructors. Legacy V1 unlocks explicitly
leave the new contained-key ID unset. Preserve session revocation, operation
leases, key cleanup and durable account-removal boundaries.

The UI adapts to the SDK's typed calendar dates using independent input buffers:
incomplete input stays visible, valid input updates the typed field, and invalid
input prevents saving. SSH generator results are converted to vault views. New
password-generator options default to unset. SDK item Sends remain explicitly
unsupported and cannot be saved through the existing text/file editor.

The SDK now attempts a feature-flag `/config` refresh during rehydration. Future
login/mTLS integration must establish the account transport before this call.
The available sync entry points still need a complete integration proof; the
app's login and sync stubs do not become functional through this upgrade.


**Validation:** `cargo test --workspace --locked` passed 219 desktop tests and the
seed-tool test; `cargo clippy --workspace --all-targets --locked` completed with
only the three existing desktop warnings. Both desktop and seed-tool release
binaries built. GUI tests loaded all 50 work-vault items from copied SDK-3 stores
and from newly generated SDK-4 stores, then verified lock, logout, restart recovery
and an empty profile after removing the last account. Existing UI snapshots passed.
The application dependency graph still excludes GPU rendering, TIFF and OpenEXR;
SDK memory-hardening remains enabled. A private SQLite-API backup of the preview
profile was created before any future migration by the new preview binary.


## Atomic read-sync snapshots through SDK SQLite

The pinned SDK's sync orchestrator does not yet register a cipher handler or
transact all handlers together. Applying its individual repositories directly
would expose mixed snapshots and allow a failed refresh to delete usable rows.
The native read-sync adapter therefore commits the complete typed encrypted
response as one SDK `Setting` row and publishes read-only cipher/folder adapters
backed by one immutable snapshot Arc. SQLite remains SDK-managed; there is no
second database engine or persistent JSON cache. Local and server account IDs,
schema version and successful sync time travel with the same commit.

Network fetch and token rotation use the existing account transport. A successful
snapshot advances session authority so queued plaintext results and grants from
the old revision cannot survive. Crypto remains in SDK components. Organization
keys are initialized under an exclusive lease with cancellation/failure rollback;
the empty-membership case explicitly clears keys, which the upstream helper does
not do itself. Already-authorized encrypted commits can drain during lock, while
plaintext publication remains forbidden after revocation. First delivery is
read-only; writes, background polling, WSS, attachments and full policies follow
separately.


## Tray visibility under Wayland

Use native backend detection when hiding the main surface. The pinned winit
Wayland implementation's `set_visible(false)` deliberately does nothing, so an
Xvfb/X11-only acceptance check cannot validate KDE/Wayland behavior. Wayland tray
hiding closes the native main surface while the Iced daemon retains account/view
state; restoring opens a replacement surface with the remembered size, fullscreen,
maximized and stacking state. No SDK session or vault data is recreated. Pending
backend results are checked against both current window ID and desired visibility.

The Linux tray uses a monochrome symbolic shield rather than the colored app icon.
KDE receives its SVG with ColorScheme-Text, allowing the panel palette to select the
foreground color; a monochrome pixmap remains available to other SNI hosts. This is
a desktop theme convention, not a requirement that all Linux tray icons be black.

Validation: 264 desktop tests and one fixture test pass, including suspension and
hide/restore race tests. A disposable native KDE/Wayland profile was closed and
restored twice through its SNI item, checking the main surface disappears/reappears
with a new native ID. An X11 profile also verified the actual Window → Hide to tray
menu click. The symbolic icon name/path were checked over D-Bus. Clippy retains only
the three existing menu/opacity warnings.
