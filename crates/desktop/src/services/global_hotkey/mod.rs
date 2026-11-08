//! Global keyboard shortcuts that fire regardless of which app has focus.
//!
//! Currently used only by the Magnify launcher window (Ctrl+Shift+Space toggles
//! it). Mirrors the broadcast-channel pattern used by `services::menu` and
//! `services::tray`: a callback installed once at startup pushes events into a
//! `tokio::sync::broadcast`, and Core subscribes via [`event_stream`].
//!
//! Wayland note: `global-hotkey` does not implement Wayland natively. Linux
//! Wayland users see a warning at startup and the launcher is unreachable
//! until they switch to X11; everything else in the app keeps working.

use std::sync::OnceLock;

use futures::Stream;
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use tokio::sync::broadcast;

/// One unit value per hotkey press. Only `Pressed` transitions are forwarded.
#[derive(Debug, Clone, Copy)]
pub struct MagnifyToggle;

static HOTKEY_EVENTS: OnceLock<broadcast::Receiver<MagnifyToggle>> = OnceLock::new();

/// Register the global-hotkey event callback and bind the V1 default
/// shortcut (Ctrl+Shift+Space on Win/Linux, Cmd+Shift+Space on macOS).
///
/// Call once at startup. Subsequent calls are no-ops because both
/// `OnceLock::set` and `GlobalHotKeyEvent::set_event_handler` are
/// single-shot. Failure to initialise the manager (Wayland, missing
/// permissions, etc.) is logged and silently ignored — the rest of the app
/// continues working without the hotkey.
pub fn install_event_handler() {
    let manager = match GlobalHotKeyManager::new() {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "global hotkey manager init failed; Magnify hotkey disabled");
            return;
        }
    };

    let modifiers = if cfg!(target_os = "macos") {
        Modifiers::SUPER | Modifiers::SHIFT
    } else {
        Modifiers::CONTROL | Modifiers::SHIFT
    };
    let hotkey = HotKey::new(Some(modifiers), Code::Space);

    if let Err(e) = manager.register(hotkey) {
        tracing::warn!(error = %e, "Magnify hotkey registration failed");
        return;
    }
    let toggle_id = hotkey.id();

    let (tx, rx) = broadcast::channel(16);
    let _ = HOTKEY_EVENTS.set(rx);
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.state == HotKeyState::Pressed && event.id == toggle_id {
            let _ = tx.send(MagnifyToggle);
        }
    }));

    // Leak the manager so the OS-level registration outlives this scope.
    // `GlobalHotKeyManager` isn't `Send + Sync` on Windows (holds a raw HWND
    // pointer), so we can't park it in a `static`. `mem::forget` keeps the
    // OS-side handle alive for the rest of the process lifetime — caller is
    // `App::new`, which only runs once at startup, so this is effectively a
    // one-shot leak.
    std::mem::forget(manager);
}

/// Runtime-independent stream of [`MagnifyToggle`]s. If [`install_event_handler`]
/// failed (e.g. Wayland), the channel was never initialised and the stream
/// terminates immediately — leaving the subscription idle.
pub fn event_stream() -> impl Stream<Item = MagnifyToggle> {
    super::broadcast_stream::from_once_lock(&HOTKEY_EVENTS, "global-hotkey")
}
