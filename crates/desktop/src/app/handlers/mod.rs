//! App-level event handlers — window/system/menu/tray dispatch, plus
//! cross-view handlers for chrome that doesn't belong to any single view
//! (sidebar, account switcher). Per-view handlers live in
//! `views/<name>/handler.rs`.

mod account_switcher;
mod export;
mod fingerprint;
mod import;
pub(super) mod magnify;
#[cfg(target_os = "macos")]
mod magnify_macos_fix;
mod platform;
mod screenshot_confirm;
mod sidebar;

pub(crate) mod vault_sync;

mod ssh_agent;
