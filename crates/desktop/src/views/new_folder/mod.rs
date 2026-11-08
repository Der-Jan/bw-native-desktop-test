//! New folder modal — wired to File → New folder.
//!
//! Single-field dialog: a `name` text input plus Save / Cancel. On Save
//! `ClientManager::create_folder` encrypts the view and writes it into the
//! per-user folder repo (no API call — same shape as `save_cipher`).
//!
//! Nested folders are a server-side convention: a name like `"Social/Forums"`
//! is stored as a single folder; the helper text under the field documents
//! it so users don't try to create the parent first.

mod handler;

use iced::{
    Element, Fill,
    widget::{self, column, container, text},
};

use crate::{
    app::{Outcome, RenderCtx, UpdateCtx, View},
    components::{FadeInOut, inputs, modal, toast::Toast},
    fl,
    theme::AppTheme,
};

pub const NAME_FIELD_ID: widget::Id = widget::Id::new("new-folder-name-field");

// ── State ─────────────────────────────────────────────────────────────────

pub struct NewFolderView {
    pub(super) fade: FadeInOut,
    name: String,
    /// True while the SDK encrypt + repo write is in flight. Disables the
    /// Save button + name input so submit-spamming can't double-create.
    saving: bool,
}

#[derive(Debug, Clone)]
pub enum NewFolderMessage {
    Close,
    NameChanged(String),
    Submit,
    /// SDK round-trip finished — `Ok(())` on success (the modal doesn't need
    /// the saved view back), `Err` is a human-readable message.
    Saved(Result<(), String>),
}

pub enum NewFolderEvent {
    /// Encrypt + persist the entered name. App spawns the SDK call.
    Run(String),
}

// ── Lifecycle ─────────────────────────────────────────────────────────────

impl NewFolderView {
    pub fn new() -> Self {
        Self {
            fade: FadeInOut::default(),
            name: String::new(),
            saving: false,
        }
    }

    pub fn open(&mut self) {
        self.fade.open();
        self.name.clear();
        self.saving = false;
    }
}

impl View for NewFolderView {
    type Message = NewFolderMessage;
    type Event = NewFolderEvent;

    fn update(&mut self, msg: NewFolderMessage, _ctx: UpdateCtx<'_>) -> Outcome<Self> {
        match msg {
            NewFolderMessage::Close => {
                self.fade.close();
                Outcome::None
            }
            NewFolderMessage::NameChanged(n) => {
                if !self.saving {
                    self.name = n;
                }
                Outcome::None
            }
            NewFolderMessage::Submit => {
                let trimmed = self.name.trim();
                if trimmed.is_empty() || self.saving {
                    return Outcome::None;
                }
                self.saving = true;
                Outcome::event(NewFolderEvent::Run(trimmed.to_string()))
            }
            NewFolderMessage::Saved(Ok(_)) => {
                self.saving = false;
                self.fade.close();
                Outcome::toast(Toast::success(fl!("new-folder-toast-success"), None))
            }
            NewFolderMessage::Saved(Err(err)) => {
                self.saving = false;
                tracing::warn!(%err, "create folder failed");
                Outcome::toast(Toast::error(
                    fl!("new-folder-toast-failed-body"),
                    Some(&fl!("new-folder-toast-failed-title")),
                ))
            }
        }
    }

    fn should_render(&self) -> bool {
        self.fade.is_visible()
    }

    fn view<'a>(&'a self, ctx: &RenderCtx<'a>) -> Element<'a, NewFolderMessage, AppTheme> {
        let progress = self.fade.progress_when_visible();

        let header = modal::dialog_header(
            fl!("new-folder-modal-title"),
            NewFolderMessage::Close,
            ctx.colors,
        );

        // Modal sits on `card_bg`, so the floating-label chip needs the
        // matching surface colour to blend cleanly.
        let mut name_field = inputs::text_field(
            fl!("new-folder-modal-field-label"),
            self.name.as_str(),
            ctx.colors,
        )
        .id(NAME_FIELD_ID.clone())
        .chip_bg(|c| c.card_bg)
        .disabled(self.saving);
        if !self.saving {
            name_field = name_field
                .on_input(NewFolderMessage::NameChanged)
                .on_submit(NewFolderMessage::Submit);
        }

        let helper = text(fl!("new-folder-modal-helper"))
            .size(crate::text_size(12))
            .color(ctx.colors.text_secondary);

        let submit_disabled = self.saving || self.name.trim().is_empty();
        let on_submit = (!submit_disabled).then_some(NewFolderMessage::Submit);
        let footer = modal::footer_actions(
            fl!("new-folder-modal-save"),
            on_submit,
            fl!("new-folder-modal-cancel"),
            NewFolderMessage::Close,
        );

        let body = column![header, name_field, helper, footer]
            .spacing(14)
            .padding(modal::BODY_PADDING)
            .width(Fill);

        modal::dialog(
            480.0,
            None,
            |c| c.card_bg,
            progress,
            container(body),
            NewFolderMessage::Close,
        )
    }
}

