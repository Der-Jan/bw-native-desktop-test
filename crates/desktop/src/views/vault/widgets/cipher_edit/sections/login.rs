//! Login-type section cards: credentials (username/password/TOTP + passkeys)
//! and autofill options (URIs).

use iced::{
    Alignment, Element, Fill,
    widget::{column, container, row, text},
};

use crate::{
    components::{
        buttons,
        inputs::{reveal_text_field, text_field},
    },
    fl,
    theme::{AppColors, AppTheme},
    views::vault::widgets::{
        cipher_edit::{CipherEditMessage, CipherForm},
        field_helpers::{card_with_margin, field_readonly, format_passkey_date, styled_card},
    },
};

pub(in super::super) fn login_card<'a>(
    form: &'a CipherForm,
    colors: &'a AppColors,
) -> Element<'a, CipherEditMessage, AppTheme> {
    let Some(login) = form.modified.login.as_ref() else {
        return iced::widget::Space::new().into();
    };

    let mut body = column![
        text_field(
            fl!("form-username"),
            login.username.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::UsernameChanged)
        .disabled(form.saving),
        reveal_text_field(
            fl!("form-password"),
            login.password.as_deref().unwrap_or(""),
            CipherEditMessage::PasswordChanged,
            form.saving,
            colors,
        ),
        text_field(
            fl!("form-totp"),
            login.totp.as_deref().unwrap_or(""),
            colors
        )
        .on_input(CipherEditMessage::TotpChanged)
        .disabled(form.saving),
    ]
    .spacing(16);

    // Passkey rows — read-only display of creation date plus a delete
    if let Some(creds) = login.fido2_credentials.as_deref() {
        for (idx, cred) in creds.iter().enumerate() {
            let info = field_readonly(
                fl!("detail-field-passkey"),
                fl!(
                    "detail-field-passkey-created",
                    date = format_passkey_date(cred.creation_date)
                ),
                colors,
            );
            let remove_btn =
                buttons::delete_icon_button(CipherEditMessage::PasskeyRemoved(idx), colors);

            body = body.push(
                row![container(info).width(Fill), remove_btn]
                    .spacing(6)
                    .align_y(Alignment::Center),
            );
        }
    }

    card_with_margin(styled_card(body))
}

pub(in super::super) fn autofill_card<'a>(
    form: &'a CipherForm,
    colors: &'a AppColors,
) -> Element<'a, CipherEditMessage, AppTheme> {
    let uris = form
        .modified
        .login
        .as_ref()
        .and_then(|l| l.uris.as_deref())
        .unwrap_or(&[]);

    let mut body = column![].spacing(12);

    if uris.is_empty() {
        body = body.push(
            text(fl!("form-uri-empty"))
                .size(crate::text_size(14))
                .color(colors.text_muted),
        );
    } else {
        for (idx, uri) in uris.iter().enumerate() {
            let value = uri.uri.as_deref().unwrap_or("");
            let input = text_field(fl!("form-uri"), value, colors)
                .on_input(move |s| CipherEditMessage::UriChanged(idx, s))
                .disabled(form.saving);
            let remove_btn =
                buttons::delete_icon_button(CipherEditMessage::UriRemoved(idx), colors);

            body = body.push(
                row![container(input).width(Fill), remove_btn,]
                    .spacing(6)
                    .align_y(Alignment::Center),
            );
        }
    }

    body = body.push(
        crate::views::vault::widgets::field_helpers::add_item_button(
            fl!("form-add-website"),
            CipherEditMessage::UriAdded,
            colors,
        ),
    );

    card_with_margin(styled_card(body))
}
