use std::future::Future;

use crate::services::sdk::SessionClient;
use iced::Task;

use crate::{
    components::toast::Toast,
    domain::{Screen, UserId},
    services::{clipboard::Sensitivity, sdk::AccountEntry},
};

use super::{App, Message, ctx::SessionScoped, window::WindowKind};

impl App {
    pub(crate) fn main_window_id(&self) -> iced::window::Id {
        self.main_window
    }

    pub(crate) fn about_window_id(&self) -> Option<iced::window::Id> {
        self.windows
            .iter()
            .find(|(_, w)| w.kind == WindowKind::About)
            .map(|(id, _)| *id)
    }

    pub(crate) fn main_window_maximized(&self) -> bool {
        self.windows
            .get(&self.main_window)
            .is_some_and(|w| w.maximized)
    }

    pub(crate) fn push_toast(&mut self, toast: crate::components::toast::Toast) {
        self.toasts.push(toast);
    }

    /// Standard "user copied something" sequence: push to clipboard, show a
    /// success toast, and minimise the main window if the active user has
    /// `minimize_on_copy` set. Used by the vault, send, and generator copy
    /// handlers — anywhere the main window is what the user is looking at.
    /// The Magnify launcher uses `clipboard.copy(...)` directly because its
    /// own window already hides itself on the same keystroke, and the main
    /// window typically isn't focused.
    pub(crate) fn copy_and_toast(
        &mut self,
        value: String,
        sensitivity: Sensitivity,
        toast_label: String,
    ) -> Task<Message> {
        self.clipboard.copy(value, sensitivity);
        self.push_toast(Toast::success(toast_label, None));
        self.minimize_after_copy_task()
    }

    /// If the active user has the `minimize_on_copy` preference set, return
    /// a `window::minimize` task for the main window. Otherwise `Task::none()`.
    /// Reusable on its own for clipboard call sites that need different
    /// toast wording (or no toast at all); most call sites should prefer
    /// [`Self::copy_and_toast`].
    pub(crate) fn minimize_after_copy_task(&self) -> Task<Message> {
        let minimize = self
            .active_user
            .as_ref()
            .is_some_and(|uid| self.settings.preferences_for(uid).minimize_on_copy);
        if minimize {
            iced::window::minimize(self.main_window_id(), true)
        } else {
            Task::none()
        }
    }

