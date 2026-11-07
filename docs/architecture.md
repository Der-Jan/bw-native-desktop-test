# Architecture

This is the prescriptive guide for the `bitwarden-desktop-next` crate. It defines the
module layout, the dependency direction, and the contract each module type must satisfy.
Treat the rules as rigid. Deviations need a short comment explaining why.

The [security remediation concept](security-concept.md),
[login and sync requirements](login-sync-requirements.md), and
[mTLS requirements](mtls-requirements.md) propose changes to account lifecycle,
SDK client ownership, persistence and networking. The first session and storage
boundaries and durable local removal are implemented below. Production account
identity, password/TOTP/email login and scoped HTTP mTLS now have a pre-sync
implementation. Full read sync uses an atomic encrypted snapshot in SDK-SQLite
and read-only runtime repository adapters; server writes, WSS and full feature
parity remain future contracts.

## Overview

`bitwarden-desktop-next` is a lightweight Rust alternative to the Electron/Angular
Bitwarden desktop app, built on the **Iced 0.15** GUI framework (git-pinned; 0.15 is still
pre-release on crates.io). It wraps the real `bitwarden-*` SDK crates for all crypto,
password derivation, wrapped account keys and local vault work. A bounded app HTTP
adapter supplies the login/2FA protocol fields missing from the modern SDK login API. The UI layer is in Rust; there is no JavaScript.

### Tech stack

- **Language**: Rust 2024
- **GUI**: Iced 0.15 (git) with the `tiny-skia` CPU backend. GPU rendering is
  excluded. SVG support remains; image codecs explicitly exclude OpenEXR and TIFF.
- **SDK**: 4.0.0 at `a13b4c4e1fa3685280241f417920fd19d7ef24a6`; `bitwarden-core` / `bitwarden-crypto` / `bitwarden-pm` / `bitwarden-state` /
  `bitwarden-vault` git-pinned.
- **Async**: Iced's `tokio` multi-thread runtime.
- **Menus**: `muda` for native macOS, custom-drawn title bar on Windows/Linux.
- **Theme**: Custom `AppTheme` with dark/light palettes, runtime switching, light default.
- **Packaging**: `cargo-packager` via `Packager.toml`.

## Layers

```
┌──────────────────────────────────────────────────┐
│ app/              router, top-level Message      │
├──────────────────────────────────────────────────┤
│ views/<name>/     per-screen state + update + UI │
├──────────────────────────────────────────────────┤
│ components/       shared UI atoms                │
├──────────────────────────────────────────────────┤
│ services/<name>/  platform + stateful services   │
├──────────────────────────────────────────────────┤
│ domain.rs + theme/  types + visual contract      │
└──────────────────────────────────────────────────┘
```

**Invariant:** a module may import only from layers strictly below its own. Within a
layer, no sibling-to-sibling imports — sibling views communicate only through `App`'s
router; sibling services communicate only through well-defined APIs.

**Enforcement:** Rust visibility is the primary mechanism. Bare `pub` is reserved for two
places: **(a)** the documented public API of the archetype a file belongs to —
`{View}View` / `{View}Message` / `{View}Event` / `view()` / `update()` / `new()` / named
task factories for views, the `(data, callbacks, ctx) -> Element` constructors for
components, the service's outward-facing methods. **(b)** enum/struct fields that are part
of that public API. Internal helpers and state use the narrowest level that works:

- `pub(in crate::views::<view>)` — visible only inside one view's subtree (including its
  `widgets/`). Use for view-local domain enums and state structs.
- `pub(super)` — visible to the parent module. Use for sub-module internals (e.g. a
  `widgets/send_edit/sections/details.rs` field helper that `view.rs` invokes).
- `pub(crate)` — visible across the crate. Reserved for view archetype entry points and
  the event-handler carve-out (`handle_{view}_event`).
- private (no `pub`) — default for struct fields and internal helpers.

Rule of thumb: before writing `pub`, ask _"is this part of the archetype's documented
API?"_ If no, narrow it.

The compiler refuses cross-layer imports at the `use` line once visibility is set
correctly. A grep-based CI check is available as a backstop. When a `pub` enum variant
must wrap a narrower type (e.g., `VaultMessage::Sidebar(SidebarMessage)` carrying
view-local payloads), silence the resulting `private_interfaces` lint with a scoped
`#[allow]` and a one-line comment pointing to the reason.

**One carve-out:** each view's `handler.rs` is the single view-layer file allowed to
import `crate::app::App`. Its job is to translate the view's events into app-level effects
(screen flips, user switches, toast pushes); locking it to the view that owns the events
keeps related code together.

## Directory Layout

```
crates/desktop/src/
├── main.rs                         # entry: font load, backend select, iced::daemon
├── domain.rs                       # truly cross-cutting types: UserId, Screen, UnlockMethod(s)
├── paths.rs                        # data_dir() — static helpers
├── assets.rs                       # include_bytes! blobs
│
├── app/                            # router layer
│   ├── mod.rs                      # App struct, new/update/view/subscription
│   ├── message.rs                  # top-level Message + WindowMessage + SystemMessage
│   ├── ctx.rs                      # UpdateCtx, RenderCtx bundle definitions
│   ├── helpers.rs                  # stateless App accessors (window id, toast push, menu_state)
│   └── handlers/
│       ├── platform.rs             # window lifecycle, system events, menu/tray dispatch
│       ├── sidebar.rs              # SidebarMessage dispatch + screen transitions
│       └── account_switcher.rs     # AccountSwitcherEvent routing + user switch
│
├── services/                       # platform + stateful cross-view services
│   ├── sdk/                        # ClientManager (bitwarden SDK facade)
│   ├── clipboard/                  # ClipboardManager + sensitivity
│   ├── favicon/                    # FaviconService + fetch stream
│   ├── global_hotkey/              # OS-level hotkey → Magnify toggle stream
│   ├── menu/                       # MenuTree (menu_tree()) + muda binding + MenuAction
│   ├── tray/                       # TrayHandle + click stream
│   ├── broadcast_stream.rs         # OnceLock<Receiver> → iced Stream adapter
│   ├── i18n/                       # fluent loader + fl! macro target
│   ├── instance_lock/              # single-instance guard + wake stream
│   ├── preferences/                # per-user UserPreferences
│   ├── settings/                   # Settings load/save
│
├── components/                     # shared UI atoms (typography + layout helpers
│   │                               #  live in mod.rs: section_label/heading/card,
│   │                               #  pane_header/footer, styled_card, separator_h/v,
│   │                               #  rail_scroll_style, favicon_icon, labeled_checkbox,
│   │                               #  CARD_SHADOW)
│   ├── buttons.rs                  # primary/secondary/ghost/ghost_icon/transparent + icon_button/delete_icon_button
│   ├── icons.rs                    # bootstrap-icons font + constants
│   ├── inputs/                     # field_frame, text_field, select_field, reveal_text_field, …
│   ├── drop_down.rs                # iced_aw fork with BelowLeft/BelowRight/AboveRight
│   ├── fade_in_out.rs              # lilt-backed open/close animator for overlays
│   ├── spinner.rs / virtual_list.rs / totp.rs
│   ├── bottom_sheet.rs / modal.rs  # window-level overlays composed at App root
│   ├── collapsible_pane.rs         # list/detail split that stays mounted on close
│   ├── list_pane.rs                # shared list-screen chrome (header + render shell)
│   ├── shell_scope.rs              # Stack capture-leak isolation wrapper
│   ├── sidebar.rs                  # app-level nav chrome (see "Sidebar + shared chrome")
│   ├── toast/                      # folder: multi-file component
│   └── account_switcher.rs         # avatar trigger + dropdown panel + header_switcher
│
├── views/                          # one folder per screen (team boundary)
│   ├── login/
│   │   ├── mod.rs
│   │   ├── handler.rs              # handle_login_event(&mut App, ev) — layer carve-out
│   │   └── (view-private subviews) # unlock.rs, login_email.rs, login_password.rs, …
│   ├── vault/
│   │   ├── mod.rs
│   │   ├── handler.rs
│   │   └── widgets/                # view-private custom widgets
│   ├── settings/, title_bar/       # same shape
│   ├── magnify/                    # secondary launcher window (Ctrl+Shift+Space)
│   └── about/                      # stateless exception (single mod.rs + handler.rs)
│
└── theme/                          # visual contract (palette + iced catalog)
```

