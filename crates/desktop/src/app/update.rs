//! `App::update` — top-level message dispatch.

use iced::Task;

use super::{App, Message, UpdateCtx, View, ViewMessage};

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.update_message(message);
        let agent_task = self.refresh_ssh_agent();
        Task::batch([task, agent_task])
    }
    fn update_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SshAgent(event) => self.handle_ssh_agent_event(event),
            Message::SshFocus { id, target } => {
                if self.ssh_approval.as_ref().is_some_and(|a| {
                    a.id == id
                        && a.current()
                        && (target != crate::services::ssh_agent::Decision::UntilLock
                            || a.can_remember)
                }) {
                    iced::widget::operation::focus(crate::components::ssh_approval::focus_id(
                        target,
                    ))
                } else {
                    Task::none()
                }
            }
            Message::SshDecision { id, decision } => self.decide_ssh_approval(id, decision),
            Message::Presentation {
                generation,
                message,
            } => {
                if generation != self.presentation_generation {
                    return Task::none();
                }
                self.update(*message)
            }
            Message::SessionResult {
                session,
                presentation,
                message,
            } => {
                if self.presentation_generation != presentation
                    || self.active_user != Some(session.user_id())
                    || !self.client_manager.accepts_session(&session)
                {
                    return Task::none();
                }
                self.update(*message)
            }
            Message::About(m) => self.handle_about_message(m),
            Message::Window(m) => self.handle_window_message(m),
            Message::System(m) => self.handle_system_message(m),
            Message::Sidebar(m) => self.handle_sidebar_message(m),
            Message::Magnify(m) => self.handle_magnify_message(m),
            Message::Favicon(crate::services::favicon::FaviconMessage::IconResolved {
                uid,
                hostname,
            }) => {
                // Arrival drives the redraw; the service has already mutated
                // its in-memory cache. Log at trace so a cold unlock with
                // thousands of icons doesn't spam RUST_LOG=info users.
                tracing::trace!(%uid, %hostname, "favicon resolved");
                Task::none()
            }
            Message::View(view_msg) => {
                if self.active_user.is_some_and(|uid|self.client_manager.is_remote(&uid)) && matches!(&view_msg,
                    ViewMessage::Send(_)|ViewMessage::Import(_)|ViewMessage::Export(_)|ViewMessage::NewFolder(_)|ViewMessage::NewItemPicker(_)|
                    ViewMessage::Vault(crate::views::vault::VaultMessage::CipherEdit(_)|crate::views::vault::VaultMessage::NewItem(_)|crate::views::vault::VaultMessage::OpenNewItemPicker|crate::views::vault::VaultMessage::ConfirmDeleteSelected|crate::views::vault::VaultMessage::CipherDetail(crate::views::vault::widgets::cipher_detail::CipherDetailMessage::Edit|crate::views::vault::widgets::cipher_detail::CipherDetailMessage::Delete))) {
                    self.push_toast(crate::components::toast::Toast::warning("Dieser Tresor ist derzeit schreibgeschützt.",None));return Task::none();
                }
                let needs_unlock = matches!(
                    &view_msg,
                    ViewMessage::Vault(_)
                        | ViewMessage::Send(_)
                        | ViewMessage::Generator(_)
                        | ViewMessage::Import(_)
                        | ViewMessage::Export(_)
                        | ViewMessage::NewFolder(_)
                        | ViewMessage::NewItemPicker(_)
                );
                if needs_unlock
                    && !self
                        .active_user
                        .is_some_and(|uid| self.client_manager.is_unlocked(&uid))
                {
                    return Task::none();
                }
                // Inlined so the borrow splits cleanly: each arm reborrows
                // `&mut self.views.<view>` alongside the slices that go into
                // `uctx`. A helper that returns `UpdateCtx<'_>` would
                // require `&mut self`, conflicting with the view borrow.
                if let ViewMessage::Vault(crate::views::vault::VaultMessage::ListLoaded(
                    uid,
                    Ok((_, folders)),
                )) = &view_msg
                    && self.active_user == Some(*uid)
                {
                    use crate::views::vault::{CollectionFilter, FolderFilter};
                    if let crate::views::vault::VaultScope::Organization(id) =
                        self.sidebar.active_vault_filter.scope
                        && !self
                            .client_manager
                            .list_organizations(uid)
                            .iter()
                            .any(|org| org.id == id)
                    {
                        self.sidebar.active_vault_filter.scope =
                            crate::views::vault::VaultScope::All;
                    }
                    let filter = &mut self.sidebar.active_vault_filter;
                    if let FolderFilter::Folder(id) = filter.folder
                        && !folders.iter().any(|f| f.id == id)
                    {
                        filter.folder = FolderFilter::All;
                    }
                    if let CollectionFilter::Collection(id) = filter.collection
                        && !self
                            .client_manager
                            .list_collections(uid)
                            .iter()
                            .any(|c| c.id == id)
                    {
                        filter.collection = CollectionFilter::All;
                    }
                }
                if matches!(&view_msg, ViewMessage::Vault(crate::views::vault::VaultMessage::ListLoaded(uid, Ok(_))) if self.active_user == Some(*uid))
                {
                    self.remember_active_filter();
                }
                let active_vault_filter = self.sidebar.active_vault_filter;
                let active_send_filter = self.sidebar.active_send_filter;
                let uctx = UpdateCtx {
                    vault_sort: self.settings.vault_sort,
                    client_manager: &mut self.client_manager,
                    active_user: self.active_user.as_ref(),
                    active_vault_filter,
                    active_send_filter,
                    open_overlay: &mut self.open_overlay,
                };
                match view_msg {
                    ViewMessage::Login(m) => self
                        .views
                        .login
                        .update(m, uctx)
                        .dispatch(self, |s, e| s.handle_login_event(e)),
                    ViewMessage::Vault(m) => self
                        .views
                        .vault
                        .update(m, uctx)
                        .dispatch(self, |s, e| s.handle_vault_event(e)),
                    ViewMessage::Send(m) => self
                        .views
                        .send
                        .update(m, uctx)
                        .dispatch(self, |s, e| s.handle_send_event(e)),
                    ViewMessage::TitleBar(m) => self
                        .views
                        .title_bar
                        .update(m, uctx)
                        .dispatch(self, |s, e| s.handle_titlebar_event(e)),
                    ViewMessage::Settings(m) => self
                        .views
                        .settings
                        .update(m, uctx)
                        .dispatch(self, |s, e| s.handle_settings_event(e)),
                    ViewMessage::Generator(m) => self
                        .views
                        .generator
                        .update(m, uctx)
                        .dispatch(self, |s, e| s.handle_generator_event(e)),
                    ViewMessage::Import(m) => self
                        .views
                        .import
                        .update(m, uctx)
                        .dispatch(self, |s, e| s.handle_import_event(e)),
                    ViewMessage::Export(m) => self
                        .views
                        .export
                        .update(m, uctx)
                        .dispatch(self, |s, e| s.handle_export_event(e)),
                    ViewMessage::NewFolder(m) => {
                        let saved = matches!(
                            &m,
                            crate::views::new_folder::NewFolderMessage::Saved(Ok(()))
                        );
                        let task = self
                            .views
                            .new_folder
                            .update(m, uctx)
                            .dispatch(self, |s, e| s.handle_new_folder_event(e));
                        if saved && let Some(uid) = self.active_user {
                            Task::batch([task, self.load_vault_list_task(uid)])
                        } else {
                            task
                        }
                    }
                    ViewMessage::NewItemPicker(m) => self
                        .views
                        .new_item_picker
                        .update(m, uctx)
                        .dispatch(self, |s, e| s.handle_new_item_picker_event(e)),
                    ViewMessage::Fingerprint(m) => self
                        .views
                        .fingerprint
                        .update(m, uctx)
                        .dispatch(self, |s, e| s.handle_fingerprint_event(e)),
                    ViewMessage::ScreenshotConfirm(m) => self
                        .views
                        .screenshot_confirm
                        .update(m, uctx)
                        .dispatch(self, |s, e| s.handle_screenshot_confirm_event(e)),
                }
            }
        }
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;
    use crate::{
        app::ctx::SessionScoped,
        debug_fmt::Summary,
        domain::{Screen, UserId},
        services::sdk::test_support::unlocked_manager,
        test_support::fixtures::{CipherSpec, make_cipher},
        views::{
            generator::{GeneratorMessage, Mode},
            vault::VaultMessage,
        },
    };

    fn app() -> (App, UserId) {
        let uid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
        let mut app = App::test();
        app.client_manager = unlocked_manager(uid);
        app.active_user = Some(uid);
        app.screen = Screen::Vault;
        app.refresh_accounts_cache();
        (app, uid)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn locking_purges_lists_forms_generator_history_and_magnify_without_an_outro() {
        let (mut app, uid) = app();
        let item = make_cipher(CipherSpec {
            name: "private item",
            ..Default::default()
        });
        let _ = app.update(
            VaultMessage::ListLoaded(
                uid,
                Ok((
                    Summary(vec![item.clone()]),
                    crate::debug_fmt::NoDebug(vec![]),
                )),
            )
            .into(),
        );
        app.magnify.results.push(item);
        app.views.generator.open(Mode::Generator);
        let _ = app.update(GeneratorMessage::Generated(Ok("generated secret".into())).into());
        app.views.export.open("test@example.invalid".into());
        app.views.fingerprint.open_with("private phrase".into());
        assert!(!app.client_manager.password_history(&uid).is_empty());
        let _ = app.handle_lock_active();
        assert_eq!(app.screen, Screen::Login);
        assert!(!app.client_manager.is_unlocked(&uid));
        assert!(app.views.vault.all_items_for(&uid).is_none());
        assert!(app.magnify.results.is_empty());
        assert!(app.client_manager.password_history(&uid).is_empty());
        assert!(!app.views.generator.should_render());
        assert!(!app.views.export.should_render());
        assert!(!app.views.fingerprint.should_render());
        let mut context = App::test();
        assert!(matches!(
            app.views
                .generator
                .update(GeneratorMessage::CopyCurrent, context.update_ctx()),
            super::super::Outcome::None
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn queued_result_cannot_repopulate_after_lock_or_same_id_account_replacement() {
        let (mut app, uid) = app();
        let session = app.client_manager.session_for(&uid).unwrap();
        let queued = Message::from(GeneratorMessage::Generated(Ok("late secret".into())))
            .for_session(session.clone(), app.presentation_generation);
        let _ = app.handle_lock_active();
        let _ = app.update(queued);
        assert!(app.client_manager.password_history(&uid).is_empty());
        app.client_manager = unlocked_manager(uid);
        // Match the presentation counter deliberately: account incarnation must also be checked.
        let queued = Message::from(GeneratorMessage::Generated(Ok("old account secret".into())))
            .for_session(session, app.presentation_generation);
        let _ = app.update(queued);
        assert!(app.client_manager.password_history(&uid).is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn switching_away_and_back_does_not_accept_a_queued_old_ui_action() {
        let (mut app, uid) = app();
        let session = app.client_manager.session_for(&uid).unwrap();
        let queued = Message::from(GeneratorMessage::Generated(Ok("late secret".into())))
            .for_session(session, app.presentation_generation);
        let other = "00000000-0000-0000-0000-000000000002".parse().unwrap();
        let _ = app.handle_user_switch(other);
        let _ = app.handle_user_switch(uid);
        assert!(app.client_manager.is_unlocked(&uid));
        let _ = app.update(queued);
        assert!(app.client_manager.password_history(&uid).is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn logout_removes_runtime_membership_and_rejects_queued_actions() {
        let (mut app, uid) = app();
        let session = app.client_manager.session_for(&uid).unwrap();
        let queued = Message::from(GeneratorMessage::Generated(Ok("late secret".into())))
            .for_session(session, app.presentation_generation);
        let _ = app.handle_log_out();
        let _ = app.update(queued);
        assert!(app.active_user.is_none());
        assert!(!app.client_manager.has_users());
        assert!(app.client_manager.password_history(&uid).is_empty());
    }
    #[tokio::test(flavor = "current_thread")]
    async fn logout_completions_are_idempotent_and_failed_commit_prevents_silent_exit() {
        use crate::{
            app::SystemMessage,
            services::sdk::{LogoutFailure, StorageWarning},
        };
        let (mut app, _) = app();
        let first = uuid::Uuid::new_v4();
        let second = uuid::Uuid::new_v4();
        app.pending_logouts.extend([first, second]);
        let _ = app.request_exit();
        assert!(app.exit_after_logout);
        for _ in 0..2 {
            let _ = app.update(Message::System(SystemMessage::LogoutCompleted {
                operation: first,
                result: Ok(()),
            }));
            assert_eq!(
                app.pending_logouts.len(),
                1,
                "duplicate completions cannot finish another logout"
            );
        }
        let _ = app.update(Message::System(SystemMessage::LogoutCompleted {
            operation: second,
            result: Err(LogoutFailure::NotCommitted),
        }));
        assert!(app.pending_logouts.is_empty());
        assert!(!app.exit_after_logout);
        assert!(matches!(
            app.storage_warning,
            Some(StorageWarning::LogoutNotStored)
        ));
        app.clear_sensitive_ui();
        assert!(
            matches!(app.storage_warning, Some(StorageWarning::LogoutNotStored)),
            "security transitions must preserve persistent storage warnings"
        );
    }
}
