use std::sync::{Arc, Mutex};

use crate::{
    components::sidebar::SidebarMessage,
    services::{
        favicon::FaviconMessage,
        sdk::{ClientManager, SessionToken},
    },
    views::{
        about::AboutMessage, export::ExportMessage, fingerprint_phrase::FingerprintMessage,
        generator::GeneratorMessage, import::ImportMessage, login::LoginMessage,
        magnify::MagnifyMessage, new_folder::NewFolderMessage,
        new_item_picker::NewItemPickerMessage, screenshot_confirm::ScreenshotConfirmMessage,
        send::SendMessage, settings::SettingsMessage, title_bar::TitleBarMessage,
        vault::VaultMessage,
    },
};

// ── Top-level Message ──────────────────────────────────────────────────────
// `View` carries sub-view messages (all need `UpdateCtx`, factored into one
// shared construction site). The rest are app-level signals.
//
// `derive_more::From` generates `From<VariantType> for Message` for every
// tuple variant *except* `View(ViewMessage)` — that one's skipped because
// the blanket below already covers `T: Into<ViewMessage>` (and the reflexive
// `From<ViewMessage> for ViewMessage` would otherwise produce a duplicate
// impl). `Window(WindowMessage)` is also skipped because its handler routes
// per-window-id rather than via a single typed conversion.

#[derive(Debug, Clone, derive_more::From)]
pub enum Message {
    SshAgent(crate::services::ssh_agent::UiEvent),
    #[from(skip)]
    SshDecision {
        id: uuid::Uuid,
        decision: crate::services::ssh_agent::Decision,
    },
    #[from(skip)]
    SshFocus {
        id: uuid::Uuid,
        target: crate::services::ssh_agent::Decision,
    },
    /// Results are checked again on the UI thread, after any queued lock/switch.
    #[from(skip)]
    SessionResult {
        session: SessionToken,
        presentation: u64,
        message: Box<Message>,
    },
    #[from(skip)]
    Presentation {
        generation: u64,
        message: Box<Message>,
    },
    #[from(skip)]
    View(ViewMessage),
    About(AboutMessage),
    #[from(skip)]
    Window(WindowMessage),
    #[from(skip)]
    System(SystemMessage),
    /// Sidebar chrome (collapse, section/filter selection). Handled at App
    /// level because it persists across authenticated screens.
    Sidebar(SidebarMessage),
    /// One per completed fetch; arrival alone triggers the redraw.
    Favicon(FaviconMessage),
    /// Routed at the top level (not via `ViewMessage`) because the launcher
    /// owns its own window and can't share `UpdateCtx` with screen views.
    Magnify(MagnifyMessage),
}

/// Sub-view messages, bundled so `App::update` builds `UpdateCtx` in one
/// place and the `&mut App::open_overlay` borrow has a single scope.
#[derive(Debug, Clone, derive_more::From)]
pub enum ViewMessage {
    Login(LoginMessage),
    Vault(VaultMessage),
    Send(SendMessage),
    TitleBar(TitleBarMessage),
    Settings(SettingsMessage),
    Generator(GeneratorMessage),
    Import(ImportMessage),
    Export(ExportMessage),
    NewFolder(NewFolderMessage),
    NewItemPicker(NewItemPickerMessage),
    Fingerprint(FingerprintMessage),
    ScreenshotConfirm(ScreenshotConfirmMessage),
}

/// Chain `XxxMessage -> ViewMessage -> Message` for view sub-messages. With
/// this, `vault_msg.into()` produces a `Message` directly via the per-view
/// `derive_more::From` impls on `ViewMessage` plus this lift, instead of
/// needing two `.into()` calls.
impl<T: Into<ViewMessage>> From<T> for Message {
    fn from(t: T) -> Self {
        Self::View(t.into())
    }
}

