//! Platform + stateful cross-view services.
//!
//! Constructed once in `App::new`, consumed by views via `UpdateCtx`. Services
//! may not import from `views/`, `components/`, or `app/`.

pub mod accounts;
pub mod broadcast_stream;
pub mod clipboard;
pub mod cursor_monitor;
pub mod favicon;
pub mod global_hotkey;
pub mod i18n;
pub mod instance_lock;
pub mod menu;
pub mod preferences;
pub mod screenshot_protection;
pub mod sdk;
pub mod search;
pub mod session_timeout;
pub mod settings;
pub mod tray;

pub mod vault_sort;

pub mod vault_navigation;

pub mod ssh_agent;