## Module Archetypes

### View (`views/<name>/`)

Always a folder. Expected file set:

| File         | Contents                                                        | Soft cap |
| ------------ | --------------------------------------------------------------- | -------- |
| `mod.rs`     | `pub use` re-exports; public entry (`new`, `update`, `view`)    | 150      |
| `state.rs`   | `{View}View` struct + view-local domain types                   | 300      |
| `message.rs` | `{View}Message`, `{View}Event`, helper enums                    | 200      |
| `update.rs`  | `impl {View}View::update` (takes `&UpdateCtx`) + helpers        | 500      |
| `view.rs`    | `impl {View}View::view` (takes `&RenderCtx`) + subview builders | 500      |
| `handler.rs` | `impl App { pub(crate) fn handle_{view}_event(...) }`           | 200      |
| `widgets/`   | view-private custom widgets                                     | —        |

The state/message/update/view split applies once the view's `mod.rs` approaches the soft
cap — smaller views can keep everything in `mod.rs` plus a `handler.rs`. Trigger rule:
when `mod.rs` crosses ~400 lines **and** holds two or more of
`{state fields and methods, message/event enums, update match, view builders}`, extract in
this order: `state.rs` first (data + struct), then `message.rs` (enums), then `update.rs`
and `view.rs` last (since they're the largest). The goal is every file under ~500 lines.

**Public surface of a view** (the full crate-visible API, callable from `App`):

- `{View}View` struct (fields stay private; construction via `new()`).
- `{View}Message`, `{View}Event` enums.
- `{View}View::new()`, `update()`, `view()`.
- `handle_{view}_event` (defined inside `handler.rs` as `impl App`, marked `pub(crate)`).

**Optional — present when the view owns that feature:**

- `dismiss_dropdowns()` — closes local overlays when a cross-view message arrives. Only
  needed when the view owns non-app-level overlays.
- `View::overlays(&RenderCtx) -> Vec<Element>` — secondary surfaces (sub-modals, bottom
  sheets, in-view confirmations) stacked on top of `view()` in z-order. Composed by
  `App::collect_overlays` at the App root so their backdrop covers the title bar and
  sidebar. Each view manages its own visibility internally and only pushes elements that
  should render this frame. Vault and Send push both their narrow-mode bottom sheet and
  their delete-confirm sub-modal here; Login pushes its self-hosted URL modal.
- `reset(&uid, filter)` / `apply_filter(&uid, filter)` — called by App from user-switch
  and sidebar-filter handlers. Views that own a filterable list implement both; they keep
  per-user cached data consistent with the current sidebar selection.
- `remove_user_items(&uid)` — drop per-user cached data on sign-out.
- `focus_search_task()` — return a `Task` that focuses the view's search input. Called by
  File-menu shortcuts (Ctrl+F).
- Named task factories (e.g. `load_list_task`). Convention: top-level
  `pub fn {action}_task(...)` in `update.rs`. Called from `App` helpers when a task needs
  to be kicked off outside the view's own `update()` (e.g., post-unlock list reload).
  Factory functions never take `&mut self` — they take owned + borrowed inputs and return
  `Task<{View}Message>`.

Everything else stays `pub(super)` or private. Struct fields default to private.

### Stateless view exception (`about`)

A view with no runtime state skips the `{View}View` struct and the `update/new` methods.
Its mod.rs exports just:

- `pub(crate) enum {View}Message` — the outbound actions (e.g. `AboutMessage::CopyInfo`,
  `AboutMessage::Close`).
- `pub(crate) fn view(ctx: &RenderCtx) -> Element<{View}Message, AppTheme>`.
- Any pure helpers the message handler needs (e.g. `info_string()`).

The `handler.rs` still exists and implements `impl App { fn handle_{view}_message(...) }`.
Use this pattern only when the view is strictly non-interactive beyond dispatching
messages and has no input state, hover state, or async work — for example, the About
window. If state is ever added, convert to the full archetype.

### Component (`components/<name>.rs` or `components/<name>/`)

Single file by default; folder only when the component needs multiple files (toast).

- Public API: `(data, callbacks, &RenderCtx_or_&AppColors) -> Element`.
- Internal state (reveal toggle, hover, countdown, fade, progress) lives **inside** the
  component via iced's `Component` trait, widget-tree state (`Tree` param), or
  `Rc<Cell<_>>` for values that style closures read during draw.
- **Forbidden:** requiring the parent to pass `is_revealed: bool` or `progress: f32` or
  any transient UI state. The parent provides data; the component renders.
- Components may not import from `views/` or `app/`. They may receive `&RenderCtx` but
  never `&UpdateCtx`.

Reference implementations:

- `components/toast/` — self-driving animations via `RedrawRequested` +
  `Rc<Cell<ToastVisuals>>`.
- `components/spinner.rs` — self-animating ring via `shell.request_redraw()`.
- `components/totp.rs` — countdown widget with internal timer.

### Service (`services/<name>/`)

Always a folder:

| File        | Contents                                     |
| ----------- | -------------------------------------------- |
| `mod.rs`    | Public API: struct + `pub(crate)` methods    |
| `<impl>.rs` | Private implementation (workers, caches, IO) |

- Constructed once in `App::new`.
- Injected to views via `UpdateCtx` (for update paths) or `RenderCtx` (for render paths
  where the service is visible — e.g. `FaviconService`).
- May not import from `views/`, `components/`, or `app/`. May import from `domain`,
  `theme`, and other services (acyclic).

### Domain (`domain.rs`)

- Cross-cutting types only: `UserId`, `Screen`, `UnlockMethod`, `UnlockMethods`.
- View-local domain lives in the owning view's `state.rs` or `mod.rs`. Examples:
  `VaultFilter` in [views/vault/state.rs](crates/desktop/src/views/vault/state.rs),
  `SendFilter` in [views/send/state.rs](crates/desktop/src/views/send/state.rs); login
  view's local types in `views/login/`. `SidebarMode` and `NavSection` live in
  [components/sidebar.rs](crates/desktop/src/components/sidebar.rs) (the only "view-aware"
  component, intentionally cross-cutting — it imports `VaultFilter` and `SendFilter` for
  nav buttons).
- No logic beyond `impl` on the types (`Display`, `From`, parse).

## Context Bundles

Signatures with 5+ args use a bundle. Two bundles are defined in `app/ctx.rs`:

```rust
// Update-time: services + session + sidebar selection + the open-overlay cell.
// Used only by view update() paths.
pub struct UpdateCtx<'a> {
    pub client_manager: &'a mut ClientManager,
    pub active_user: Option<&'a UserId>,
    pub active_vault_filter: VaultFilter,
    pub active_send_filter: SendFilter,
    pub open_overlay: &'a mut Option<Overlay>,
}

// Render-time: everything needed to render. Allowed in components.
pub struct RenderCtx<'a> {
    pub colors: &'a AppColors,
    pub favicon: &'a FaviconService,
    pub show_favicons: bool,
    pub window_width: f32,
    pub active_user: Option<&'a UserId>,
    pub active_email: Option<&'a str>,
    pub active_server_url: &'a str,
    pub accounts: &'a [AccountEntry],
    pub open_overlay: Option<Overlay>,
}
```

Rules:

- Every view implements the [`View`](crates/desktop/src/app/ctx.rs) trait — its `update()`
  takes `UpdateCtx<'_>` and returns `Outcome<Self>` (see next section).
- Every view's `view()` and its internal render helpers take `&RenderCtx`.
- Components take `&RenderCtx` (or a narrower arg like `&AppColors` if they only need
  colors).
- Components **never** take `UpdateCtx` — the type isn't reachable from `components/`, so
  the compiler prevents it.
- Any new service added to `UpdateCtx` or `RenderCtx` becomes available to all views with
  no signature churn.

## The `View` trait

Every screen-level view implements the [`View`](crates/desktop/src/app/ctx.rs) trait,
which welds state + `Message` + `Event` + `update` + `should_render` + `view` + `overlays`
together in one place:

```rust
impl View for VaultView {
    type Message = VaultMessage;
    type Event = VaultEvent;

    fn update(&mut self, msg: VaultMessage, mut ctx: UpdateCtx<'_>) -> Outcome<Self> {
        /* … */
    }

    fn view<'a>(&'a self, ctx: &RenderCtx<'a>) -> Element<'a, VaultMessage, AppTheme> {
        /* primary render surface */
    }

    // Optional overrides:
    // - `should_render` (default `true`) — modal views gate on their FadeInOut.
    // - `overlays` (default `Vec::new()`) — secondary surfaces stacked on top
    //   of `view`: sub-modals, bottom sheets, in-view confirmations.
}
```

Rendering is split into three pieces:

- **`should_render`** — predicate gating whether `view` is called this frame. Default
  `true` for full-screen views; modal views override to return `self.fade.is_visible()`.
- **`view`** — the primary render surface. Returns `Element` directly (callers must check
  `should_render` first; the method may panic otherwise).
- **`overlays`** — `Vec<Element>` of secondary surfaces, stacked in z-order on top of
  `view`. Used for sub-modals (vault delete-confirm, login self-hosted), bottom sheets
  (narrow-mode cipher detail), and any in-view confirmations.

### Outcome

Every `update` returns an [`Outcome<Self>`](crates/desktop/src/app/ctx.rs):

```rust
pub enum Outcome<V: View> {
    None,
    Task(Task<V::Message>),
    Event(V::Event),
    Toast(Toast),
}
```

Mutually-exclusive cases. Call sites use named constructors:

```rust
Outcome::None                                            // fallthrough / no-op
return Outcome::event(LoginEvent::Unlocked { uid });     // bubble a fact
return Outcome::perform(async move { ... }, |r| msg);    // spawn an SDK task
return Outcome::toast(Toast::error(body, Some(&title))); // surface a toast directly
return Outcome::from_option(maybe_event);                // Some/None → Event/None
```

No `From<Event>` blanket impl — Rust's coherence rules conflict with the reflexive
`From<T> for T` from stdlib. The named `Outcome::event(e)` constructor is the accepted
equivalent.

App-side routing uses the `dispatch` method (see "Router + cross-view dismissal" below).
`Outcome::Toast(t)` is routed by `dispatch` directly through the `PushToast` impl on App —
views construct toasts with `Outcome::toast(t)` rather than threading a `ToastRequested`
event variant.

**The `Both(Task, Event)` case is deliberately excluded.** When a view wants both to spawn
a task _and_ bubble an event (typically "SDK op succeeded → reload list + show success
toast"), it emits a richer event (e.g. `VaultEvent::ItemSaved { uid }`) and App's handler
fires both effects. The view reports the fact; App decides the follow-up.

### `perform_with_active_client` helpers

The most common shape is "extract the active user's `SessionClient`, run async,
dispatch result". Helpers on both App and `UpdateCtx` capture that pattern:

```rust
// From app/helpers.rs — App-side, returns Task<Message>:
self.perform_with_active_client(
    move |client| client.list_ciphers(),
    |uid, res| VaultMessage::ListLoaded(uid, res).into(),
)

// From app/ctx.rs — view-side, returns Outcome<Self>:
ctx.perform_with_active_client(
    move |client| client.save_cipher(view),
    move |uid, res| VaultMessage::SaveCompleted(uid, res),
)

// Variant for callers with an explicit uid (from a message payload):
ctx.perform_with_client(uid, move |client| client.full_cipher(id), …)
```

Internally these call `ClientManager::client_for(uid)`, which returns a revocable
`SessionClient`, and wrap `Task::perform`; the manager is never borrowed across `.await`.
Raw SDK handles stay inside `services/sdk`. Every operation takes a per-account
read lease and checks its session generation before and after SDK work. Lock/logout
revoke authority synchronously, then obtain an exclusive lease before clearing SDK
keys. Crypto initialization uses the same exclusive lease and cancellation cleanup.

Task results and secret-bearing rendered UI messages carry a `SessionToken` and
presentation generation. `App::update` rejects stale generations, replaced account
incarnations, and results for a different selected account. Account switches advance
the presentation generation even when switching back to the same account.

Lock immediately drops sensitive forms, lists, generator state, overlays, Magnify
results and app-owned clipboard text, without an outro containing old secrets.
In-flight side effects drain before final key cleanup; revocation cannot undo an
already committed mutation. Export holds its lease through a private staged write,
checks authority before publication, and zeroizes its final serialization buffer.
Do not treat dropping ordinary SDK/GUI strings as proof of complete memory erasure.

Normal startup uses the OS local application-data directory, mode 0700 on Unix,
and does not load mock metadata. `--demo <directory>` explicitly opts into the mock
loader. Production account loading remains unimplemented.

`SessionState` owns the optional SDK client. `SessionClient`, `UnlockData` and
`LockOperation` retain authority tokens; they obtain a raw client only under their
execution lease. This lets removal close the SDK owner even while old tasks or
native dialogs have not started/resumed. Manager replacement also revokes tokens
and releases its SDK owners.

`ClientManager::log_out` returns a `LogoutOperation`. It commits a private durable
marker through `RemovalStore`, finishes crypto cleanup under the exclusive lease,
then removes demo account metadata and UUID-derived SQLite files. Mutations of the
shared metadata are serialized; I/O runs on blocking workers. Startup validates
markers and retries cleanup before opening any database. Invalid journal state
blocks account loading. Markers remain to reject restored old caches.

App tracks pending logout operation IDs so duplicate completions cannot finish a
different logout. Pending progress and storage failures render above the page and
survive account/lock transitions. Normal menu/tray/window shutdown waits for logout;
a failed intent commit cancels deferred exit so its warning remains visible.
Production token-store deletion and account incarnation namespaces must extend this
protocol when login is implemented.

### `ClientExt` extension trait

App-facing async SDK calls live on
[`SessionClient`](../crates/desktop/src/services/sdk/session_client.rs) via the
`ClientExt` trait — `client.list_ciphers().await`, `client.save_cipher(view).await`,
`client.export_vault(format).await`, etc. Importing `services::sdk::ClientExt` brings them
in scope. Unlock is the exception: it lives on `UnlockData` (returned by
`ClientManager::unlock_data_for(uid)`) because it needs more than the client.

## Messages + Events

### Suffix discipline

- `*Message` — inbound (widget events + async completions). The `update(msg)` argument.
  Imperative widget events (`LoginMessage::Unlock`) and past-tense async completions
  (`VaultMessage::ListLoaded`) both belong here.
- `*Event` — outbound fact a sub-view bubbles up. Describes what _happened_, not what to
  do next. Examples: `LoginEvent::Unlocked`, `VaultEvent::ItemSaved`,
  `SettingsEvent::Applied`. Toasts don't need event variants — views surface them directly
  via `Outcome::toast(t)`.
- `*Action` — discriminant for an abstract operation: `MenuAction::Quit`,
  `WindowAction::Minimize`. Use as a payload on an `Event` when the view is requesting an
  app-level effect.

Private widget-internal enums under `components/` don't participate in the router; they
aren't governed by these conventions.

### Compositional MVU

Each view implements [`View::update`](crates/desktop/src/app/ctx.rs):

```rust
fn update(
    &mut self,
    msg: Self::Message,
    ctx: UpdateCtx<'_>,
) -> Outcome<Self>;
```

The return is an [`Outcome<Self>`](#outcome) — `None` / `Task(t)` / `Event(e)` /
`Toast(t)`, mutually exclusive. App's `handle_{view}_event` methods (in the view's
`handler.rs`) translate each event into concrete side effects: screen switches, user
swaps, toast pushes, menu actions, follow-on tasks.

Async callbacks live inside sub-messages, not at the top level.
`LoginMessage::UnlockCompleted(uid, Result)` is handled by `LoginView::update`
(stale-check, clear `unlock_in_progress`, emit `LoginEvent::Unlocked` or a sanitized error
toast).

### Top-level Message

```rust
pub enum Message {
    View(ViewMessage),          // per-view messages, fanned out below
    About(AboutMessage),
    Window(WindowMessage),      // per-window OS events
    System(SystemMessage),      // MudaEvent, TrayClick, ThemeChanged, etc.
    Sidebar(SidebarMessage),
    Magnify(MagnifyMessage),
    Favicon(FaviconMessage),    // fetch completions
}

pub enum ViewMessage {
    Login(LoginMessage),
    Vault(VaultMessage),
    Send(SendMessage),
    TitleBar(TitleBarMessage),
    Settings(SettingsMessage),
    Generator(GeneratorMessage),
    Import(ImportMessage),
    Export(ExportMessage),
    NewFolder(NewFolderMessage),
}
```

Each per-view message type has an `impl From<XxxMessage> for Message` (declared via the
`impl_from_view_message!` / `impl_from_top_message!` macros in
[`app/message.rs`](crates/desktop/src/app/message.rs)). Call sites lift with `.into()` /
`.map(Into::into)` — no per-view wrapper fn needed. Generic helpers (`ViewExt::push_render`,
`Outcome::dispatch`) carry an `M: From<Self::Message>` bound and infer `M = Message` from
context.

### Router + cross-view dismissal

`App::update`'s `Message::View` arm has a pre-match dismissal block followed by the
per-variant router:

```rust
ViewMessage::Vault(m) => self
    .views
    .vault
    .update(m, uctx)
    .dispatch(self, |s, e| s.handle_vault_event(e)),
```

[`Outcome::dispatch`](crates/desktop/src/app/ctx.rs) takes `&mut S` (the App) plus an
event-handler closure (`FnOnce(&mut S, V::Event) -> Task<TopMsg>`). The `TopMsg`
target (typically `app::Message`) is inferred from the closure's return type, and the
view's local message lifts via the `M: From<V::Message>` bound — each view declares
`impl From<XxxMessage> for Message` in [`app/message.rs`](crates/desktop/src/app/message.rs).
Toast routing comes from `S: PushToast` ([impl on App in `app/helpers.rs`](crates/desktop/src/app/helpers.rs)),
so views construct toasts with `Outcome::toast(t)` rather than threading a
`ToastRequested` event variant.

The pre-match block is **load-bearing**: any non-titlebar view message dismisses the
title-bar menu, and any title-bar message dismisses other overlays (AccountSwitcher /
ServerSelector / NewItemMenu). Without it, an accidental menu click from the vault would
leave the account-switcher dropdown open.

## Cross-Cutting Rules

1. **Dependency direction** — enforced via visibility (`pub(super)` / `pub(crate)`). A
   view's internals are `pub(super)`; a component's helpers are `pub(super)`. Only
   documented archetype APIs are `pub(crate)`.

2. **Size budget** — soft cap ~500 lines per file (cognitive load + git-diff readability +
   parallel work without conflicts). Trigger the archetype split when a file crosses ~400
   lines and contains two or more separable concerns. Split in the order described in the
   View archetype above (state → message → update/view). Pure widget files (under
   `widgets/`) may exceed the cap temporarily; prefer extracting sub-sections (e.g.,
   `cipher_edit/sections/`) over folding growth back into `mod.rs`.

3. **Tests are opt-in, not mandatory** — the four-layer setup (pure / update / Simulator /
   snapshot) and helpers live in [`crate::test_support`](../crates/desktop/src/test_support/mod.rs).
   Reach for a test when logic has multiple branches with observable effects, when a bug
   would be silent at runtime, or when a comment says "this is non-obvious". See
   [docs/testing.md](./testing.md) for the full setup and the "when to skip" rules.

4. **Crate root** — only `main.rs`, `domain.rs`, `paths.rs`, `assets.rs`. Everything else
   has a categorized home.

5. **Style / theming / i18n**
   - Colors come from `AppTheme.colors` via `RenderCtx` or `&AppColors`. Never hardcode
     outside `theme/dark.rs` and `theme/light.rs`.
   - Radii (`RADIUS_SM/MD/LG/PILL`) are structural constants, not theme-dependent.
   - Font sizes: 5 values — 12, 14, 16, 18, 28.
   - Localized strings go through the `fl!` macro (compile-time checked against
     `assets/i18n/en/*.ftl`).

6. **`update()` arm style** — when a match has many state-mutation arms and a few emitter
   arms, use tail-fallthrough: each state arm is a plain statement (no tuple constructor),
   emitter arms use `return`, and the function ends with a single `(Task::none(), None)`
   after the match. This keeps the common case terse and makes `return` a visual marker
   for "this arm bubbles out". Reserve the expression-match style (explicit tuple in every
   arm) for cases where almost every arm emits, or where the match is a dispatch
   delegating to handlers that already return tuples.

   ```rust
   // Tail-fallthrough — most arms mutate, a few emit:
   match msg {
       Message::Toggle => self.open = !self.open,
       Message::Reset => self.reset(),
       Message::Save => return (save_task(), None),
       Message::Cancel => return (Task::none(), Some(Event::Cancelled)),
   }
   (Task::none(), None)
   ```

## How to Add a New View

Follow this recipe. End-to-end you'll touch your own view directory plus four mechanical
adds in `app/mod.rs` and one in each matching list.

### 1. Create the view folder

```
crates/desktop/src/views/generator/
├── mod.rs              # GeneratorView struct + GeneratorMessage + GeneratorEvent + new/update/view
├── handler.rs          # impl App { fn handle_generator_event(...) -> Task<Message> }
└── widgets/            # optional: custom widgets specific to this view
```

Minimal `mod.rs` skeleton:

```rust
mod handler;

use iced::Element;

use crate::{
    app::{Outcome, RenderCtx, UpdateCtx, View},
    theme::AppTheme,
};

#[derive(Debug, Clone)]
pub enum GeneratorMessage { /* widget events + async completions */ }

pub enum GeneratorEvent { /* outbound facts for App */ }

pub struct GeneratorView { /* local state */ }

impl GeneratorView {
    pub fn new() -> Self { Self { /* ... */ } }

    pub fn view<'a>(
        &'a self,
        _ctx: &RenderCtx<'a>,
    ) -> Element<'a, GeneratorMessage, AppTheme> {
        iced::widget::text("Generator").into()
    }
}

impl View for GeneratorView {
    type Message = GeneratorMessage;
    type Event = GeneratorEvent;

    fn update(
        &mut self,
        msg: GeneratorMessage,
        _ctx: UpdateCtx<'_>,
    ) -> Outcome<Self> {
        // match msg { ... }
        Outcome::None
    }
}
```

**Convention:** in every view-side method, `ctx` (the `UpdateCtx` or `RenderCtx`) is the
first parameter after `&mut self` / `&self`. Per-message handlers split out of `update()`
follow the same rule:

```rust
fn handle_save_completed(
    &mut self,
    ctx: &UpdateCtx<'_>,
    msg_uid: UserId,
    result: Result<…>,
) -> Outcome<Self> { … }
```

Register in `crates/desktop/src/views/mod.rs`:

```rust
pub(crate) mod generator;
```

### 2. Add the Message variant in `app/message.rs`

```rust
pub enum Message {
    // existing variants...
    Generator(GeneratorMessage),
}
```

### 3. Add the view to `Views` in `app/mod.rs`

```rust
pub struct Views {
    // existing fields...
    pub(super) generator: generator::GeneratorView,
}

impl Views {
    pub fn new() -> Self {
        Self {
            // existing initializers...
            generator: generator::GeneratorView::new(),
        }
    }
}
```

The `Views` substruct exists so `App::update` can split borrows: the view-dispatch arm
takes `&mut self.views` plus `&mut self.open_overlay` via `UpdateCtx`, while event
handlers reborrow other App fields without overlap.

### 4. Add the router arm in `App::update`

```rust
ViewMessage::Generator(m) => self
    .views
    .generator
    .update(m, uctx)
    .dispatch(self, |s, e| s.handle_generator_event(e)),
```

Add `GeneratorMessage => Generator` to `impl_from_view_message!` in
[`app/message.rs`](crates/desktop/src/app/message.rs) so `Outcome::dispatch` can lift
the local message to `Message` via `From`.

Cross-view dismissal lives at the top of the `Message::View` arm in `App::update`: a
pre-match block closes the title-bar menu when a non-titlebar view message arrives, and
closes other overlays (AccountSwitcher / ServerSelector / NewItemMenu) when a title-bar
message arrives. New view types automatically participate — the block matches on
titlebar-vs-others, not on a per-view enumeration, so you don't need to extend it for a
typical new view.

### 5. Implement the event handler in `views/generator/handler.rs`

```rust
use iced::Task;

use crate::{
    app::{App, Message},
    views::generator::GeneratorEvent,
};

impl App {
    pub(crate) fn handle_generator_event(&mut self, event: GeneratorEvent) -> Task<Message> {
        match event {
            // translate facts into app-level effects
        }
    }
}
```

### 6. Wire a screen switch (if the view owns a screen)

Add `Screen::Generator` in `domain.rs`, branch on it in `App::view_main`, and flip to it
from the triggering action. For a screen driven from the sidebar, see "Sidebar + Shared
Authenticated Chrome" above — adding a `NavSection` variant + handler dispatch arm is the
idiomatic path.

If the view is a panel/modal within an existing screen, skip this step.

### 7. Expose window-level overlays (if the view owns any)

If the view owns an overlay that should cover the **entire** window (including the title
bar and sidebar) — typically a delete-confirmation modal or a narrow-mode bottom sheet —
push it from `View::overlays`. Each view's `overlays(&RenderCtx) -> Vec<Element>`
returns its sub-modals and sheets in z-order; `App::collect_overlays` extends the
window-level stack with them. Manage visibility internally (typically via a `FadeInOut`)
and only push when visible. See [decisions.md](./decisions.md) → "Window-Level Overlays
Composed at App Root" for the why.

### What this recipe costs in `app.rs`

Exactly five mechanical adds plus one `impl App { fn handle_generator_event }` in the
view's own folder:

1. One `Message::Generator(GeneratorMessage)` variant.
2. One `From<GeneratorMessage> for Message` entry in `impl_from_view_message!`.
3. One `generator_view: GeneratorView` field on `App`.
4. One router arm (calls `.update().dispatch(...)`).
5. One line in `App::collect_overlays` — `self.views.generator.push_render(rctx, &mut out)`.

Plus, as applicable: one `Screen::Generator` variant + `view_main` branch (if the view
owns a screen), nothing extra for window-level sub-overlays beyond what `overlays()`
already exposes, and one `NavSection::Generator` + handler arm (if it's sidebar-driven).

## Sidebar + Shared Authenticated Chrome

Authenticated screens (Vault, Send) share two pieces of chrome: the **left sidebar**
(`components/sidebar.rs`) and the **top-right account switcher**
(`components/account_switcher.rs`). Both are rendered alongside the per-screen content in
`App::view_main` — the views themselves don't re-render them. Sidebar buttons may also
open window-level modals (Generator, Import, Export) without flipping `Screen`.

### Sidebar state lives on `App`

`SidebarState` (mode, active section, per-screen filters, collapsed navigation groups) persists across
screen switches, so it belongs on `App`, not inside any one view.
[`app/handlers/sidebar.rs`](../crates/desktop/src/app/handlers/sidebar.rs) owns the full
dispatcher: section clicks may flip `Screen`, filter clicks mutate `SidebarState` **and**
push the new filter down to the owning view via its `apply_filter(&uid, filter)` method.
The view never reads `SidebarState` back — App is the single writer, views are receivers.

Vault filtering combines three independent selections: ownership scope (all, personal,
or one organization), item type, and view (active, favorites, archive, trash), plus
the search query. Sidebar changes update only their own selection. Switching accounts
resets the selections. If a selected
organization disappears during sync, only the ownership scope returns to all vaults.
The navigation has independent collapse controls for Vaults, View, Item types, Folders,
Collections and Send. Collapsing a group changes only its visibility, preserving every
filter and the other groups. There is no outer Vault group. The list heading follows
the selected ownership scope (all vaults, personal vault, or organization name).
A secondary heading line lists active type, favorites/archive/trash, folder, collection
and search constraints; defaults are omitted. Scope and filter text stay on one line
each, clipped to available width with full text on hover in narrow panes.
Heading labels avoid the pinned renderer's height-limited ellipsis cache, which
can hide unchanged text after a list relayout. The vault table adds the SDK cipher's
`revision_date` in local time and the first usable web URI's hostname/port. URI paths,
queries, fragments and user information are excluded. Non-login items show no URL.
Header and rows share a horizontal scroll viewport with a 720 px minimum table width;
the nested vertical list remains virtualized with its existing row-height contract.
The horizontal bar is inset from the window resize edge.
Name, last-change and base-URL headers select/toggle column sorting. The preference
(column and direction only) persists in Settings; transient comparison keys live with
the per-account item cache and are discarded on lock/logout. Reordering reconciles
selection by cipher ID and resets the virtual list offset. URL-less rows stay last.
Manual sync is available through File → Sync now; the list has no duplicate sync button.
The sidebar navigation scrolls so all filters remain reachable with many organizations.
Folder and collection filters combine with these selections and search. Folder names
and organization collection names use slash-separated nesting and render as indented
trees, including non-selectable headings for missing parents. Selecting a real parent
includes its descendants (collection descendants stay within that organization).
“All” and “None” choices clear the corresponding constraint or select unfiled items.
Collections are grouped by organization and hidden for the personal vault. Changing
ownership clears an incompatible collection choice. List reload clears only folder
or collection selections whose IDs have disappeared. Folder metadata loads alongside
ciphers under the same revocable SDK lease; caches and names are purged on lock/logout.

### Adding a sidebar-driven nav section (screen _or_ modal)

Editing [`components/sidebar.rs`](../crates/desktop/src/components/sidebar.rs) is a
single-file touch that covers every sidebar concern. The sidebar treats two different
shapes uniformly:

- **Screen-typed entries** (Vault, Send): clicking flips `Screen` and the right-hand pane
  re-renders. The handler arm calls `switch_to_{name}()`.
- **Modal-typed entries** (Generator, Import, Export): clicking opens a window-level
  modal. The handler arm calls `open_{name}_modal()`. `active_section` is **not** mutated
  — the highlight stays on the previously-active screen so it tracks where the user
  returns when the modal closes.

Steps for either shape:

1. Add a `NavSection::{Yours}` variant.
2. If the entry owns a filterable list, add a `{Yours}Filter` enum +
   `active_{yours}_filter` field on `SidebarState` + `{Yours}FilterSelected(...)` on
   `SidebarMessage`. Model from `SendFilter` / `VaultFilter`. (Modals typically skip
   this.)
3. Render the section in `expanded_panel()` (parent header row + nav buttons) and the icon
   rail.
4. Add a dispatch arm in
   [`app/handlers/sidebar.rs`](../crates/desktop/src/app/handlers/sidebar.rs):
   - Screen entry: `self.sidebar.active_section = section; self.switch_to_{yours}()`
     (plus, if applicable, a filter arm calling
     `self.views.{yours}.apply_filter(&uid, filter)` then the switch helper).
   - Modal entry: `self.open_{yours}_modal()` and nothing else.

Sidebar-related changes are intentionally centralized — the sidebar is logically one
widget. This is _not_ a layering violation; it's the correct home for chrome that spans
multiple screens. When it grows to the point of friction, revisit by splitting per-section
sub-modules under `components/sidebar/` rather than by distributing state back into views.

### Account switcher reuse

The top-right avatar + dropdown is wired via
`components::account_switcher::header_switcher(active_email, accounts, is_open, colors)` —
one call returns the complete trigger + panel in `AccountSwitcherMessage` space. Each view
maps it into its own message type with one `.map(MyMessage::AccountSwitcher)`. The
resulting `*Event::AccountSwitcher` bubbles up through the view's handler and lands in
[`app/handlers/account_switcher.rs`](../crates/desktop/src/app/handlers/account_switcher.rs)
— the single place where switch / add / lock / logout semantics live.

## Menu System

`services::menu::menu_tree()` builds a `MenuTree` at runtime — every label flows through
the `fl!()` macro at construction so Fluent message IDs are validated against the
catalogue at compile time. `App::menu` caches the tree; `App::rebuild_menu()` reconstructs
it (and re-attaches the native menu) on language change so both surfaces below pick up new
translations on the next frame. Both consumers walk the same tree:

- **Custom title bar** (Windows/Linux): renders labels, dropdowns, shortcuts, submenus via
  `views/title_bar/`. Walks `RenderCtx::menu.sections` per frame; `ItemAction(MenuAction)`
  click messages embed the resolved action so dispatch is a one-liner.
- **Native muda** (macOS, or `DEV_BOTH_MENUS=1`): `Shortcut::to_accelerator()` adds
  shortcuts; `NativeMenuHandle` bridges events through
  `services::menu::muda_event_stream()`. A push callback registered once at startup via
  `muda::MenuEvent::set_event_handler` fans events out through a
  `tokio::sync::broadcast::Sender`; each subscription run calls `.subscribe()` for a fresh
  receiver.

`MenuChildren` makes the three child shapes explicit:

- `None` — leaf entry (has an action, no children).
- `Static(Vec<MenuEntry>)` — fixed at build time (e.g. File → New item).
- `Dynamic(DynamicSubmenu)` — children resolved at render time from live account state.
  Used by File → Lock vault (per unlocked account) and File → Log out (per known
  account). Both renderers walk dynamic children via
  `MenuEntry::effective_children(accounts)`; native muda also tracks the submenu handles
  and refills them via `NativeMenuHandle::sync_dynamic` from `refresh_accounts_cache`.

`MenuState { is_locked, has_accounts, has_lockable_accounts }` — three bools driving
`EnabledWhen::Always / Unlocked / HasAccounts / HasLockable`.

## Overlays

Two kinds, both via iced's native overlay system (window-level rendering, not nested in
the page layout):

- **Dropdowns** — `components::drop_down::DropDown` (local fork of iced_aw). Custom
  alignments: `BelowLeft`, `BelowRight`, `AboveRight`. Used for the account switcher and
  title-bar menus. Wraps user-supplied overlay content in `iced::widget::opaque` so
  clicks on the panel's empty space do not fall through. Panels use borders and
  background colors for separation; painted drop shadows are excluded.
- **Toasts** — `components::toast::Manager` wraps the app content and overlays a vertical
  stack of toasts in the lower-right. Animation state (fade, progress, hover-pause) is
  driven from the overlay's `update()` via `window::Event::RedrawRequested` ticks — no
  separate `Subscription`.

In addition, two **window-level overlay archetypes** are composed at the App root (above
the main column, including the sidebar and title bar):

- **Modal dialogs** — `components::modal::{view, dialog, confirm_dialog, info_dialog}`.
  Backdrop scrim, click-to-dismiss, a visible border, and immediate open/close via the compatibility `FadeInOut` visibility state.
  The dialog body is itself wrapped in `opaque` so clicks on its empty space don't dismiss;
  the entire stack is wrapped in `opaque` so hover doesn't leak through to widgets behind.

  Canonical scaffolding for app-level modals:

  ```rust
  let header = modal::dialog_header(title, M::Close, ctx.colors);
  // …build body fields…
  let footer = modal::footer_actions(save_label, on_save, cancel_label, M::Close);
  let body = column![header, …fields, footer]
      .spacing(N)
      .padding(modal::BODY_PADDING)
      .width(Fill);
  modal::dialog(width, None, |c| c.card_bg, progress, body, M::Close)
  ```

  Use `modal::info_dialog(width, icon, title, body, actions, …)` instead when the modal
  is a confirmation/notification with an accent-coloured icon ring up top — it bakes in
  the ring + centered title + button stack so the per-call site shrinks to
  the body element and the action button list. `modal::confirm_dialog` is the shorthand
  for the common "title + body text + Cancel/Confirm" shape.
- **Bottom sheet** — `components::bottom_sheet`. Narrow-mode detail/form pane. Same
  scrim + opaque pattern. Open/close uses a `FadeInOut` on the owning view's `Selection`;
  the close path schedules selection cleanup immediately, without an animation timer.

Iced overlays support only ONE level — a `DropDown` inside another `DropDown`'s overlay
won't render its own overlay. Submenus must be part of the same overlay content (e.g.
`row![main_panel, submenu]`).

## Windows

`App` holds `windows: HashMap<window::Id, WindowInfo>` and dispatches per-window via
`WindowKind::{Main, About, Magnify}` in `view()` / `theme()`. Three concrete windows
today; the `WindowKind` enum is the single point of extension for adding more.

- **Main** — created in `App::new` via `iced::window::open`. Holds the entire
  authenticated experience (vault, send, generator, settings). Close-to-tray is gated by
  `exit_on_close_request: false` so the custom handler decides whether to actually close.
- **About** — opened on demand from `MenuAction::About`; closes normally. Reference
  implementation for "second window with normal chrome" — see
  [app/handlers/platform.rs](../crates/desktop/src/app/handlers/platform.rs).
- **Magnify launcher** — borderless transparent secondary window summoned by the
  `Ctrl+Shift+Space` global hotkey
  ([crates/desktop/src/services/global_hotkey/](../crates/desktop/src/services/global_hotkey/mod.rs)).
  Created only on an explicit hotkey press and destroyed on dismissal. There
  is no hidden startup window: Wayland ignores winit's visibility setting. Resizes dynamically
  (`window::resize`) as the result count changes — the only window in the app that does.
  Owns its own state on `App::magnify` and reads decrypted ciphers off
  `VaultView::all_items_for(uid)`; its message variant `Message::Magnify(MagnifyMessage)`
  routes directly into `app::handlers::magnify` rather than through `ViewMessage`, because
  the launcher's update path doesn't share `UpdateCtx` with the screen-driven views.
  Sticky search persists across summons within a 5-minute TTL; expired or cross-user
  summons reset.

The global hotkey is wired through the same `tokio::sync::broadcast` + `Subscription::run`
pattern used by `services::menu` and `services::tray` — `install_event_handler()` runs
once at startup before the first hotkey can fire.

## Animation

Only local status indicators (spinner, toast, TOTP countdown) animate. They drive their
own redraws through `RedrawRequested` / `request_redraw_at`, without an app frame ticker.
Dialogs, sheets, dropdown menus and the generator tab indicator switch immediately.
`FadeInOut` retains its existing name/API but stores only a boolean; visible progress
is always 1.0. `focus_after_open` focuses on mount without sleeping. A sheet cleanup
uses `Task::done` and ignores completion if the sheet has already reopened.

## External Event Sources (broadcast-channel recipe)

Crates that hand the app a global `set_event_handler` callback (`muda` for menus,
`tray-icon` for the tray, `global-hotkey` for the launcher hotkey) all funnel into iced
via the same shape — codified across `services/menu`, `services/tray`, and
`services/global_hotkey`. Use this recipe whenever wiring a new external event source
instead of `iced::time::every` polling or pushing through a thread:

```rust
static EVENTS: OnceLock<broadcast::Receiver<MyEvent>> = OnceLock::new();

pub fn install_event_handler() {
    // Underlying lib's setter is OnceCell-backed and rejects re-registration —
    // so this entire function is a one-shot. Idempotent failure is fine.
    let (tx, rx) = broadcast::channel(16);
    let _ = EVENTS.set(rx);
    Lib::set_event_handler(Some(move |e| {
        let _ = tx.send(translate(e));
    }));
}

pub fn event_stream() -> impl Stream<Item = MyEvent> {
    super::broadcast_stream::from_once_lock(&EVENTS, "my-source")
}
```

The pump body (resubscribe, recv-loop, lag-warn, close-break) lives in
[`services/broadcast_stream.rs`](../crates/desktop/src/services/broadcast_stream.rs); each
service exports a one-line `event_stream` wrapper around it.

The App side wires it once via `Subscription::run(event_stream)` mapped onto a top-level
`Message`. Three invariants worth burning in:

- **Store the `Receiver`, not the `Sender`.** `broadcast::Receiver::resubscribe()` is the
  supported way to fan out — calling it inside `event_stream` gives every iced
  subscription its own cursor while the original receiver in the `OnceLock` keeps the
  channel alive (broadcast channels close when all receivers drop, _not_ when senders
  drop).
- **`Subscription::run` identity is by function pointer.** Pass a bare `fn`, not a closure
  — closures hash to a fresh identity on every `view()` and iced will tear down + re-spawn
  the stream every frame. `Subscription::run(event_stream)` is correct;
  `Subscription::run(|| event_stream())` is not.
- **Always handle `RecvError::Lagged(n)`** by logging and continuing — broadcast channels
  are bounded (16 is the convention here) and a paused subscriber that misses messages
  must be allowed to recover instead of breaking out of the loop.

`install_event_handler()` is called from `App::new` (or equivalent startup point) exactly
once. Failing to install (Wayland for `global-hotkey`, no SNI host on Linux for
`tray-icon`, etc.) is logged and silently ignored — the rest of the app keeps running and
`event_stream()` terminates immediately, leaving the subscription idle.

## Iced Gotchas

- `button::Style` / `rule::Style` require `snap: false` — missing it produces a confusing
  compile error pointing at the struct literal.
- `view()` returns `Element<'_, M, Theme>` borrowing from `&self`. Locally-computed `Vec`s
  can't flow into the returned Element — use cached fields (`ViewCache`, per-user
  `ItemCache`).
