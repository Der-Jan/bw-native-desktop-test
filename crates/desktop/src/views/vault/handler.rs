use iced::Task;

use crate::{
    app::{App, Message},
    components::toast::Toast,
    fl,
    services::clipboard,
    views::vault::VaultEvent,
};

impl App {
    pub(crate) fn handle_vault_event(&mut self, event: VaultEvent) -> Task<Message> {
        match event {
            VaultEvent::SortChanged(sort) => {
                self.settings.vault_sort = sort;
                self.settings.save();
                iced::widget::operation::snap_to(
                    crate::views::vault::widgets::item_list::LIST_ID,
                    iced::widget::scrollable::RelativeOffset::START,
                )
            }

            VaultEvent::MagnifyCopyAuthorized(id, field) => {
                if self.magnify.selected_item().and_then(|item| item.id) != Some(id) {
                    self.client_manager.clear_grants();
                    return Task::none();
                }
                self.magnify_copy_field(field)
            }
            VaultEvent::AccountSwitcher(e) => self.handle_account_switcher_event(e),
            VaultEvent::ItemSaved { uid } => {
                self.push_toast(Toast::success(fl!("vault-toast-item-saved"), None));
                self.load_vault_list_task(uid)
            }
            VaultEvent::ItemDeleted { uid } => {
                self.push_toast(Toast::success(fl!("vault-toast-item-deleted"), None));
                self.load_vault_list_task(uid)
            }
            VaultEvent::ClipboardCopyRequested {
                value,
                sensitivity,
                toast_label,
            } => self.copy_and_toast(value, sensitivity, toast_label),
            VaultEvent::LaunchUrlRequested { uri } => {
                clipboard::launch_url(&uri);
                Task::none()
            }
            VaultEvent::OpenNewItemPicker => self.open_new_item_picker(),
        }
    }
}
