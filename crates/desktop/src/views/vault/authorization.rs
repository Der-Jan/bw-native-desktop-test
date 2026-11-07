use super::{
    VaultMessage, VaultView,
    widgets::{
        cipher_detail::CipherDetailMessage, cipher_edit::CipherEditMessage,
        item_list::ItemListMessage,
    },
};
use crate::components::buttons::secondary as button;
use crate::{
    app::{Outcome, RenderCtx, UpdateCtx, View},
    components::{modal, toast::Toast},
    services::sdk::{Action, Authorization, SessionToken},
    theme::AppTheme,
};
use iced::{
    Element,
    widget::{column, container, row, text, text_input},
};
use uuid::Uuid;
#[derive(Clone)]
pub enum PromptMessage {
    Password(String),
    Submit,
    Cancel,
    Completed(Uuid, Result<(), String>),
}
impl std::fmt::Debug for PromptMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Authorization(<redacted>)")
    }
}
pub(super) struct Prompt {
    id: Uuid,
    target: Authorization,
    message: Box<VaultMessage>,
    session: SessionToken,
    password: String,
    busy: bool,
    error: bool,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl Drop for Prompt {
    fn drop(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
impl VaultView {
    fn target(
        &self,
        msg: &VaultMessage,
        ctx: &UpdateCtx<'_>,
    ) -> Option<(Authorization, bool, bool)> {
        let action = match msg {
            VaultMessage::ItemList(ItemListMessage::ItemSelected(_)) => Action::Read,
            VaultMessage::CipherDetail(CipherDetailMessage::Edit) => Action::Edit,
            VaultMessage::CipherDetail(
                CipherDetailMessage::CopyUsername
                | CipherDetailMessage::CopyPassword
                | CipherDetailMessage::CopyTotp
                | CipherDetailMessage::CopySshPrivateKey
                | CipherDetailMessage::CopySshPublicKey
                | CipherDetailMessage::CopySshFingerprint
                | CipherDetailMessage::CopyCustomField(_)
                | CipherDetailMessage::CopyUrl(_),
            ) => Action::Copy,
            VaultMessage::CipherEdit(CipherEditMessage::Save) => Action::Write,
            VaultMessage::ConfirmDeleteSelected => Action::Delete,
            VaultMessage::MagnifyCopy(_, _)
            | VaultMessage::ItemList(
                ItemListMessage::CopyUsername(_) | ItemListMessage::CopyPassword(_),
            ) => Action::Copy,
            _ => return None,
        };
        let item = match msg {
            VaultMessage::MagnifyCopy(id, _)
            | VaultMessage::ItemList(
                ItemListMessage::CopyUsername(id) | ItemListMessage::CopyPassword(id),
            ) => self
                .items
                .get(ctx.active_user?)?
                .all
                .iter()
                .find(|i| i.id == Some(*id))
                .map(|i| i.as_ref()),
            VaultMessage::ItemList(ItemListMessage::ItemSelected(index)) => self
                .items
                .get(ctx.active_user?)?
                .cached
                .get(*index)
                .map(|i| i.as_ref()),
            _ => self
                .items
                .get(ctx.active_user?)?
                .all
                .iter()
                .find(|i| i.id == self.selection.id)
                .map(|i| i.as_ref()),
        }?;
        let denied = !item.view_password
            || (matches!(action, Action::Edit | Action::Write | Action::Delete) && !item.edit)
            || (action == Action::Delete && item.permissions.as_ref().is_some_and(|p| !p.delete));
        Some((
            Authorization {
                cipher: item.id,
                revision: Some(item.revision_date),
                action,
            },
            item.reprompt == bitwarden_vault::CipherRepromptType::Password,
            denied,
        ))
    }
    pub(super) fn authorize_action(
        &mut self,
        msg: &VaultMessage,
        ctx: &UpdateCtx<'_>,
    ) -> Option<Outcome<Self>> {
        if matches!(msg, VaultMessage::Authorization(_)) {
            return None;
        }
        // Any new action cancels the old prompt; its eventual result cannot replay it.
        if self.reauthorize.is_some() {
            ctx.client_manager.clear_grants();
        }
        self.reauthorize = None;
        let (target, reprompt, denied) = self.target(msg, ctx)?;
        if denied {
            return Some(Outcome::toast(Toast::error(
                "Für diesen Eintrag fehlt die Berechtigung.",
                None,
            )));
        }
        if !reprompt {
            return None;
        }
        let uid = ctx.active_user?;
        let consume = matches!(target.action, Action::Copy | Action::Edit)
            && !matches!(
                msg,
                VaultMessage::MagnifyCopy(_, _)
                    | VaultMessage::ItemList(
                        ItemListMessage::CopyUsername(_) | ItemListMessage::CopyPassword(_)
                    )
            );
        if ctx.client_manager.permitted(uid, &target, consume) {
            return None;
        }
        self.reauthorize = Some(Prompt {
            id: Uuid::new_v4(),
            target,
            message: Box::new(msg.clone()),
            session: ctx.client_manager.session_for(uid)?,
            password: String::new(),
            busy: false,
            error: false,
            cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        });
        Some(Outcome::None)
    }
    pub(super) fn update_authorization(
        &mut self,
        message: PromptMessage,
        ctx: UpdateCtx<'_>,
    ) -> Outcome<Self> {
        let Some(prompt) = self.reauthorize.as_mut() else {
            return Outcome::None;
        };
        if ctx.active_user != Some(&prompt.session.user_id())
            || !ctx.client_manager.accepts_session(&prompt.session)
        {
            self.reauthorize = None;
            return Outcome::None;
        }
        match message {
            PromptMessage::Password(s) => prompt.password = s,
            PromptMessage::Cancel => {
                self.reauthorize = None;
                ctx.client_manager.clear_grants();
            }
            PromptMessage::Submit => {
                if prompt.busy {
                    return Outcome::None;
                }
                let Some((client, envelope)) = ctx
                    .client_manager
                    .validation_data_for(&prompt.session.user_id())
                else {
                    return Outcome::None;
                };
                let password = std::mem::take(&mut prompt.password);
                let target = prompt.target.clone();
                let id = prompt.id;
                prompt.busy = true;
                return Outcome::perform(
                    client.authorize_password_scoped(
                        envelope,
                        password,
                        target,
                        prompt.cancelled.clone(),
                    ),
                    move |r| {
                        VaultMessage::Authorization(PromptMessage::Completed(
                            id,
                            r.map_err(|_| "Authorization failed".into()),
                        ))
                    },
                );
            }
            PromptMessage::Completed(id, result) => {
                if id != prompt.id {
                    return Outcome::None;
                }
                if result.is_err() {
                    prompt.busy = false;
                    prompt.error = true;
                    return Outcome::None;
                }
                let prompt = self.reauthorize.take().unwrap();
                // Re-evaluate current item/revision/action before using the one-shot grant.
                return self.update((*prompt.message).clone(), ctx);
            }
        }
        Outcome::None
    }
    pub(super) fn authorization_overlay<'a>(
        &'a self,
        _ctx: &RenderCtx<'a>,
    ) -> Option<Element<'a, VaultMessage, AppTheme>> {
        let p = self.reauthorize.as_ref()?;
        let m = |m| VaultMessage::Authorization(m);
        let mut body = column![
            text("Master-Passwort bestätigen").size(22),
            text("Diese Freigabe gilt einmalig für die angeforderte Aktion."),
            text_input("Master-Passwort", &p.password)
                .secure(true)
                .on_input(move |s| m(PromptMessage::Password(s)))
                .on_submit(m(PromptMessage::Submit))
        ]
        .spacing(16)
        .padding(24);
        if p.error {
            body = body.push(text("Passwort ungültig oder Aktion nicht mehr verfügbar."));
        }
        body = body.push(
            row![
                button("Abbrechen").on_press(m(PromptMessage::Cancel)),
                button(if p.busy { "Prüfe …" } else { "Bestätigen" })
                    .on_press_maybe((!p.busy).then(|| m(PromptMessage::Submit)))
            ]
            .spacing(12),
        );
        Some(modal::dialog(
            480.0,
            None,
            |c| c.background,
            1.0,
            container(body),
            m(PromptMessage::Cancel),
        ))
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;
    use crate::{
        app::App,
        test_support::fixtures::{CipherSpec, make_cipher},
    };
    use std::sync::Arc;

    #[tokio::test]
    async fn list_copy_uses_clicked_cipher_and_requires_its_own_copy_grant() {
        crate::test_support::init();
        let mut app = App::test();
        let uid = crate::domain::UserId::new_v4();
        app.client_manager = crate::services::sdk::test_support::unlocked_manager(uid);
        app.active_user = Some(uid);
        let mut item = make_cipher(CipherSpec::default());
        let id = bitwarden_vault::CipherId::new_v4();
        Arc::get_mut(&mut item).unwrap().id = Some(id);
        Arc::get_mut(&mut item).unwrap().reprompt = bitwarden_vault::CipherRepromptType::Password;
        let mut view = VaultView::new();
        view.selection.id = Some(bitwarden_vault::CipherId::new_v4());
        view.items.entry(uid).or_default().all = vec![item];
        for msg in [
            ItemListMessage::CopyUsername(id),
            ItemListMessage::CopyPassword(id),
        ] {
            let message = VaultMessage::ItemList(msg);
            let ctx = app.update_ctx();
            let (target, reprompt, denied) = view.target(&message, &ctx).unwrap();
            assert_eq!(target.cipher, Some(id));
            assert_eq!(target.action, Action::Copy);
            assert!(reprompt && !denied);
            assert!(matches!(view.update(message, ctx), Outcome::None));
            assert_eq!(view.reauthorize.as_ref().unwrap().target.cipher, Some(id));
        }
        Arc::get_mut(&mut view.items.get_mut(&uid).unwrap().all[0])
            .unwrap()
            .view_password = false;
        assert!(matches!(
            view.update(
                VaultMessage::ItemList(ItemListMessage::CopyPassword(id)),
                app.update_ctx()
            ),
            Outcome::Toast(_)
        ));
        assert!(view.reauthorize.is_none());
    }
}