- **Nested container backgrounds mask parent border-radius**: put the rounded fill on the
  innermost container, or drop inner backgrounds.
- **Stack doesn't cull or clip**: `iced::widget::stack` lays out every child fully at the
  same bounds. Prefer exclusive branches in `view()` over layering when only one is shown
  at a time.
- **`iced::widget::Stack` short-circuits between its own children on
  `shell.is_event_captured()`, but the `Shell` is shared across the whole event pass.** A
  capture set inside one widget subtree leaks into a sibling subtree's `Stack`, which then
  bails before reaching its remaining children. Wrap each affected subtree in
  [`components::shell_scope::ShellScope`](../crates/desktop/src/components/shell_scope.rs)
  — see [Shell Capture Isolation](#shell-capture-isolation-shellscope) below.
- External event sources are pushed into `tokio::sync::broadcast` channels at startup and
  consumed via `Subscription::run(fn_pointer)`. Avoid `iced::time::every()` for external
  sources.
- `AppTheme` is cloned every time iced calls `App::theme(window_id)`. Keep it cheap:
  `name` is `&'static str`, `colors` is `Copy`.

## Shell Capture Isolation (`ShellScope`)

`Shell` is iced's per-event-pass side-channel: widgets use it to publish messages, request
redraws, and call `capture_event()` to mark an event as handled so ancestors stop
bubbling. iced uses **one** `Shell` for the whole event pass — it's a `&mut` reference
threaded down the tree, so every widget reads and writes the same flag.

That's mostly fine because few widgets _read_ `is_event_captured()`. The problem ones are
widgets whose `update` does

```rust
self.child.update(.., shell, ..);
if shell.is_event_captured() { return; }
// ...follow-up logic that should only run if the child didn't handle it
```

In iced 0.15 (rev `4e0bdcf`) the readers are
`widget/src/{stack.rs:242, button.rs:282, mouse_area.rs:221, scrollable.rs:783, helpers.rs:906}`.
`button` / `mouse_area` / `scrollable` only skip their own self-targeted follow-up, so a
stale flag from a sibling at most makes them no-op when the cursor isn't over them anyway
— invisible. **`Stack` is the outlier**: its check sits inside the loop that delivers the
event to its remaining children, so a sibling-set flag stops other children from running,
which _is_ observable.

Concrete repro: two `pick_list`-bearing
[`field_frame`](../crates/desktop/src/components/inputs/mod.rs)s as siblings in a column.
Open the second (its picker sets `is_open = true`). Click the first. The first's `Stack`
reaches its picker (its own first child doesn't capture), the picker captures on open.
iced's column then iterates to the second `field_frame`'s `Stack`, which processes its
first child (label, no capture), checks `shell.is_event_captured()` — _true, set by the
first stack's picker_ — and returns before reaching its own picker. The second picker's
close branch never runs, both menus end up open.

**Fix shape:** give the inner subtree a private `Shell`, run `update` against it, then
`Shell::merge` the result back. iced uses this exact pattern internally for
[`combo_box::update`](https://github.com/iced-rs/iced/blob/4e0bdcf54aeaadec6753fffd1b1185087bf03b67/widget/src/combo_box.rs#L578)
and
[`lazy::component`](https://github.com/iced-rs/iced/blob/4e0bdcf54aeaadec6753fffd1b1185087bf03b67/widget/src/lazy/component.rs#L326)
— both stash a `local_shell` so their child's capture status is computed in isolation.
[`ShellScope`](../crates/desktop/src/components/shell_scope.rs) is the same pattern
factored out as a reusable wrapper widget so any subtree containing a `Stack` can be
insulated with `ShellScope::new(stack![...]).into()`. `field_frame` already wraps with it;
if you write a new helper that uses `Stack` and is meant to live as a sibling of itself,
do the same.

A small upstream patch would obviate this — `Stack::update` could capture
`was_captured_before = shell.is_event_captured()` once at the start of the loop and
short-circuit only on captures that happened _during_ its own iteration:

```rust
let was_captured_before = shell.is_event_captured();
for child in ... {
    child.update(.., shell, ..);
    if shell.is_event_captured() && !was_captured_before { return; }
    ...
}
```

If that lands upstream, drop `ShellScope` and the wrap call in `field_frame`.

## Iced Source Reference

Git-pinned checkouts for investigation:

- iced core: `~/.cargo/git/checkouts/iced-*/<rev>/core/src/`
- iced widgets: `~/.cargo/git/checkouts/iced-*/<rev>/widget/src/`
- iced futures: `~/.cargo/git/checkouts/iced-*/<rev>/futures/src/`
- iced_aw (registry, kept as reference): `~/.cargo/registry/src/*/iced_aw-0.13.1/src/`
- muda: `~/.cargo/registry/src/*/muda-0.18*/src/`

## Key Dependencies

- `iced` (git, 0.15) with `tokio`, `tiny-skia`, `advanced`, `svg`, `image`, `crisp`,
  `hinting`, `web-colors`, `x11`, `wayland` features
- `muda = "0.19"` — native OS menus
- `iced_aw` (0.13, default-features = false) — kept as local source reference for our
  `DropDown` fork
- `system-theme = "0.3"` — OS dark/light detection
- `bitwarden-*` — SDK (git-pinned via `[workspace.dependencies]`)
- `tracing` / `tracing-subscriber` — structured logging driven by `RUST_LOG`
- `cargo-packager` — used by `tools/packager`


## Persisted navigation, Linux tray and SSH agent

`services::vault_navigation` owns the serializable independent vault facets. Settings
store selections by local account UUID and global collapsed sidebar groups/mode.
Startup/account switch restores the selected account's choices; list reconciliation
clears missing folder, collection and organization IDs. Logout removes that account's
selection. Only IDs/enums are persisted, never a search string or item name.

Linux tray uses `ksni` StatusNotifierItem directly, with callbacks bridged into the
existing broadcast subscription. Other platforms retain `tray-icon`. Hiding requires
an available tray; loss of the host restores a hidden window. Explicit hidden state
plus unminimize/focus makes restoration independent of stale window-mode queries.
On Wayland, the pinned winit implements `set_visible` as a no-op. Hiding queries
the actual raw backend, then closes only the main native surface on Wayland. The
logical window geometry and all view/SDK state stay in the daemon. Restore creates
a new main surface/ID. On opening, virtual list scroll offsets are reapplied to
the fresh widget tree so cached row windows and scrollbars stay aligned. Hiding
marks the main window unfocused for session idle deadlines. Intentional suspension never follows the application-exit
path; stale hide/close/raw-handle results are ignored after restoration. X11 and
other native backends retain cheap visibility toggles. Linux advertises a monochrome
`bitwarden-symbolic` SVG through SNI IconName/IconThemePath, with KDE
ColorScheme-Text styling and a monochrome ARGB fallback.

`services::ssh_agent` owns the listener, framing/parser, session bindings, short-lived
approval broker and remembered grants. `services::sdk::ssh_agent` is the sole private
key boundary, using the existing session leases and revision/policy checks. The app
refreshes agent authority after every update; SDK token invalidation also denies
operations before the next UI update. Approval metadata enters Iced through a
broadcast subscription and renders as an exclusive modal. Runtime shutdown revokes
authority before aborting tasks/removing the owned socket. See the
[SSH agent requirements](ssh-agent-requirements.md) for protocol/security limits.


The SSH listener's default path is now stable and readable:
`$XDG_RUNTIME_DIR/bitwarden-desktop-next/ssh-agent.sock`, with a private per-UID
system-temp fallback. No profile hash is exposed in the endpoint. Parallel profiles
may choose explicit absolute `BITWARDEN_SSH_AUTH_SOCK` endpoints; occupied paths
fail closed. The settings UI can copy either the literal path or a shell-quoted
export command. It never attempts to modify the parent shell environment. The unused
DuckDuckGo integration setting and its messages/translations have been removed;
legacy serialized values are ignored and omitted on the next settings save.


## Native SSH approval windows and keyboard focus

Each SSH approval owns a separate `WindowKind::SshApproval` surface, associated
with its request UUID. A new always-on-top surface gains activation on KDE/Wayland,
where the pinned winit `focus_window` on an existing surface is a no-op. The main
vault keeps its prior visibility and is covered by an inert modal while approval
is pending. Only public request metadata enters the native dialog. Native close,
Escape, timeout, disconnect, lock and authority changes close it with denial.
Late open/close/dismiss events cannot adopt or dismiss a newer request; an open
that loses a race with cancellation is closed immediately.

`components::focus_button` adds the missing keyboard focus behavior to the pinned
Iced button: a focus ring, Enter/Space, no auto-repeat approval, and a Tab cycle
restricted to the prompt's action IDs. Messages carry the original approval UUID;
there is no global Enter handler that could approve a replacement prompt. A new
request gets a fresh widget tree. The selected prompt preference controls whether
Allow once or the eligible Allow until lock action is the initial default.

Reusable grants require both a verified OpenSSH session binding and a host-bound
signature covering its final host key. Both ask modes offer the explicit grant;
plain/unverified requests remain one-use. Permission/revision/key rechecks and
session-generation revocation are unchanged.
