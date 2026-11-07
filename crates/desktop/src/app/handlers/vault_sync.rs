use crate::{
    app::{App, Message, SystemMessage},
    components::toast::Toast,
    domain::{Screen, UserId},
    services::sdk::vault_sync,
};
use iced::Task;
#[derive(Default)]
pub(crate) struct SyncUi {
    pub operation: Option<uuid::Uuid>,
    pub error: Option<String>,
}
impl App {
    pub(crate) fn start_vault_sync(&mut self) -> Task<Message> {
        let Some(uid) = self.active_user else {
            return Task::none();
        };
        if !self.client_manager.is_remote(&uid) {
            self.push_toast(Toast::warning("Demo-Daten haben keinen Sync-Server.", None));
            return Task::none();
        }
        let Some(client) = self.client_manager.client_for(&uid) else {
            return Task::none();
        };
        let Some(store) = self.client_manager.profiles() else {
            return Task::none();
        };
        let status = self.sync_status.entry(uid).or_default();
        if status.operation.is_some() {
            return Task::none();
        }
        let operation = uuid::Uuid::new_v4();
        status.operation = Some(operation);
        status.error = None;
        // Deliberately do not bind this to the old UI generation: a successful
        // commit advances that generation. The result carries its new authority.
        Task::perform(
            vault_sync::synchronize(client, crate::paths::data_dir(), store),
            move |result| {
                Message::System(SystemMessage::SyncCompleted {
                    uid,
                    operation,
                    result,
                })
            },
        )
    }
    pub(crate) fn finish_vault_sync(
        &mut self,
        uid: UserId,
        operation: uuid::Uuid,
        result: Result<vault_sync::Report, String>,
    ) -> Task<Message> {
        let Some(status) = self.sync_status.get_mut(&uid) else {
            return Task::none();
        };
        if status.operation != Some(operation) {
            return Task::none();
        }
        status.operation = None;
        match result {
            Ok(report) => {
                if report.session.user_id() != uid
                    || !self.client_manager.accepts_session(&report.session)
                    || !self.client_manager.is_unlocked(&uid)
                {
                    return Task::none();
                }
                status.error = None;
                let selected = self.views.vault.selected_id();
                self.views.vault.remove_user_items(&uid);
                if self.active_user != Some(uid) {
                    return Task::none();
                }
                self.clear_sensitive_ui();
                if let crate::views::vault::VaultScope::Organization(id) =
                    self.sidebar.active_vault_filter.scope
                    && !self
                        .client_manager
                        .list_organizations(&uid)
                        .iter()
                        .any(|o| o.id == id)
                {
                    self.sidebar.active_vault_filter.scope = crate::views::vault::VaultScope::All;
                }
                self.views.vault.restore_selection_after_sync(selected);
                self.set_screen(Screen::Vault);
                self.push_toast(Toast::success(
                    format!(
                        "{} Einträge synchronisiert · {}",
                        report.ciphers,
                        report
                            .downloaded_at
                            .with_timezone(&chrono::Local)
                            .format("%H:%M")
                    ),
                    None,
                ));
                self.load_vault_list_task(uid)
            }
            Err(error) => {
                status.error = Some(error.clone());
                if self.active_user == Some(uid)
                    && !self.client_manager.is_unlocked(&uid)
                    && matches!(self.screen, Screen::Vault | Screen::Send)
                {
                    return self.lock_user(&uid);
                }
                if self.active_user == Some(uid) && self.client_manager.is_unlocked(&uid) {
                    self.push_toast(Toast::error(error, Some("Synchronisierung fehlgeschlagen")));
                }
                Task::none()
            }
        }
    }
}
