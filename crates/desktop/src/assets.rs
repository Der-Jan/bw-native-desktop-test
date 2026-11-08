//! Compile-time embedded assets. All binary resources in one place.

#[cfg(test)]
pub const FONT_MEDIUM: &[u8] = include_bytes!("../../../assets/Inter/static/Inter_18pt-Medium.ttf");
#[cfg(test)]
pub const FONT_BOLD: &[u8] = include_bytes!("../../../assets/Inter/static/Inter_18pt-Bold.ttf");
pub const ICON_PNG: &[u8] = include_bytes!("../../../assets/icon.png");

/// Matches cargo-packager's desktop file basename so Wayland resolves our icon.
pub const DESKTOP_APP_ID: &str = "bitwarden-desktop-next";

/// Decode through our explicit codecs without enabling the image crate's default codecs.
pub fn window_icon() -> Option<std::sync::Arc<eframe::egui::IconData>> {
    static ICON: std::sync::OnceLock<Option<std::sync::Arc<eframe::egui::IconData>>> =
        std::sync::OnceLock::new();
    ICON.get_or_init(|| {
        let rgba = image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png)
            .ok()?
            .into_rgba8();
        let (width, height) = rgba.dimensions();
        Some(std::sync::Arc::new(eframe::egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        }))
    })
    .clone()
}

// Favicon fallback — rendered when a login cipher has no URI, the fetch is
// still pending, or the icon service returned an error.
pub const BWI_GLOBE_PNG: &[u8] = include_bytes!("../../../assets/bwi-globe.png");

// Original outlined Bitwarden wordmark; independent of UI/system fonts.
pub const PASSWORD_MANAGER_LOGO: &[u8] =
    include_bytes!("../../../assets/password-manager-logo.svg");

// Tray icon — per-OS for the best asset (ICO on Windows for multi-res,
// template PNG on macOS; Linux provides a symbolic SVG and monochrome fallback).
#[cfg(target_os = "windows")]
pub const TRAY_ICON: &[u8] = include_bytes!("../../../assets/tray/icon.ico");
#[cfg(target_os = "macos")]
pub const TRAY_ICON: &[u8] = include_bytes!("../../../assets/tray/icon-template.png");
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub const TRAY_ICON: &[u8] = include_bytes!("../../../assets/tray/icon.png");
