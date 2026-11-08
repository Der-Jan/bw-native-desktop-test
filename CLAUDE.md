# bitwarden-desktop-next

Lightweight Rust alternative to the Bitwarden desktop app using the Iced 0.15 GUI framework (git-pinned — 0.15 is still pre-release on crates.io).

UI-only stub: crypto, API, auth, vault decrypt run through the real `bitwarden-*` SDK crates (git-pinned via `sdk-internal`). Multi-user (one `PasswordManagerClient` per user in `ClientManager`). Custom `AppTheme` with light default, dark/light toggle. Dual menu system (custom on Windows/Linux, native muda on macOS). Cargo workspace; `default-members` skip `tools/*`.

## Quick Start

```bash
cargo run                                     # Empty profile in private OS application storage
cargo clippy                                  # Lint check (must pass clean)
cargo run --bin packager                      # Package .app/.dmg/.msi
cargo run -p fake-data -- --output ./demo-data # New/empty explicit demo directory
cargo run -- --demo ./demo-data                # Synthetic account preview
```

## Dev Modes

```bash
DEV_BOTH_MENUS=1 cargo run                                            # Native + custom menus side-by-side
RUST_LOG=bitwarden_desktop_next=debug,bitwarden_core=debug cargo run  # Full SDK tracing
```

Rendering defaults to wgpu with automatic tiny-skia CPU fallback, following the user’s renewed priority on scrolling responsiveness. Use `ICED_BACKEND=tiny-skia cargo run` for a CPU comparison or `cargo run --no-default-features` for a CPU-only build. Keep dialogs immediate and shadow-free. Image codecs are selected explicitly in `crates/desktop/Cargo.toml`; keep Iced on `image-without-codecs` so it does not re-enable OpenEXR or TIFF through image defaults.

## Docs (READ BEFORE CHANGES)

- [docs/architecture.md](docs/architecture.md) — project structure, state model, compositional MVU pattern, menu/icon systems, **"How to Add a New View"** recipe.
- [docs/decisions.md](docs/decisions.md) — rationale for framework choice, theme system, overlay approach, MVU, startup lazy-load, packaging, etc. Read first when changing an architectural choice.
- [docs/todo.md](docs/todo.md) — pending work tiers, deferred / upstream items.
- [docs/design-reference.md](docs/design-reference.md) — official-app findings: button styles, palette, fonts, menu structure, screens.
- [docs/testing.md](docs/testing.md) — four-layer test setup (pure / update / Simulator / snapshot), `test_support` helpers, when to write a test, when to skip.
- [.claude/skills/audit/SKILL.md](.claude/skills/audit/SKILL.md) — partition-by-directory + combined-lens audit recipe, runnable via the `audit` skill.

## Coding Conventions

### Imports
- Group imports: `use crate::{a, b};` not separate `use crate::a; use crate::b;`.
- Nest component imports: `use crate::components::{buttons, icons, account_switcher};`.
- Import commonly-used iced types directly: `Background`, `Border`, `Color`, `Shadow`, `Alignment`.

### Padding
- Use `[v, h]` shorthand: `.padding([8, 16])`.
- Use `Padding { top, right, bottom, left }` only for asymmetric cases where neither pair is symmetric.

### Theming
- All colors come from `AppTheme.colors`. Never hardcode outside `theme/dark.rs` and `theme/light.rs`.
- Radii (`RADIUS_SM/MD/LG/PILL`) are structural constants, not theme-dependent.
- In `.style()` closures: `theme.colors.xxx`. For `text().color()` / `icon.render()`: pass `&AppColors`.
- Sidebar-specific tokens: `nav_text` (white in both themes) and `nav_item_hover`.
- Text size roles use `crate::text_size(12 | 14 | 16 | 18 | 28)`: compact, rounded point sizes with a 9 pt minimum. Icon sizes remain separate.
- Prefer builder form: `Border::default().rounded(r).color(c).width(w)` and `container::Style::default().background(c).border(b)` over `{ ..Default::default() }` struct literals.

