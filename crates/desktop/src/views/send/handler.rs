use bitwarden_generators::PasswordGeneratorRequest;
use iced::Task;

use crate::{
    app::{App, Message},
    components::toast::Toast,
    debug_fmt::NoDebug,
    fl,
    services::sdk::ClientExt,
    views::send::{SendEvent, SendMessage},
};

impl App {
    pub(crate) fn handle_send_event(&mut self, event: SendEvent) -> Task<Message> {
        match event {
            SendEvent::AccountSwitcher(e) => self.handle_account_switcher_event(e),
            SendEvent::ItemSaved { uid } => {
                self.push_toast(Toast::success(fl!("send-toast-item-saved"), None));
                self.load_send_list_task(uid)
            }
            SendEvent::ItemDeleted { uid } => {
                self.push_toast(Toast::success(fl!("send-toast-item-deleted"), None));
                self.load_send_list_task(uid)
            }
            SendEvent::ClipboardCopyRequested {
                value,
                sensitivity,
                toast_label,
            } => self.copy_and_toast(value, sensitivity, toast_label),
            SendEvent::RegeneratePasswordRequested => self.regenerate_send_password(),
        }
    }

    /// Drive the Send form's regenerate button through the SDK with a
    /// fixed sensible default (14-char, all charsets on, min one digit + one
    /// symbol). The value (not the history snapshot) is piped back into
    /// the form via `SendMessage::PasswordGenerated`; the SDK still records
    /// the entry in `ClientManager::password_history` as a side effect.
    fn regenerate_send_password(&self) -> Task<Message> {
        let req = PasswordGeneratorRequest {
            custom_allowed_chars: None,
            custom_required_chars: None,
            max_consecutive: None,
            lowercase: true,
            uppercase: true,
            numbers: true,
            special: true,
            length: 14,
            avoid_ambiguous: false,
            min_lowercase: None,
            min_uppercase: None,
            min_number: Some(1),
            min_special: Some(1),
        };
        self.perform_with_active_client(
            move |client| client.generate_password(req),
            |_uid, res| SendMessage::PasswordGenerated(res.map(NoDebug)).into(),
        )
    }
}
