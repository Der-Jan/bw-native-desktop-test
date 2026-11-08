//! Application menu service: shared menu definitions, keyboard shortcut
//! lookup, native (muda) menu attachment, and the muda event broadcaster.
//!
//! ## Layout
//!
//! - [`shortcut`] — [`Shortcut`] / [`ShortcutKey`] + display formatting and
//!   keyboard matching.
//! - [`entry`] — [`MenuState`], [`EnabledWhen`], [`MenuAction`], [`MenuEntry`]
//!   + the small const builder (`E`, `L`, `SEP`).
//! - [`definitions`] — the static [`MENUS`] table + [`find_shortcut_action`].
//! - [`native`] — [`NativeMenuHandle`] + muda menu construction.
//!
//! The custom title-bar renderer (the egui menu bar) and the native
//! muda menu both consume the same [`MENUS`] tree; only one runs per platform
//! (macOS → native; Win/Linux → custom; both with `DEV_BOTH_MENUS=1`).

mod definitions;
mod entry;
mod native;
mod shortcut;

use std::sync::OnceLock;

use futures::Stream;
use tokio::sync::broadcast;

pub use definitions::menu_tree;
pub use entry::{
    DynamicSubmenu, EnabledWhen, MenuAction, MenuChildren, MenuEntry, MenuState, MenuTree,
};
pub use native::NativeMenuHandle;
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub use native::attach_menu;

// ── Event forwarding (push callback → broadcast → iced subscription) ──────

/// Broadcast fan-out for muda events. Every `MenuEvent` (from the native app
/// menu and the tray context menu — muda shares one receiver for both) is
/// pushed into this channel.
static MUDA_EVENTS: OnceLock<broadcast::Receiver<muda::MenuEvent>> = OnceLock::new();

/// Register muda's global event handler. Call once at startup, before any
/// menu is built. Subsequent calls are ignored (both `OnceLock::set` and
/// muda's own `OnceCell`-backed `set_event_handler` silently no-op after
/// the first).
pub fn install_event_handler() {
    let (tx, rx) = broadcast::channel(16);
    let _ = MUDA_EVENTS.set(rx);
    muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
        let _ = tx.send(event);
    }));
}

/// Runtime-independent stream of [`muda::MenuEvent`] values, one per muda-managed
/// menu click. Use as a `fn` pointer with the Core event dispatcher.
pub fn muda_event_stream() -> impl Stream<Item = muda::MenuEvent> {
    super::broadcast_stream::from_once_lock(&MUDA_EVENTS, "muda")
}

// ── Platform-specific selection ───────────────────────────────────────────

pub fn should_use_custom_menu_bar() -> bool {
    cfg!(not(target_os = "macos")) || std::env::var("DEV_BOTH_MENUS").as_deref() == Ok("1")
}

pub fn should_use_native_title_bar() -> bool {
    cfg!(target_os = "macos") || std::env::var("DEV_BOTH_MENUS").as_deref() == Ok("1")
}
