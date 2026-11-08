//! Per-window screen-capture exclusion.
//!
//! Mirrors Bitwarden's Electron client (`BrowserWindow.setContentProtection`):
//! when `protect = true` the window is hidden from screenshots and screen-
//! recording APIs at the OS level. Toggle is runtime-settable on Windows
//! and macOS — no restart needed despite what the previous TODO entry
//! claimed.
//!
//! Linux is genuinely unsupported: winit's `set_content_protected` is a
//! no-op on both X11 and Wayland (no upstream protocol), matching
//! Electron's behavior on Linux. The function still returns a `Task` so
//! the caller doesn't need to gate on `cfg(target_os = ...)` at every
//! call site.
//!
//! ## Caller responsibility
//!
//! Apply at startup (post-`window::open`) for the persisted state, and on
//! any `SettingChange::AllowScreenshots` toggle. The settings flow uses
//! the "confirm window still visible" pattern (5 s timeout, auto-revert)
//! so a user accidentally enabling protection in a remote-desktop session
//! doesn't lose access to the window.

use raw_window_handle::HasWindowHandle;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use raw_window_handle::RawWindowHandle;
/// Call only on the UI thread, while the native handle is live.
pub fn apply(window: &impl HasWindowHandle, protect: bool) {
    let Ok(handle) = window.window_handle() else {
        return;
    };
    #[cfg(target_os = "windows")]
    apply_windows(handle.as_raw(), protect);
    #[cfg(target_os = "macos")]
    apply_macos(handle.as_raw(), protect);
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let _ = (handle, protect);
}

#[cfg(target_os = "windows")]
fn apply_windows(handle: RawWindowHandle, protect: bool) {
    use windows_sys::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE},
    };

    let RawWindowHandle::Win32(h) = handle else {
        tracing::warn!("screenshot_protection: non-Win32 handle on Windows");
        return;
    };
    let hwnd: HWND = h.hwnd.get() as *mut _;
    let affinity = if protect {
        WDA_EXCLUDEFROMCAPTURE
    } else {
        WDA_NONE
    };
    // SAFETY: hwnd came from winit via raw-window-handle; valid for the
    // lifetime of the window. SetWindowDisplayAffinity is documented as
    // safe to call from any thread for any window the process owns.
    unsafe {
        SetWindowDisplayAffinity(hwnd, affinity);
    }
}

#[cfg(target_os = "macos")]
fn apply_macos(handle: RawWindowHandle, protect: bool) {
    use objc2_app_kit::{NSView, NSWindowSharingType};

    let RawWindowHandle::AppKit(h) = handle else {
        tracing::warn!("screenshot_protection: non-AppKit handle on macOS");
        return;
    };

    // SAFETY: AppKitWindowHandle.ns_view is a non-null pointer to a live
    // NSView for as long as the iced window exists. iced runs window
    // operations on the main thread, where AppKit access is required.
    let ns_view: &NSView = unsafe { h.ns_view.cast::<NSView>().as_ref() };
    let Some(ns_window) = ns_view.window() else {
        tracing::warn!("screenshot_protection: NSView has no window");
        return;
    };
    let sharing = if protect {
        NSWindowSharingType::None
    } else {
        NSWindowSharingType::ReadOnly
    };
    ns_window.setSharingType(sharing);
}