    /// Bundle the most common SDK call shape: extract the active user's
    /// `SessionClient`, run an async call against it, and route the
    /// result through `on_complete`. `Task::none()` if no active user or the
    /// user isn't loaded — the same defensive bail every handler used to
    /// open with by hand.
    ///
    /// `spawn` runs sync once with the cloned client and returns the future;
    /// the future is `Task::perform`'d. `on_complete` receives the active
    /// uid alongside the result so call sites that need it (export, magnify)
    /// don't have to capture it twice.
    pub(crate) fn perform_with_active_client<Spawn, Fut, T>(
        &self,
        spawn: Spawn,
        on_complete: impl Fn(UserId, T) -> Message + Send + 'static,
    ) -> Task<Message>
    where
        Spawn: FnOnce(SessionClient) -> Fut,
        Fut: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let Some(uid) = self.active_user else {
            return Task::none();
        };
        self.perform_with_client(uid, spawn, move |t| on_complete(uid, t))
    }

    /// Variant of [`Self::perform_with_active_client`] for callers that
    /// already have a specific uid in hand (e.g. captured from a message
    /// payload, so we shouldn't re-read `self.active_user`). Same defensive
    /// `Task::none()` bail when the user isn't loaded.
    pub(crate) fn perform_with_client<Spawn, Fut, T>(
        &self,
        uid: UserId,
        spawn: Spawn,
        on_complete: impl Fn(T) -> Message + Send + 'static,
    ) -> Task<Message>
    where
        Spawn: FnOnce(SessionClient) -> Fut,
        Fut: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let Some(client) = self.client_manager.client_for(&uid) else {
            return Task::none();
        };
        let session = client.session();
        let presentation = self.presentation_generation;
        Task::perform(spawn(client), move |result| {
            on_complete(result).for_session(session.clone(), presentation)
        })
    }

    pub(crate) fn active_account_entry(&self) -> Option<&AccountEntry> {
        self.active_user
            .as_ref()
            .and_then(|uid| self.cache.accounts.iter().find(|a| &a.user_id == uid))
    }

    /// Repopulate the accounts snapshot + resync the native menu's enabled
    /// state. Called from handlers that mutate SDK-side user state (login,
    /// logout, lock, unlock, user switch, manager load).
    pub(crate) fn refresh_accounts_cache(&mut self) {
        self.cache.accounts = self.client_manager.accounts();

        // Precompute `MenuState` ahead of the mutable native-menu borrow so
        // the borrow checker accepts the two field accesses side-by-side.
        let state = self.menu_state();
        if let Some(ref mut handle) = self.native_menu {
            handle.sync_dynamic(&self.cache.accounts);
            handle.sync_enabled(&state);
        }
    }

    /// Assign a new screen plus standard transition bookkeeping: drop the
    /// previous overlay, resync the accounts cache + native menu state.
    /// Callers do view-specific pre-work (e.g. `show_unlock_for`) first.
    pub(crate) fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
        self.open_overlay = None;
        self.refresh_accounts_cache();
    }

    /// Lock a user's keystore. If `uid` is the active user, also transitions
    /// to the login screen with their unlock prompt; otherwise the call is
    /// silent and the lock is reflected next time the user is selected.
    ///
    /// Does **not** unenroll from `session_timeout` — `logout_after` keeps
    /// ticking while the user is locked. The deadline is recomputed because
    /// `lock_after` is no longer relevant for this user.
    pub(crate) fn lock_user(&mut self, uid: &UserId) -> Task<Message> {
        let cleanup = self.client_manager.lock(uid);
        self.views.vault.remove_user_items(uid);
        self.views.send.remove_user_items(uid);
        self.favicon.evict_user(uid);
        self.refresh_session_timeout_deadline();
        let route = if self.active_user.as_ref() == Some(uid) {
            self.clear_sensitive_ui();
            self.show_login_for_active()
        } else {
            Task::none()
        };
        Task::batch([Self::finish_session_cleanup(cleanup), route])
    }

    /// Modal-open preamble shared across the four `open_*_modal` helpers:
    /// returns the active user (or `None` to bail) and clears any other
    /// overlay so a stale dropdown doesn't sit behind the new modal.
    pub(crate) fn require_active_user_and_close_overlay(&mut self) -> Option<UserId> {
        let uid = self.active_user?;
        if !self.client_manager.is_unlocked(&uid) {
            return None;
        }
        self.open_overlay = None;
        Some(uid)
    }

    /// Lock every user whose `lock_on_system_lock` preference is set. Called
    /// from the OS-session-event handler on `Locked` / `Suspended`.
    pub(crate) fn lock_system_lock_users(&mut self) -> Task<Message> {
        let uids: Vec<UserId> = self
            .settings
            .user_preferences
            .iter()
            .filter(|(_, p)| p.lock_on_system_lock)
            .map(|(uid, _)| *uid)
            .collect();
        Task::batch(
            uids.iter()
                .map(|uid| self.lock_user(uid))
                .collect::<Vec<_>>(),
        )
    }

    /// Snapshot every signed-in user for the session-timeout module: pairs
    /// each `UserId` with their preferences and current unlock state.
    pub(crate) fn session_timeout_snapshots(
        &self,
    ) -> Vec<crate::services::session_timeout::UserSnapshot> {
        self.client_manager
            .user_ids()
            .into_iter()
            .map(|uid| crate::services::session_timeout::UserSnapshot {
                uid,
                prefs: self.settings.preferences_for(&uid),
                is_unlocked: self.client_manager.is_unlocked(&uid),
            })
            .collect()
    }

    /// Recompute and push the soonest deadline. Called on every transition
    /// that may shift it: input event (after the throttle), unlock, lock,
    /// log-out, settings change, user switch, suspend/resume, focus change.
    pub(crate) fn refresh_session_timeout_deadline(&self) {
        let snaps = self.session_timeout_snapshots();
        self.session_timeout.recompute_and_push_deadline(
            &snaps,
            self.active_user.as_ref(),
            self.main_window_focused,
        );
    }

    /// Record an input event for the active user, then recompute the
    /// deadline if the throttle didn't suppress the bump.
    pub(crate) fn record_session_activity(&mut self) {
        let Some(uid) = self.active_user else {
            return;
        };
        if self.session_timeout.record_activity(uid) {
            self.refresh_session_timeout_deadline();
        }
    }

    /// Inline cleanup for a single user log-out. Same shape as the body of
    /// [`Self::handle_log_out`] but without the active-user-transition
    /// step, so callers that want to log out a non-active user (or batch
    /// multiple) don't trip the screen swap.
    pub(crate) fn log_out_user(&mut self, uid: &UserId) -> Task<Message> {
        self.sync_status.remove(uid);
        let cleanup = self.client_manager.log_out(uid);
        let removed_preferences = self.settings.user_preferences.remove(uid).is_some();
        let removed_filter = self.settings.vault_filters.remove(uid).is_some();
        if removed_preferences || removed_filter {
            self.settings.save();
        }
        self.views.vault.remove_user_items(uid);
        self.views.send.remove_user_items(uid);
        self.favicon.evict_user(uid);
        self.session_timeout.unenroll(uid);
        if self.active_user.as_ref() == Some(uid) {
            self.clear_sensitive_ui();
        }
        match cleanup {
            Some(cleanup) => {
                let operation = uuid::Uuid::new_v4();
                self.pending_logouts.insert(operation);
                Task::perform(cleanup.finish(), move |result| {
                    Message::System(super::SystemMessage::LogoutCompleted { operation, result })
                })
            }
            None => Task::none(),
        }
    }

    /// Let durable removals finish before normal application shutdown.
    pub(crate) fn request_exit(&mut self) -> Task<Message> {
        self.shutting_down = true;
        self.ssh_agent.stop();
        let cancel = self.cancel_ssh_approval();
        if !self.pending_logouts.is_empty() {
            self.exit_after_logout = true;
            cancel
        } else {
            Task::batch([cancel, iced::exit()])
        }
    }

    pub(crate) fn finish_session_cleanup(
        cleanup: Option<crate::services::sdk::LockOperation>,
    ) -> Task<Message> {
        match cleanup {
            Some(cleanup) => Task::perform(cleanup.finish(), |session| {
                Message::System(super::SystemMessage::SessionQuiesced(session))
            }),
            None => Task::none(),
        }
    }

    /// Discard secret-bearing UI immediately, without retaining an outro frame.
    pub(crate) fn clear_sensitive_ui(&mut self) {
        self.client_manager.clear_grants();
        self.views.login.cancel_login();
        self.presentation_generation = self
            .presentation_generation
            .checked_add(1)
            .expect("presentation generation exhausted");
        self.open_overlay = None;
        self.toasts.clear();
        self.clipboard.clear_owned();
        self.magnify_reset_sticky();
        self.magnify.mode = crate::views::magnify::Mode::Locked;
        if let Some(uid) = self.active_user {
            self.views
                .vault
                .reset(&uid, self.sidebar.active_vault_filter);
            self.views.send.reset(&uid, self.sidebar.active_send_filter);
        }
        self.views.login = crate::views::login::LoginView::new();
        self.views.generator = crate::views::generator::GeneratorView::new();
        self.views.export = crate::views::export::ExportView::new();
        self.views.import = crate::views::import::ImportView::new();
        self.views.new_folder = crate::views::new_folder::NewFolderView::new();
        self.views.new_item_picker = Default::default();
        self.views.fingerprint = Default::default();
        self.views.screenshot_confirm = Default::default();
    }

    /// Log out a specific user from the File → Log out per-account submenu.
    /// Routes to the active-user transition via `handle_log_out` when the
    /// uid matches, else does an inline cleanup and refreshes caches.
    pub(crate) fn handle_log_out_for(&mut self, uid: UserId) -> Task<Message> {
        if self.active_user.as_ref() == Some(&uid) {
            self.handle_log_out()
        } else {
            let cleanup = self.log_out_user(&uid);
            self.refresh_accounts_cache();
            self.refresh_session_timeout_deadline();
            cleanup
        }
    }

    /// Apply session-timeout actions for every signed-in user. Pure
    /// classification lives on [`SessionTimeout::plan_timeout_actions`];
    /// this method only sequences the side effects the plan implies.
    pub(crate) fn run_session_timeout_check(&mut self) -> Task<Message> {
        let snaps = self.session_timeout_snapshots();
        let plan = self.session_timeout.plan_timeout_actions(
            &snaps,
            self.active_user.as_ref(),
            self.main_window_focused,
        );

        // Inline logouts run first so `handle_log_out`'s next-active-user
        // hand-off only sees the post-cleanup `client_manager`.
        let mut tasks: Vec<Task<Message>> = plan
            .inline_logouts
            .iter()
            .map(|uid| self.log_out_user(uid))
            .collect();
        if plan.handle_active_logout {
            tasks.push(self.handle_log_out());
        }
        for uid in plan.locks {
            tasks.push(self.lock_user(&uid));
        }

        self.refresh_session_timeout_deadline();
        Task::batch(tasks)
    }

    /// Standard route-to-login transition: drop sticky Magnify state, point
    /// the login view at the active user's unlock prompt, switch to
    /// [`Screen::Login`], and return the auto-focus task. Used after lock,
    /// after the initial `ClientManager` load, after switching to a locked
    /// user, and from the Magnify launcher when it routes back to the main
    /// window.
    pub(crate) fn show_login_for_active(&mut self) -> Task<Message> {
        self.magnify_reset_sticky();
        self.views
            .login
            .show_unlock_for(self.active_user.as_ref(), &self.client_manager);
        self.set_screen(Screen::Login);
        self.views.login.auto_focus_task().map(Into::into)
    }

    /// Standard post-unlock transition: load both the vault list and the
    /// send list, plus the delayed auto-focus task. Used after a successful
    /// unlock and after switching to an already-unlocked user; keeping the
    /// three-task batch in one place stops the unlock and switch paths from
    /// drifting (e.g. one path forgetting to reload the Send list).
    pub(crate) fn switch_to_vault_task(&self, uid: UserId) -> Task<Message> {
        Task::batch([
            self.load_vault_list_task(uid),
            self.load_send_list_task(uid),
            crate::views::vault::VaultView::delayed_auto_focus_task().map(Into::into),
        ])
    }

    /// Lift `VaultView::load_list_task` into a top-level `Task<Message>`,
    /// hiding the per-call-site `.map(Into::into)`.
    pub(crate) fn load_vault_list_task(&self, uid: UserId) -> Task<Message> {
        let Some(session) = self.client_manager.session_for(&uid) else {
            return Task::none();
        };
        let presentation = self.presentation_generation;
        crate::views::vault::VaultView::load_list_task(uid, &self.client_manager)
            .map(move |m| Message::from(m).for_session(session.clone(), presentation))
    }

    pub(crate) fn load_send_list_task(&self, uid: UserId) -> Task<Message> {
        let Some(session) = self.client_manager.session_for(&uid) else {
            return Task::none();
        };
        let presentation = self.presentation_generation;
        crate::views::send::SendView::load_list_task(uid, &self.client_manager)
            .map(move |m| Message::from(m).for_session(session.clone(), presentation))
    }

    /// Route Edit-menu copying through the same ID-bound SDK operation as
    /// the list buttons; TOTP continues through the detail handler.
    /// No-op when the user isn't on the Vault screen or no cipher is selected
    /// (matches the official client — the menu fires regardless and the
    /// vault decides whether there's anything to copy).
    pub(crate) fn copy_from_active_selection(
        &self,
        msg: crate::views::vault::widgets::cipher_detail::CipherDetailMessage,
    ) -> Task<Message> {
        if self.screen != Screen::Vault {
            return Task::none();
        }
        use crate::views::vault::widgets::{
            cipher_detail::CipherDetailMessage, item_list::ItemListMessage,
        };
        let Some(id) = self.views.vault.selected_id() else {
            return Task::none();
        };
        let message = match msg {
            CipherDetailMessage::CopyUsername => {
                crate::views::vault::VaultMessage::ItemList(ItemListMessage::CopyUsername(id))
            }
            CipherDetailMessage::CopyPassword => {
                crate::views::vault::VaultMessage::ItemList(ItemListMessage::CopyPassword(id))
            }
            other => crate::views::vault::VaultMessage::CipherDetail(other),
        };
        Task::done(message.into())
    }

    /// Rebuild the cached menu tree and re-attach the native menu so all
    /// labels pick up the active language. Call after every
    /// `i18n::set_language` / `i18n::init` invocation.
    pub(crate) fn rebuild_menu(&mut self) {
        self.menu = crate::services::menu::menu_tree();
        if let Some(raw_id) = self.main_window_raw_id {
            // Re-attach replaces the OS-level menu pointer; the previous
            // muda::Menu leak is harmless (the OS released it via
            // SetMenu/mainMenu replacement). The new handle's
            // `dynamic_submenus` start empty — populate from current state.
            self.native_menu = crate::services::menu::attach_menu(raw_id, &self.menu);
            let state = self.menu_state();
            if let Some(ref mut handle) = self.native_menu {
                handle.sync_dynamic(&self.cache.accounts);
                handle.sync_enabled(&state);
            }
        }
    }

    pub(crate) fn menu_state(&self) -> crate::services::menu::MenuState {
        let has_accounts = self.client_manager.has_users();
        let is_locked = self
            .active_user
            .as_ref()
            .map(|uid| !self.client_manager.is_unlocked(uid))
            .unwrap_or(true);
        let has_lockable = self.client_manager.has_unlocked_users();

        crate::services::menu::MenuState {
            is_locked,
            has_accounts,
            has_lockable_accounts: has_lockable,
        }
    }
}

// `Outcome::dispatch` reaches `push_toast` through the `PushToast` trait.
// The inherent method above is kept for direct `self.push_toast(...)`
// calls in handlers.
impl crate::app::PushToast for App {
    fn push_toast(&mut self, toast: crate::components::toast::Toast) {
        App::push_toast(self, toast);
    }

    fn task_scope(&self) -> Option<(crate::services::sdk::SessionToken, u64)> {
        Some((
            self.client_manager
                .session_for(self.active_user.as_ref()?)?,
            self.presentation_generation,
        ))
    }
}

// ── Platform-specific window settings ─────────────────────────────────────
// `iced::daemon` doesn't take a `.window(Settings)` — boot creates the window
// via `window::open` and passes this through.

pub(crate) fn main_window_platform_specific() -> iced::window::settings::PlatformSpecific {
    #[cfg(target_os = "windows")]
    {
        use iced::window::settings::PlatformSpecific;
        PlatformSpecific {
            undecorated_shadow: true,
            corner_preference: iced::window::settings::platform::CornerPreference::Round,
            ..Default::default()
        }
    }

    #[cfg(target_os = "macos")]
    {
        use iced::window::settings::PlatformSpecific;
        PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true,
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        iced::window::settings::PlatformSpecific::default()
    }
}