/// Per-window OS events. Every variant carries `window::Id` for daemon
/// multi-window dispatch.
#[derive(Debug, Clone)]
pub enum WindowMessage {
    Opened(iced::window::Id),
    GotRawId(iced::window::Id, u64),
    /// Result of inspecting the real native backend before hiding a surface.
    HideBackendResolved {
        id: iced::window::Id,
        wayland: bool,
    },
    /// OS-emitted close (Alt+F4, native X, window-list "Close"). Funnels
    /// into the same `WindowAction::Close` path the custom title-bar X uses,
    /// so close-to-tray applies uniformly. `exit_on_close_request: false`
    /// on the main window defers the real close to our handler.
    CloseRequested(iced::window::Id),
    Closed(iced::window::Id),
    KeyPressed(iced::window::Id, iced::keyboard::Event, iced::event::Status),
    /// Mouse-button down anywhere in a window. Carried separately from
    /// `KeyPressed` so the session-timeout handler can record activity
    /// without us having to materialise a synthetic `keyboard::Event`.
    /// Cursor moves are deliberately *not* included — they fire ~per-frame
    /// while the cursor is in the window and would defeat any timeout.
    MouseInput(iced::window::Id),
    Resized(iced::window::Id, iced::Size),
    /// Used by the session-timeout handler to skip the active+focused
    /// defensive grace, and by Magnify for click-outside-to-dismiss; other
    /// windows ignore the latter.
    Focused(iced::window::Id),
    Unfocused(iced::window::Id),
}

/// Global signals that aren't tied to a specific window.
#[derive(Debug, Clone)]
pub enum SystemMessage {
    /// Muda menu item activated — native app menu and tray context menu
    /// share one global receiver; the handler resolves the id on whichever
    /// handle owns it.
    MudaEvent(muda::MenuEvent),
    /// Tray icon left-click; the pump in [`crate::services::tray::click_stream`]
    /// already filters to the toggle case.
    TrayClick(crate::services::tray::TrayAction),
    /// OS-level light/dark scheme changed. Only acted on under
    /// `ThemePreference::System`.
    ThemeChanged,
    CloseToast(usize),
    /// Background `ClientManager::load` finished. The loader stores the
    /// populated manager in a one-shot slot; App's handler `take()`s it
    /// out. `Arc<Mutex<...>>` because `Message` derives `Clone` and
    /// `ClientManager` isn't `Clone` (its in-memory `sends` /
    /// `password_history` are mutated through `&mut self` and shouldn't
    /// silently fork on a stray message clone).
    ClientManagerLoaded(Arc<Mutex<Option<ClientManager>>>),
    /// Single-instance listener forwarded a "show" signal from a second launch.
    InstanceWakeRequested,
    /// SDK work drained and final key cleanup completed.
    SessionQuiesced(SessionToken),
    /// Durable removal finished or remains pending; delivered after account removal.
    LogoutCompleted {
        operation: uuid::Uuid,
        result: Result<(), crate::services::sdk::LogoutFailure>,
    },
    /// OS session state change (screen lock/unlock, suspend/resume). The
    /// handler locks every vault when any user has
    /// [`UserPreferences::lock_on_system_lock`][crate::services::preferences::UserPreferences::lock_on_system_lock]
    /// enabled.
    SessionEvent(session_events::SessionEvent),
    SyncCompleted {
        uid: crate::domain::UserId,
        operation: uuid::Uuid,
        result: Result<crate::services::sdk::vault_sync::Report, String>,
    },
    /// `services::session_timeout`'s driver task fired — at least one user
    /// has crossed their `lock_after` or `logout_after` deadline. Handler
    /// re-runs the per-user check and applies lock / log-out as needed.
    SessionTimeoutCheck,
}

impl super::ctx::SessionScoped for Message {
    fn for_session(self, session: SessionToken, presentation: u64) -> Self {
        Self::SessionResult {
            session,
            presentation,
            message: Box::new(self),
        }
    }
}
