//! macOS magnify-window rounded clipping for tiny-skia.
//!
//! Softbuffer presents a CGImage without an alpha channel. Clip the
//! contentView's layer to the window's rounded shape so its corners remain
//! transparent even though the framebuffer itself cannot supply that alpha.

#![cfg(target_os = "macos")]

use iced::window::raw_window_handle::RawWindowHandle;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_quartz_core::CALayer;

/// Apply rounded clipping to the magnify window's contentView. Safe to call
/// multiple times — every setter touched is idempotent.
pub fn apply(window: &dyn iced::window::Window, corner_radius: f32) {
    let Ok(window_handle) = window.window_handle() else {
        tracing::warn!("magnify_macos_fix: failed to acquire window handle");
        return;
    };
    let RawWindowHandle::AppKit(handle) = window_handle.as_raw() else {
        tracing::warn!("magnify_macos_fix: not an AppKit window handle");
        return;
    };

    // SAFETY: `AppKitWindowHandle::ns_view` is documented as a valid pointer
    // to an NSView (winit's contentView). `iced::window::run` invokes us on
    // the main thread, which is required for AppKit access.
    let view: Retained<NSView> =
        unsafe { Retained::retain(handle.ns_view.as_ptr().cast()).expect("ns_view was null") };
    let Some(layer) = view.layer() else {
        tracing::warn!("magnify_macos_fix: NSView has no backing layer");
        return;
    };

    apply_corner_fix(&layer, corner_radius);
}

/// `cornerRadius` + `masksToBounds` on the contentView's layer. The OS
/// applies the rounded clip to all sublayers automatically.
fn apply_corner_fix(layer: &CALayer, corner_radius: f32) {
    layer.setCornerRadius(corner_radius as _);
    layer.setMasksToBounds(true);
}
