#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod assets;
mod core;
mod domain;
mod paths;
mod services;
#[cfg(test)]
mod test_support;
mod theme;
mod ui;

#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        ::i18n_embed_fl::fl!($crate::services::i18n::LANGUAGE_LOADER, $message_id)
    }};
    ($message_id:literal, $($args:expr),*) => {{
        ::i18n_embed_fl::fl!($crate::services::i18n::LANGUAGE_LOADER, $message_id, $($args),*)
    }};
}

fn main() -> eframe::Result {
    init_tracing();
    services::i18n::init();
    if let Err(error) = paths::initialize() {
        eprintln!("{error}");
        std::process::exit(2);
    }
    bitwarden_core::init_host_platform_info(bitwarden_core::HostPlatformInfo {
        user_agent: "Bitwarden Rust-SDK".into(),
        device_type: bitwarden_core::DeviceType::SDK,
        device_identifier: None,
        bitwarden_client_version: Some(
            services::accounts::transport::BITWARDEN_PROTOCOL_VERSION.into(),
        ),
        bitwarden_package_type: None,
    });
    let _startup_guard = match services::instance_lock::startup_guard() {
        Ok(Some(guard)) => guard,
        Ok(None) => {
            services::instance_lock::notify_primary_if_running();
            return Ok(());
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    if services::instance_lock::notify_primary_if_running() {
        return Ok(());
    }
    services::instance_lock::cleanup_stale_socket();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("background runtime");
    let _entered = runtime.enter();
    ui::window_loop::run()
}

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
