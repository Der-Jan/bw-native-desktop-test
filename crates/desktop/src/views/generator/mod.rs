//! Generator modal — Password / Passphrase / Username tabs plus an in-memory
//! history panel.
//!
//! Opens via View → Generator (Ctrl/Cmd+G) and View → Generator history.
//! Any form-field change auto-regenerates: the view mutates local state,
//! emits [`GeneratorEvent::Generate`], and App dispatches the SDK call via
//! `ClientExt::generate_password` / `_passphrase` / `_username`.
//!
//! Every generated value is appended to the user's `password_history` on
//! `ClientManager`; the `Generated` result carries back the fresh history
//! snapshot so the view can rebind without a second round-trip.
//!
//! Closes via the X button, backdrop click, or Escape — same scaffolding as
//! the Settings modal ([`crate::views::settings`]).
//!
//! ## Layout
//!
//! - [`state`] — `Mode`, `TabKind`, `UsernameKind`, per-tab form structs and
//!   their SDK request adapters; numeric-input parse helpers.
//! - [`message`] — [`GeneratorMessage`] / [`GeneratorEvent`] / [`GenerateKind`].
//! - [`update`] — `update()` dispatch.
//! - [`view`] — `modal_view` and the supporting render helpers.

mod handler;
mod history;
mod message;
mod state;
mod tabs;
mod update;
mod view;

use crate::{components, services::sdk::PasswordHistoryEntry};

pub use message::{GenerateKind, GeneratorEvent, GeneratorMessage};
pub use state::{TabKind, UsernameKind};

pub use self::state::Mode;
use self::state::{PassphraseForm, PasswordForm, UsernameForm};

pub struct GeneratorView {
    pub(super) fade: components::FadeInOut,
    mode: Mode,
    active_tab: TabKind,
    password: PasswordForm,
    passphrase: PassphraseForm,
    username: UsernameForm,
    /// Latest generated value for the active tab. `None` until the first
    /// successful generation after open / tab switch.
    current: Option<String>,
    /// Cached snapshot of `ClientManager::password_history` for the active
    /// user. App refreshes this on modal open and after every successful
    /// `Generated` result.
    history: Vec<PasswordHistoryEntry>,
}

impl GeneratorView {
    pub fn new() -> Self {
        Self {
            fade: components::FadeInOut::default(),
            mode: Mode::Generator,
            active_tab: TabKind::Password,
            password: PasswordForm::default(),
            passphrase: PassphraseForm::default(),
            username: UsernameForm::default(),
            current: None,
            history: Vec::new(),
        }
    }

    pub fn open(&mut self, mode: Mode) {
        self.fade.open();
        self.mode = mode;
        if matches!(mode, Mode::Generator) {
            self.active_tab = TabKind::Password;
            self.current = None;
        }
    }

    /// Replace the cached history snapshot. App calls this on modal open
    /// and after `ClearHistory` so the panel reflects storage.
    pub fn set_history(&mut self, history: Vec<PasswordHistoryEntry>) {
        self.history = history;
    }

    /// Build the regeneration request for the currently-active tab.
    /// Exposed to App so the menu-open path can dispatch an initial
    /// generation without going through the update loop.
    pub fn current_request(&self) -> GenerateKind {
        match self.active_tab {
            TabKind::Password => GenerateKind::Password(self.password.to_request()),
            TabKind::Passphrase => GenerateKind::Passphrase(self.passphrase.to_request()),
            TabKind::Username => GenerateKind::Username(self.username.to_request()),
        }
    }
}

impl Default for GeneratorView {
    fn default() -> Self {
        Self::new()
    }
}
