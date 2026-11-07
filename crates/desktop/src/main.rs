#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod components;
mod debug_fmt;
mod domain;
mod paths;
mod services;
mod theme;
mod views;

#[cfg(test)]
mod test_support;

/// Convenience wrapper around [`i18n_embed_fl::fl!`] that passes our static
/// [`i18n::LANGUAGE_LOADER`] implicitly. Usage:
///
/// ```ignore
/// fl!("login-unlock-title")
/// fl!("login-server-accessing", server = "bitwarden.com")
/// ```
///
/// Unknown message IDs or wrong argument names are a **compile error** — the
/// macro validates against the `.ftl` files under `i18n/` at build time.
#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        ::i18n_embed_fl::fl!($crate::services::i18n::LANGUAGE_LOADER, $message_id)
    }};
    ($message_id:literal, $($args:expr),*) => {{
        ::i18n_embed_fl::fl!($crate::services::i18n::LANGUAGE_LOADER, $message_id, $($args),*)
    }};
}

use iced::{
    Font,
    font::{Family, Weight},
};

use crate::{app::App, components::icons};

/// App-wide default font (Inter 18pt Medium).
pub const APP_FONT: Font = Font {
    family: Family::Name("Inter 18pt"),
    weight: Weight::Medium,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub const APP_FONT_BOLD: Font = Font {
    weight: Weight::Bold,
    ..APP_FONT
};

fn main() -> iced::Result {
    init_tracing();
    services::i18n::init();
    if let Err(error) = paths::initialize() {
        eprintln!("{error}");
        std::process::exit(2);
    }

    // Required by `Client::load_from_state` (called by the SDK loader on each
    // user), which reads `get_host_platform_info()` when constructing
    // `ClientSettings`. Must be set once before any SDK client construction.
    bitwarden_core::init_host_platform_info(bitwarden_core::HostPlatformInfo {
        user_agent: "Bitwarden Rust-SDK".to_string(),
        device_type: bitwarden_core::DeviceType::SDK,
        device_identifier: None,
        bitwarden_client_version: Some(
            services::accounts::transport::BITWARDEN_PROTOCOL_VERSION.into(),
        ),
        bitwarden_package_type: None,
    });

    // Single-instance guard: if another process is already running, tell it
    // to surface its window and exit. Otherwise take over as the primary —
    // `wake_stream` will bind the listener once iced's tokio runtime is up.
    if services::instance_lock::notify_primary_if_running() {
        tracing::info!("Bitwarden Desktop already running; signalled primary. Exiting.");
        return Ok(());
    }
    services::instance_lock::cleanup_stale_socket();

    tracing::info!("Starting app with Iced backend: tiny-skia");

    iced::daemon(App::new, App::update, App::view)
        .settings(iced::Settings {
            // Maps to `WM_CLASS` on X11 and `app_id` on Wayland so the running
            // window matches the `.desktop` file (and its icon). Kept in sync
            // with `Packager.toml`'s `identifier`. Ignored on Windows + macOS.
            id: Some("com.bitwarden.desktop.next".into()),
            fonts: vec![
                assets::FONT_MEDIUM.into(),
                assets::FONT_BOLD.into(),
                assets::BWI_FONT.into(),
                icons::FONT_BYTES.into(),
            ],
            default_font: APP_FONT,
            antialiasing: true,
            ..Default::default()
        })
        .subscription(App::subscription)
        .title(App::title)
        .theme(App::theme)
        .scale_factor(App::scale_factor)
        .run()
}

/// Install a `tracing` subscriber driven by the `RUST_LOG` env var. SDK crates
/// emit `#[tracing::instrument]` spans (unlock, crypto init, vault decrypt) so
/// once this runs they become visible under e.g.
/// `RUST_LOG=bitwarden_desktop_next=debug,bitwarden_core=debug cargo run`.
fn init_tracing() {
    use tracing_subscriber::{
        EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt,
    };

    // Flight-recorder layer keeps recent events in an in-memory circular
    // buffer that `bitwarden_logging::read_flight_recorder()` can dump for
    // diagnostics — independent of the stderr filter below.
    let flight_recorder =
        bitwarden_logging::init_flight_recorder(bitwarden_logging::FlightRecorderConfig::default());

    // `EnvFilter::from_default_env()` alone falls back to `LevelFilter::ERROR`
    // when `RUST_LOG` is unset, which would silence our `warn!` / `debug!`.
    const DEFAULT_FILTER: &str = "bitwarden_desktop_next=debug,warn";
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_writer(std::io::stderr)
        .with_filter(filter);

    tracing_subscriber::registry()
        .with(flight_recorder)
        .with(fmt_layer)
        .init();
}