### Button Components
- Use `components::buttons::{primary, secondary, ghost, ghost_icon, transparent}(content)`.
- Takes `impl Into<Element>`, returns `Button` for chaining.
- `ghost(content, is_active, active_bg, hover_bg, radius)` — full knobs for sidebar + lists.
- `ghost_icon(content, hover_bg)` — shorthand for icon-only transparent buttons (RADIUS_SM, no active state).
- `icon_button(icon, msg, &AppColors)` — 18 px ghost-icon button, returns `Element`. Used for copy / launch row actions.
- `delete_icon_button(msg, &AppColors)` — destructive variant of `icon_button` with the trash glyph in `titlebar_close_hover` (red).

### Icons
- `icon.render(size, color)` — renders as Element for general use.
- `icon.input_icon(size, side)` — builds `text_input::Icon` for `TextInput::icon()`.
- `icon.char()` is `pub(crate)` — used by `toast` and `window_chrome` where the codepoint needs to compose with other text styling.

### Dropdowns
- Pick the right helper for the job:
  - **Single-select from a small fixed list** (card brand, deletion preset, access type) → `inputs::select_field` (iced `pick_list`). Handles its own positioning and dismissal. **Default to this** unless you specifically need one of the other two.
  - **Searchable single-select** (folder, organization) → `inputs::search_select_field` (iced `combo_box`).
  - **Checkbox panel or other custom trigger/panel content** (collection multi-select) → `inputs::multi_select_field` (our `DropDown`). Only reach for this when `pick_list` genuinely can't render what you need — the custom `DropDown`'s overlay positioning is naive (flips left past the viewport edge if the trigger sits in the right half of the window), so fields inside a right-side pane commonly misposition.
- If you do use `DropDown` directly: always set `.on_dismiss(message)` for click-outside-to-close, and cross-view dismissal lives at the App router (see [docs/architecture.md](docs/architecture.md) → "Router + Cross-View Dismissal"). Sub-views provide `dismiss_dropdowns()` helpers; don't make sub-views aware of each other.
- **Field chip background.** `field_frame` / `select_field` paint the floating-label chip with `colors.background` so the chip cleanly cuts the border behind it. When the field sits on a different surface (e.g. directly inside a dialog body painted with `card_bg`), the chip will read as a tile of contrasting colour. Reach for `field_frame_on` / `select_field_on` (or `errored_field_frame_on` for the validation-error state) and pass the surface's colour token (`|c| c.card_bg`) so the chip blends. `TextField`'s builder routes `chip_bg` through both the normal and errored branches automatically.

### Dead Code
- Use `#[expect(dead_code)]` (not `#[allow]`) — warns if suppression becomes unnecessary.
- Exception: `components/icons.rs` uses `#![allow(dead_code)]` at file scope because the generated `bootstrap_icons_generated.rs` includes many unused `Icon` constants.

### Widget IDs
- Widget IDs referenced from more than one file live as `pub const <NAME>: widget::Id = widget::Id::new("...")` in the widget's own module (e.g. `views/vault/widgets/search_bar.rs::SEARCH_ID`).

### MVU + SDK
- `ClientManager` ([services/sdk/mod.rs](crates/desktop/src/services/sdk/mod.rs)) holds plain `HashMap<UserId, _>`s, no internal locking. Mutating methods take `&mut self` and run sync from `App::update`. Async SDK calls go through `ClientExt` methods on revocable `SessionClient` handles. Raw SDK clients stay in the session runtime and are obtained only under operation leases, so queued tasks cannot retain a database owner after logout. The manager is never borrowed across `.await`. Use `ClientManager::client_for(uid)` to get the handle, or `unlock_data_for(uid)` / `validation_data_for(uid)` when the call needs more than the client.
- The "extract per-user client + run async + dispatch result" shape is `App::perform_with_active_client` / `App::perform_with_client` (returning `Task<Message>`) and the matching `UpdateCtx::perform_with_active_client` / `perform_with_client` (returning `Outcome<V>`). Reach for these before re-implementing.
- `Outcome::perform(future, on_complete)` is the analogue of `Task::perform`. `Outcome::toast(t)` surfaces a toast via `dispatch` directly — views don't need a `ToastRequested` event variant.
- `ctx` is the first parameter after `self` in every view handler / view function. Both `UpdateCtx` and `RenderCtx`.

