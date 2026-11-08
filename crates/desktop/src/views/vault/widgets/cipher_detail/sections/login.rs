//! Login-type read-only cards: credentials (username / password / TOTP +
//! passkeys) and the autofill URIs list.

use bitwarden_vault::LoginView;
use iced::{
    Element, Fill,
    widget::{column, text},
};

use crate::{
    components::{self, icons, inputs::reveal_field},
    fl,
    theme::{AppColors, AppTheme},
    views::vault::widgets::{
        cipher_detail::CipherDetailMessage,
        field_helpers::{card_with_margin, field_readonly, format_passkey_date, styled_card},
    },
};

use super::shared::field_with_action;

pub(in super::super) fn login_card<'a>(
    login: &'a LoginView,
    colors: &'a AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let mut fields: Vec<Element<'a, CipherDetailMessage, AppTheme>> = Vec::new();

    if let Some(username) = login.username.as_deref() {
        fields.push(field_with_action(
            fl!("detail-field-username"),
            username,
            &[icons::BWI_COPY],
            &[CipherDetailMessage::CopyUsername],
            colors,
        ));
    }

    if let Some(password) = login.password.as_deref() {
        fields.push(reveal_field(
            fl!("detail-field-password"),
            password,
            Some(CipherDetailMessage::CopyPassword),
            colors,
        ));
    }

    if let Some(totp) = login.totp.as_deref().filter(|s| !s.is_empty()) {
        fields.push(components::totp::view(
            totp,
            CipherDetailMessage::CopyTotp,
            colors,
        ));
    }

    // `fido2_credentials` holds the still-encrypted `Fido2Credential` (the
    // SDK keeps this field opaque on the view — see the upstream TODO on
    // `LoginView`). The only plaintext field is `creation_date`, so that's
    // all we surface here.
    if let Some(creds) = login.fido2_credentials.as_deref() {
        for cred in creds {
            fields.push(field_readonly(
                fl!("detail-field-passkey"),
                fl!(
                    "detail-field-passkey-created",
                    date = format_passkey_date(cred.creation_date)
                ),
                colors,
            ));
        }
    }

    if fields.is_empty() {
        fields.push(
            text(fl!("detail-empty-credentials"))
                .size(crate::text_size(14))
                .color(colors.text_muted)
                .into(),
        );
    }

    card_with_margin(styled_card(column(fields).spacing(16).width(Fill)))
}

pub(in super::super) fn autofill_card<'a>(
    uris: &[(usize, &'a str)],
    colors: &'a AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let rows: Vec<Element<'a, CipherDetailMessage, AppTheme>> = uris
        .iter()
        .copied()
        .map(|(idx, uri)| autofill_row(idx, uri, colors))
        .collect();
    card_with_margin(styled_card(column(rows).spacing(12).width(Fill)))
}

fn autofill_row<'a>(
    idx: usize,
    uri: &'a str,
    colors: &'a AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    field_with_action(
        fl!("detail-field-website"),
        uri,
        &[icons::BWI_COPY, icons::BWI_EXTERNAL_LINK],
        &[
            CipherDetailMessage::CopyUrl(idx),
            CipherDetailMessage::OpenUrl(idx),
        ],
        colors,
    )
}

/// Non-empty URIs on a login, paired with their original index in
/// `login.uris`. The index is the stable key used by the `CopyUrl` /
/// `OpenUrl` messages, so we must preserve position — callers should
/// not sort or re-index this.
pub(in super::super) fn collect_login_uris(login: &LoginView) -> Vec<(usize, &str)> {
    login
        .uris
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .enumerate()
        .filter_map(|(i, u)| u.uri.as_deref().map(|s| (i, s)))
        .collect()
}

/// Convenience used by handlers that only care about a single URI by its
/// position in `login.uris`. Re-exported from `cipher_detail::mod.rs`.
pub(crate) fn login_uri_at(login: &LoginView, index: usize) -> Option<&str> {
    login
        .uris
        .as_deref()?
        .get(index)
        .and_then(|u| u.uri.as_deref())
}