#[cfg(test)]
mod tests_snapshot {
    use super::*;
    use crate::test_support::{self, ViewTestExt};

    #[tokio::test(flavor = "current_thread")]
    async fn new_folder_modal() {
        let mut view = NewFolderView::new();
        view.open();
        test_support::settle_animations();

        view.assert_themed_snapshots("new_folder_modal").await;
    }
}

#[cfg(test)]
mod tests_interaction {
    use super::*;
    use crate::{app::App, test_support};

    #[tokio::test(flavor = "current_thread")]
    async fn type_name_then_click_save_submits() {
        test_support::init();
        let mut view = NewFolderView::new();
        view.open();
        test_support::settle_animations();

        let mut app = App::test();

        // Focus the name field via its stable widget::Id, then type.
        let element = view.view(&app.render_ctx_main());
        let messages = test_support::drive_element(element, |ui| {
            ui.click(NAME_FIELD_ID.clone())
                .expect("name field has a click target");
            ui.typewrite("Social");
        });

        // text_input emits one NameChanged per keystroke with the cumulative
        // text. Apply them all so the next render reflects the full typed
        // value.
        for msg in messages {
            let _ = view.update(msg, app.update_ctx());
        }
        assert_eq!(view.name, "Social");
        assert!(!view.saving);

        // With a non-empty name the Save button is now enabled. Click it.
        let element = view.view(&app.render_ctx_main());
        let messages = test_support::drive_element(element, |ui| {
            ui.click("Save").expect("Save button enabled");
        });
        test_support::assert_emitted(&messages, "Submit", |m| {
            matches!(m, NewFolderMessage::Submit)
        });

        for msg in messages {
            let _ = view.update(msg, app.update_ctx());
        }
        assert!(view.saving, "Submit flipped saving=true");
    }
}

#[cfg(test)]
mod tests_update {
    use super::*;
    use crate::{
        components::toast::ToastStatus,
        test_support::{OutcomeExt, ViewTestExt},
    };

    #[tokio::test(flavor = "current_thread")]
    async fn name_changed_updates_when_not_saving() {
        let mut view = NewFolderView::new();
        view.run(NewFolderMessage::NameChanged("Social".into()))
            .await
            .expect_none();
        assert_eq!(view.name, "Social");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn name_changed_is_ignored_while_saving() {
        let mut view = NewFolderView::new();
        view.open();
        view.run([
            NewFolderMessage::NameChanged("First".into()),
            NewFolderMessage::Submit,
        ])
        .await;
        assert!(view.saving);

        view.run(NewFolderMessage::NameChanged("Second".into()))
            .await
            .expect_none();
        assert_eq!(
            view.name, "First",
            "name field is locked while the SDK call is in flight"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_with_empty_name_returns_none() {
        let mut view = NewFolderView::new();
        view.run(NewFolderMessage::Submit).await.expect_none();
        assert!(!view.saving);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_with_whitespace_only_name_returns_none() {
        let mut view = NewFolderView::new();
        view.run([NewFolderMessage::NameChanged("   \t".into())])
            .await;
        view.run(NewFolderMessage::Submit).await.expect_none();
        assert!(!view.saving);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_with_valid_name_emits_run_event_and_flips_saving() {
        let mut view = NewFolderView::new();
        view.run([NewFolderMessage::NameChanged("  Social  ".into())])
            .await;
        let NewFolderEvent::Run(name) = view.run(NewFolderMessage::Submit).await.expect_event();
        assert_eq!(name, "Social");
        assert!(view.saving);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_while_already_saving_returns_none() {
        let mut view = NewFolderView::new();
        view.run([
            NewFolderMessage::NameChanged("Social".into()),
            NewFolderMessage::Submit,
        ])
        .await;
        assert!(view.saving);

        view.run(NewFolderMessage::Submit).await.expect_none();
        assert!(view.saving, "still saving");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn saved_ok_clears_saving_and_emits_success_toast() {
        let mut view = NewFolderView::new();
        view.run([
            NewFolderMessage::NameChanged("Social".into()),
            NewFolderMessage::Submit,
        ])
        .await;

        let toast = view
            .run(NewFolderMessage::Saved(Ok(())))
            .await
            .expect_toast();
        assert!(matches!(toast.status, ToastStatus::Success));
        assert!(!view.saving);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn saved_err_clears_saving_and_emits_error_toast() {
        let mut view = NewFolderView::new();
        view.run([
            NewFolderMessage::NameChanged("Social".into()),
            NewFolderMessage::Submit,
        ])
        .await;

        let toast = view
            .run(NewFolderMessage::Saved(Err("boom".into())))
            .await
            .expect_toast();
        assert!(matches!(toast.status, ToastStatus::Error));
        assert!(!view.saving);
    }
}