### Standard helpers
- `modal::dialog_header(title, on_close, &AppColors)` — title + close-X row at 20pt.
- `modal::footer_actions(primary, on_primary: Option<M>, secondary, on_secondary)` — left-aligned primary/secondary button row. Disable the primary by passing `None`.
- `modal::dialog(width, height, bg_picker, progress, body, on_dismiss)` — card-shape modal shell.
- `components::rail_scroll_style` — `.style(...)` fn for the muted-rail scrollable look.
- `components::rounded_top_pane(content, top_radius)` — right-pane shell painted with `card_bg`.
- `components::pane_header(title, on_close, &AppColors)` / `components::pane_footer(content)` — 18 px title + close-X header (separator below) and `background`-coloured action bar (separator above) for right-pane shells. Used by cipher detail / cipher edit.
- Use borders and background colors to distinguish cards, menus, and dialogs. Do not add painted drop shadows: the CPU renderer recomputes their full pixel area on redraw.
- `components::section_label(label, &AppColors)` (14 px primary) / `components::section_heading(label, &AppColors)` (16 px bold primary) — typography for grouping labels above a card or settings sub-section.
- `components::section_card(heading, body, &AppColors)` — bold 14 px heading stacked above a `styled_card` body. Used by send-edit; import has its own visually-distinct heading-inside-card variant.
- `components::labeled_checkbox(checked, label, on_toggle)` — standard 18 px / 8 sp checkbox used across settings + generator tab toggles.
- `components::favicon_icon(handle)` — 32×32 pre-clipped favicon slot. Pair with `FaviconService::handle_for_login_uri(uid, uri)` which folds the URI → hostname → cache lookup → globe-fallback chain into one call.
- `field_helpers::add_item_button(label, msg, &AppColors)` — "+ Add X" secondary button used by cipher-edit cards.
- `services::broadcast_stream::from_once_lock(&LOCK, "lag-label")` — iced `Stream` over a `OnceLock<broadcast::Receiver<T>>`. Used by `services/menu`, `services/tray`, `services/global_hotkey`.
- `UpdateCtx::is_active_user(uid)` — `bool` for stale-result guards in async completion handlers.
- `FadeInOut` is an immediate boolean visibility adapter; `close_with_finalize(msg)` closes and emits `msg` without a timer.
- `fade_in_out::focus_after_open(id)` — focus a mounted overlay input immediately.

## Iced Gotchas

