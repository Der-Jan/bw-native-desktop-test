use iced::Task;

use crate::{
    app::{App, Message},
    domain::Screen,
    views::login::LoginEvent,
};

impl App {
    pub(crate) fn handle_login_event(&mut self, event: LoginEvent) -> Task<Message> {
        match event {
            LoginEvent::SyncRequested => self.start_vault_sync(),
            LoginEvent::Unlocked { uid } | LoginEvent::LoggedIn { uid } => {
                // `LoginView::update`'s `UnlockCompleted` / `LoginCompleted`
                // arms have already verified `uid == active_user` — events
                // run synchronously after the view's update inside one
                // `App::update` call, so there's no window for the active
                // user to change between then and now.
                //
                // Stamp `last_activity` so neither timer has a retroactive
                // head-start — the throttle path doesn't matter here since
                // there's no prior entry to suppress against.
                if self.active_user != Some(uid) {
                    self.clear_sensitive_ui();
                    self.active_user = Some(uid);
                    self.restore_active_filter();
                }
                self.refresh_accounts_cache();
                self.session_timeout.record_activity(uid);
                self.refresh_session_timeout_deadline();
                if self.client_manager.is_remote(&uid) {
                    let sync = self.start_vault_sync();
                    if self.client_manager.last_sync(&uid).is_some() {
                        self.set_screen(Screen::Vault);
                        return Task::batch([self.load_vault_list_task(uid), sync]);
                    }
                    self.views.login.show_ready();
                    self.set_screen(Screen::Login);
                    return sync;
                }
                self.set_screen(Screen::Vault);
                tracing::info!(%uid, "unlock succeeded; loading vault + send lists");
                self.switch_to_vault_task(uid)
            }
            LoginEvent::AccountSwitcher(e) => self.handle_account_switcher_event(e),
        }
    }

    /// Lock the active user's keystore and route to its unlock screen.
    pub(crate) fn handle_lock_active(&mut self) -> Task<Message> {
        let Some(uid) = self.active_user else {
            return Task::none();
        };
        self.lock_user(&uid)
    }

    /// Sign the active user out: drop their decrypted vault cache, remove
    /// them from `ClientManager`, and either switch to another account or
    /// return to the login screen when no accounts remain.
    pub(crate) fn handle_log_out(&mut self) -> Task<Message> {
        let cleanup = if let Some(uid) = self.active_user {
            let cleanup = self.log_out_user(&uid);
            self.refresh_session_timeout_deadline();
            cleanup
        } else {
            Task::none()
        };
        // Drop any sticky Magnify search keyed to the user we just signed
        // out — otherwise the launcher still holds Arc clones of their
        // decrypted ciphers in `results`.
        self.magnify_reset_sticky();
        // The user we just logged out is already gone from `user_ids()`, so
        // any remaining id is a candidate for the next active user.
        let next_uid = self.client_manager.user_ids().into_iter().next();
        let route = match next_uid {
            Some(uid) => self.handle_user_switch(uid),
            None => {
                self.active_user = None;
                self.views.login.reset_to_email_entry(&self.client_manager);
                self.set_screen(Screen::Login);
                self.views.login.auto_focus_task().map(Into::into)
            }
        };
        Task::batch([cleanup, route])
    }
}
