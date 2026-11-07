//! Driver's license edit section. Field order mirrors upstream
//! `bitwarden/clients#20461`: name triple → license number (revealed) →
//! date-of-birth → issuing-country/state/authority → issue/expiration dates →
//! license class. Dates are plain text — the SDK stores them as
//! `Option<String>` and we don't ship a date-group widget.

use iced::{Element, widget::column};

use crate::{
    components::inputs::{reveal_text_field, text_field},
    fl,
    theme::{AppColors, AppTheme},
    views::vault::widgets::{
        cipher_edit::{CipherEditMessage, CipherForm},
        field_helpers::{card_with_margin, styled_card},
    },
};

pub(in super::super) fn drivers_license_card<'a>(
    form: &'a CipherForm,
    colors: &'a AppColors,
) -> Element<'a, CipherEditMessage, AppTheme> {
    let Some(d) = form.modified.drivers_license.as_ref() else {
        return iced::widget::Space::new().into();
    };

    let body = column![
        text_field(
            fl!("form-dl-first-name"),
            d.first_name.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::DlFirstNameChanged)
        .disabled(form.saving),
        text_field(
            fl!("form-dl-middle-name"),
            d.middle_name.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::DlMiddleNameChanged)
        .disabled(form.saving),
        text_field(
            fl!("form-dl-last-name"),
            d.last_name.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::DlLastNameChanged)
        .disabled(form.saving),
        reveal_text_field(
            fl!("form-dl-license-number"),
            d.license_number.as_deref().unwrap_or(""),
            CipherEditMessage::DlLicenseNumberChanged,
            form.saving,
            colors,
        ),
        text_field(
            format!("{} (YYYY-MM-DD)", fl!("form-dl-date-of-birth")),
            &form.dates.birth,
            colors,
        )
        .on_input(CipherEditMessage::DlDateOfBirthChanged)
        .disabled(form.saving),
        text_field(
            fl!("form-dl-issuing-country"),
            d.issuing_country.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::DlIssuingCountryChanged)
        .disabled(form.saving),
        text_field(
            fl!("form-dl-issuing-state"),
            d.issuing_state.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::DlIssuingStateChanged)
        .disabled(form.saving),
        text_field(
            fl!("form-dl-issuing-authority"),
            d.issuing_authority.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::DlIssuingAuthorityChanged)
        .disabled(form.saving),
        text_field(
            format!("{} (YYYY-MM-DD)", fl!("form-dl-issue-date")),
            &form.dates.issued,
            colors,
        )
        .on_input(CipherEditMessage::DlIssueDateChanged)
        .disabled(form.saving),
        text_field(
            format!("{} (YYYY-MM-DD)", fl!("form-dl-expiration-date")),
            &form.dates.expires,
            colors,
        )
        .on_input(CipherEditMessage::DlExpirationDateChanged)
        .disabled(form.saving),
        text_field(
            fl!("form-dl-license-class"),
            d.license_class.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::DlLicenseClassChanged)
        .disabled(form.saving),
    ]
    .spacing(12);

    card_with_margin(styled_card(body))
}
