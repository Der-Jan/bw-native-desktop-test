//! Passport edit section. Field order mirrors upstream
//! `bitwarden/clients#20514`: given/surname → date-of-birth → sex →
//! birth place → nationality → passport number (revealed) → passport type →
//! national identification number (revealed) → issuing country / authority
//! → issue / expiration dates.

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

pub(in super::super) fn passport_card<'a>(
    form: &'a CipherForm,
    colors: &'a AppColors,
) -> Element<'a, CipherEditMessage, AppTheme> {
    let Some(p) = form.modified.passport.as_ref() else {
        return iced::widget::Space::new().into();
    };

    let body = column![
        text_field(
            fl!("form-pp-given-name"),
            p.given_name.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::PpGivenNameChanged)
        .disabled(form.saving),
        text_field(
            fl!("form-pp-surname"),
            p.surname.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::PpSurnameChanged)
        .disabled(form.saving),
        text_field(
            format!("{} (YYYY-MM-DD)", fl!("form-pp-date-of-birth")),
            &form.dates.birth,
            colors,
        )
        .on_input(CipherEditMessage::PpDateOfBirthChanged)
        .disabled(form.saving),
        text_field(fl!("form-pp-sex"), p.sex.as_deref().unwrap_or(""), colors)
            .on_input(CipherEditMessage::PpSexChanged)
            .disabled(form.saving),
        text_field(
            fl!("form-pp-birth-place"),
            p.birth_place.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::PpBirthPlaceChanged)
        .disabled(form.saving),
        text_field(
            fl!("form-pp-nationality"),
            p.nationality.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::PpNationalityChanged)
        .disabled(form.saving),
        reveal_text_field(
            fl!("form-pp-passport-number"),
            p.passport_number.as_deref().unwrap_or(""),
            CipherEditMessage::PpPassportNumberChanged,
            form.saving,
            colors,
        ),
        text_field(
            fl!("form-pp-passport-type"),
            p.passport_type.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::PpPassportTypeChanged)
        .disabled(form.saving),
        reveal_text_field(
            fl!("form-pp-national-id-number"),
            p.national_identification_number.as_deref().unwrap_or(""),
            CipherEditMessage::PpNationalIdNumberChanged,
            form.saving,
            colors,
        ),
        text_field(
            fl!("form-pp-issuing-country"),
            p.issuing_country.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::PpIssuingCountryChanged)
        .disabled(form.saving),
        text_field(
            fl!("form-pp-issuing-authority"),
            p.issuing_authority.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(CipherEditMessage::PpIssuingAuthorityChanged)
        .disabled(form.saving),
        text_field(
            format!("{} (YYYY-MM-DD)", fl!("form-pp-issue-date")),
            &form.dates.issued,
            colors,
        )
        .on_input(CipherEditMessage::PpIssueDateChanged)
        .disabled(form.saving),
        text_field(
            format!("{} (YYYY-MM-DD)", fl!("form-pp-expiration-date")),
            &form.dates.expires,
            colors,
        )
        .on_input(CipherEditMessage::PpExpirationDateChanged)
        .disabled(form.saving),
    ]
    .spacing(12);

    card_with_margin(styled_card(body))
}