- `button::Style` / `rule::Style` require `snap: false` — missing it produces a confusing compile error pointing at the struct literal, not the field.
- `view()` returns `Element<'_, M, Theme>` borrowing from `&self`. Locally computed `Vec`s can't flow into the returned Element — use cached fields on the struct (`ViewCache`, per-user `ItemCache`).
- iced overlays only support ONE level — a `DropDown` inside another `DropDown`'s overlay won't render its own overlay. Submenus must be part of the same overlay content (e.g. `row![main_panel, submenu]`).
- **Nested container backgrounds mask parent border-radius**: iced clips a container's own background fill to its border radius, but a child container's rectangular fill paints right over the rounded edge. Put the rounded fill on the innermost container, or drop inner backgrounds.
- **Stack doesn't cull or clip**: `iced::widget::stack` lays out every child fully at the same bounds. Two heavy subtrees stacked at full window width ≈ 2× layout cost per frame. Prefer exclusive branches in `view()` over layering when only one is shown at a time. For window-level overlays (sheets, modals), compose at the App level so the underlying view can take a cheaper exclusive branch.
- **Renderer batches text and quads separately within a layer.** Inside a single iced rendering layer, all `fill_quad` (backgrounds, borders, shadows) submissions paint first, then all text glyphs paint on top. When two overlays share a layer and overlap on screen, the lower overlay's text can leak above the upper overlay's background — both backgrounds composite as one batch, then both texts as another. **Fix: any custom overlay archetype should wrap its `draw` body in `renderer.with_layer(bounds, |r| ...)` to flush pending primitives and force a per-overlay composite.** [`components::drop_down::DropDownOverlay::draw`](crates/desktop/src/components/drop_down.rs) is the reference. Single-instance overlays (modal, bottom sheet, toast manager) don't need this today because the app never stacks two of them, but adding it defensively when introducing a new overlay type costs nothing and bulletproofs against future stacking.
- **`iced::widget::Stack` leaks `shell.is_event_captured()` across siblings.** `Shell` is shared across the whole event pass (`&mut shell` threaded down the tree). `Stack::update` (`widget/src/stack.rs:242` in the pinned rev) checks `is_event_captured()` between its own children to decide whether to keep iterating — but that flag also reflects captures done by *earlier sibling subtrees*, so a `Stack` whose first child didn't capture but whose later children would have, will bail out spuriously. Fix: wrap the affected subtree in [`components::shell_scope::ShellScope`](crates/desktop/src/components/shell_scope.rs), which gives the child a private `Shell` and `Shell::merge`s back. iced itself uses this pattern in `widget/src/combo_box.rs:578` and `widget/src/lazy/component.rs:326` — `ShellScope` is the same recipe as a reusable wrapper. `field_frame` already wraps; do the same for any new helper that builds a `Stack` and is meant to sit as a sibling of itself. See [docs/architecture.md → Shell Capture Isolation](docs/architecture.md#shell-capture-isolation-shellscope) for the full story.
- External event sources are pushed into `tokio::sync::broadcast` channels at startup and consumed by iced via `Subscription::run(fn_pointer)` — no polling, no pump threads. muda and tray-icon get their `set_event_handler` callbacks installed once (`menu::install_event_handler`, `tray::install_event_handler`); both crates' setters are `OnceCell`-backed and only accept a single registration. `instance_lock::wake_stream` uses an async socket listener inside an `iced::stream::channel` directly. Avoid `iced::time::every()` for external sources — it wakes `App::update` 60×/s even when idle.
- For per-widget animation prefer the self-driving `RedrawRequested` + `shell.request_redraw()` (or `request_redraw_at` when the cadence must be explicit) pattern — it avoids running full `App::update` cycles just to redraw one widget.
- `widget::operation::focus(Id)` sets input focus without a message round-trip; it's a `Task`-returning operation.
- Dialogs, sheets, dropdowns and generator tabs switch immediately; do not add transition animations or focus delays. Use `focus_after_open(id)` when opening an input dialog.
- `iced_aw` (0.13) is kept as a dependency with `default-features = false` purely so the source is in the cargo registry cache for reference — we use our own `drop_down.rs` fork at runtime.
- `AppTheme` is cloned every time iced calls `App::theme(window_id)`. Keep the struct cheap: `name` is `&'static str`, `colors` is `Copy`.
- **`pane_grid` eats the first click / first keystroke on widgets inside a newly-mounted pane.** `pane_grid::PaneGrid` has a custom `diff` (widget/src/pane_grid.rs:362-392 in the pinned iced rev) that retains children by matching the new `panes` list against a stashed `Memory::order`. On first mount (and on any view() that toggles the grid in/out of the tree), order is empty and every child tree slot is allocated fresh — `text_input::is_focused` resets to `None`, `text_editor`'s focus state is dropped. Result: first click on `text_editor` is ignored; first keystroke on `text_input` types one char then loses focus. Subsequent content swaps inside an already-mounted grid are fine. **Fix: use [`components::collapsible_pane::CollapsiblePane`](crates/desktop/src/components/collapsible_pane.rs).** It mounts pane_grid once from view construction and toggles visibility via the split ratio (0.0–1.0, where 1.0 collapses the right pane to zero width); `min_size(0)` on the builder lets the pane genuinely disappear. Don't roll a fresh `pane_grid::State<T>` per screen unless you're certain the grid never enters/leaves the widget tree. The cipher detail/form flow dodged this accidentally because clicking a row mounts the grid with a read-only detail pane, and Edit later swaps the detail for the form — pane_grid has warmed up by the time fields are on screen.

## Source Reference Locations

When investigating iced internals, the git-pinned iced checkouts live at:

- iced core: `~/.cargo/git/checkouts/iced-*/<rev>/core/src/`
- iced widgets: `~/.cargo/git/checkouts/iced-*/<rev>/widget/src/`
- iced futures (runtime / backend / executors): `~/.cargo/git/checkouts/iced-*/<rev>/futures/src/`
- iced_aw (registry, old version kept as reference): `~/.cargo/registry/src/*/iced_aw-0.13.1/src/`
- muda: `~/.cargo/registry/src/*/muda-0.18*/src/`
