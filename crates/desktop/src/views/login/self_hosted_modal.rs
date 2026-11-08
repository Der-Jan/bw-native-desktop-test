//! Self-hosted environment configuration modal.
//!
//! Opened from the server selector dropdown when the user picks "Self-hosted".
//! Collects a base URL and writes it back into the active `LoginEmail`'s
//! `selected_server` on Save.

use iced::{
    Element, Fill,
    widget::{self, column, container, text},
};

use crate::{
    components::{inputs, modal},
    fl,
    theme::{AppColors, AppTheme},
};

use super::{LoginMessage, SelfHostedModal};

pub const SELF_HOSTED_URL_FIELD_ID: widget::Id = widget::Id::new("self-hosted-url-field");

pub fn view<'a>(
    state: &'a SelfHostedModal,
    certificate_controls: Element<'a, LoginMessage, AppTheme>,
    save_enabled: bool,
    colors: &'a AppColors,
) -> Option<Element<'a, LoginMessage, AppTheme>> {
    let progress = state.fade.progress_if_visible()?;

    let header = modal::dialog_header(
        fl!("login-self-hosted-modal-title"),
        LoginMessage::SelfHostedCancel,
        colors,
    );

    let url_input = inputs::bare_text_input(&state.url_input)
        .id(SELF_HOSTED_URL_FIELD_ID)
        .on_input(LoginMessage::SelfHostedUrlChanged)
        .on_submit(LoginMessage::SelfHostedSave);
    let url_field = if state.url_error {
        inputs::errored_field_frame(fl!("login-self-hosted-modal-url-label"), url_input, colors)
    } else {
        inputs::field_frame(fl!("login-self-hosted-modal-url-label"), url_input, colors)
    };

    // Inline validation row replaces the helper text once the user hits Save
    // with an invalid URL — they read as alternatives, not stacked siblings.
    let info_row: Element<'a, LoginMessage, AppTheme> = if state.url_error {
        inputs::field_error_row(fl!("login-self-hosted-modal-url-error"), colors)
    } else {
        text(fl!("login-self-hosted-modal-url-helper"))
            .size(crate::text_size(12))
            .color(colors.text_secondary)
            .into()
    };

    let footer = modal::footer_actions(
        fl!("login-self-hosted-modal-save"),
        save_enabled.then_some(LoginMessage::SelfHostedSave),
        fl!("login-self-hosted-modal-cancel"),
        LoginMessage::SelfHostedCancel,
    );

    let body = column![header, url_field, info_row, certificate_controls, footer]
        .spacing(16)
        .padding(modal::BODY_PADDING)
        .width(Fill);

    Some(modal::dialog(
        620.0,
        None,
        |c| c.background,
        progress,
        container(iced::widget::scrollable(body).height(iced::Length::Shrink)),
        LoginMessage::SelfHostedCancel,
    ))
}
