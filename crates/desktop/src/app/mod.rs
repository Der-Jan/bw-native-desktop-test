//! Top-level application state + iced daemon integration.
//!
//! ## Layout
//!
//! - This file: [`App`] / [`Views`] / [`ThemeState`] / [`ViewCache`] +
//!   their constructors. Public re-exports.
//! - [`lifecycle`] — [`App::new`] (construction) + [`App::subscription`]
//!   (event sources).
//! - [`update`] — [`App::update`] dispatch.
//! - [`view`] — [`App::view`] / [`App::title`] / [`App::theme`] /
//!   [`App::scale_factor`] (iced daemon callbacks).

mod ctx;
mod handlers;
mod helpers;
mod lifecycle;
mod message;
mod update;
mod view;
pub(crate) mod window;

pub use ctx::{Outcome, Overlay, PushToast, RenderCtx, UpdateCtx, View, ViewExt};
pub use message::{Message, SystemMessage, ViewMessage, WindowMessage};

use std::{collections::HashMap, rc::Rc};

use crate::{
    components::{sidebar::SidebarState, toast::Toast},
    domain::{Screen, UserId},
    services::{
        clipboard::ClipboardManager, sdk::AccountEntry, sdk::ClientManager, settings::Settings,
        tray::TrayHandle,
    },
    theme::{AppTheme, ThemePreference},
    views::{
        export as export_view, fingerprint_phrase, generator as generator_view,
        import as import_view, login, magnify, new_folder as new_folder_view,
        new_item_picker as new_item_picker_view, screenshot_confirm, send,
        settings as settings_view, title_bar, vault,
    },
};

use window::WindowInfo;

pub(crate) const MAIN_WINDOW_SIZE: iced::Size = iced::Size::new(1024.0, 800.0);

pub struct App {
    // ── Session ───────────────────────────────────────────────────────────
    pub(crate) active_user: Option<UserId>,
    /// Invalidates queued UI completions across account switches and security transitions.
    pub(crate) presentation_generation: u64,
    pub(crate) storage_warning: Option<crate::services::sdk::StorageWarning>,
    pub(crate) sync_status: std::collections::HashMap<UserId, handlers::vault_sync::SyncUi>,
    pub(crate) pending_logouts: std::collections::HashSet<uuid::Uuid>,
    pub(crate) exit_after_logout: bool,
    pub(crate) client_manager: ClientManager,
    /// Read once at startup. Lifecycle branches re-read `self.settings.<field>`
    /// at event time (never cached) so live changes apply without a restart.
    pub(crate) settings: Settings,

    // ── Navigation ────────────────────────────────────────────────────────
    pub(crate) screen: Screen,
    /// Persists across screen transitions so collapse / filter selection
    /// isn't reset when navigating between Vault and Send.
    pub(crate) sidebar: SidebarState,

    // ── Sub-views ─────────────────────────────────────────────────────────
    pub(crate) views: Views,

    // ── Windows ───────────────────────────────────────────────────────────
    pub(crate) windows: HashMap<iced::window::Id, WindowInfo>,
    /// Current main surface ID; Wayland tray hiding may suspend the surface
    /// and assign a new ID on restore. Application/vault state stays alive.
    pub(crate) main_window: iced::window::Id,
    /// Tracked from `WindowMessage::Focused`/`Unfocused` for the main window.
    /// Defaults to `true` because the process is foreground when launched
    /// (and iced doesn't always emit a `Focused` event for the initial
    /// surface). Consumed by `SessionTimeout::expired` so an active+focused
    /// user is exempt from `lock_after`.
    pub(crate) main_window_focused: bool,
    pub(crate) main_window_hidden: bool,
    pub(crate) main_window_suspended: bool,
    pub(crate) magnify: magnify::MagnifyView,

    // ── Native chrome ─────────────────────────────────────────────────────
    pub(crate) theme: ThemeState,
    /// Rebuilt at startup and on every language change. Source of truth for
    /// both the custom title-bar dropdown and the native muda menu. See
    /// [`crate::services::menu::menu_tree`].
    pub(crate) menu: crate::services::menu::MenuTree,
    pub(crate) native_menu: Option<crate::services::menu::NativeMenuHandle>,
    /// OS-level handle for the main window. Captured by `WindowMessage::GotRawId`
    /// once the surface is up; needed to re-attach the native menu when the
    /// language changes ([`crate::app::App::rebuild_menu`]).
    pub(crate) main_window_raw_id: Option<u64>,
    pub(crate) tray: Option<TrayHandle>,

    // ── Cross-cutting services ────────────────────────────────────────────
    /// Single writer: every clipboard `set` flows through here so the 30 s
    /// auto-clear bookkeeping sees every write.
    pub(crate) clipboard: ClipboardManager,
    pub(crate) ssh_agent: crate::services::ssh_agent::AgentRuntime,
    pub(crate) ssh_approval: Option<std::sync::Arc<crate::services::ssh_agent::Approval>>,
    pub(crate) ssh_approval_window: Option<(iced::window::Id, uuid::Uuid)>,
    pub(crate) shutting_down: bool,
    pub(crate) favicon: crate::services::favicon::FaviconService,
    /// Per-user lock/logout timer driver. Owns the deadline watch,
    /// tick-broadcast, and the spawned tokio task (aborted on drop).
    pub(crate) session_timeout: crate::services::session_timeout::SessionTimeout,

    // ── Transient UI ──────────────────────────────────────────────────────
    /// Source of truth for which dropdown/menu is open. Writing auto-
    /// dismisses any other overlay by construction.
    pub(crate) open_overlay: Option<Overlay>,
    pub(crate) toasts: Vec<Toast>,

    // ── Derived ───────────────────────────────────────────────────────────
    pub(crate) cache: ViewCache,
}

/// Derived data cached across `view()` rebuilds. Refreshed only by handlers
/// that mutate the inputs (login, logout, lock, unlock, user switch, manager
/// load). Cached on App because `view()` returns an `Element<'_, ...>`
/// borrowing `&self` — slices handed to children can't be built inline.
#[derive(Default)]
pub(crate) struct ViewCache {
    pub accounts: Vec<AccountEntry>,
}

/// Sub-views grouped into their own struct so `App::update` can split its
/// borrow: the view-dispatch arm takes `&mut self.views` plus `&mut
/// self.open_overlay` via `UpdateCtx`, while event handlers reborrow other
/// App fields (client_manager, settings, etc.) without overlap.
pub struct Views {
    pub(crate) login: login::LoginView,
    pub(crate) vault: vault::VaultView,
    pub(crate) send: send::SendView,
    pub(crate) settings: settings_view::SettingsView,
    pub(crate) generator: generator_view::GeneratorView,
    pub(crate) import: import_view::ImportView,
    pub(crate) export: export_view::ExportView,
    pub(crate) new_folder: new_folder_view::NewFolderView,
    pub(crate) new_item_picker: new_item_picker_view::NewItemPickerView,
    pub(crate) title_bar: title_bar::TitleBarView,
    /// Account → Fingerprint phrase modal. Closed unless the user explicitly
    /// opened it via the menu.
    pub(crate) fingerprint: fingerprint_phrase::FingerprintModal,
    /// Settings → Allow screenshots: post-toggle "is the window still
    /// visible?" dialog with auto-revert. Closed unless the user just
    /// enabled screen-capture protection.
    pub(crate) screenshot_confirm: screenshot_confirm::ScreenshotConfirmModal,
}

impl Views {
    pub fn new() -> Self {
        Self {
            login: login::LoginView::new(),
            vault: vault::VaultView::new(),
            send: send::SendView::new(),
            settings: settings_view::SettingsView::new(),
            generator: generator_view::GeneratorView::new(),
            import: import_view::ImportView::new(),
            export: export_view::ExportView::new(),
            new_folder: new_folder_view::NewFolderView::new(),
            new_item_picker: new_item_picker_view::NewItemPickerView::default(),
            title_bar: title_bar::TitleBarView::new(),
            fingerprint: fingerprint_phrase::FingerprintModal::default(),
            screenshot_confirm: screenshot_confirm::ScreenshotConfirmModal::default(),
        }
    }
}

/// Theme state bundle: user preference, resolved `AppTheme` instance,
/// and the OS observer we subscribe to for system-theme changes. The
/// observer is `None` on platforms where construction failed (headless
/// CI, sandboxed installs); the resolver falls back to a static theme
/// when the OS scheme can't be queried.
pub struct ThemeState {
    pub(crate) preference: ThemePreference,
    pub(crate) current: AppTheme,
    pub(crate) system: Option<Rc<system_theme::SystemTheme>>,
}

impl ThemeState {
    pub fn new(preference: ThemePreference) -> Self {
        let system = match system_theme::SystemTheme::new() {
            Ok(s) => Some(Rc::new(s)),
            Err(err) => {
                tracing::warn!(%err, "failed to init system theme observer; using static fallback");
                None
            }
        };
        let current = preference.resolve(system.as_deref());
        Self {
            preference,
            current,
            system,
        }
    }

    pub fn refresh(&mut self) {
        if self.preference == ThemePreference::System {
            self.current = self.preference.resolve(self.system.as_deref());
        }
    }
}
