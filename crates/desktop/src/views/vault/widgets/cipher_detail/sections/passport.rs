//! Passport read-only section.

use bitwarden_vault::PassportView;
use iced::{
    Element, Fill,
    widget::{column, text},
};

use crate::{
    components::inputs::reveal_field,
    fl,
    theme::{AppColors, AppTheme},
    views::vault::widgets::{
        cipher_detail::CipherDetailMessage,
        field_helpers::{card_with_margin, styled_card},
    },
};

use super::shared::push_optional_field;

pub(in super::super) fn passport_card<'a>(
    pp: &'a PassportView,
    colors: &'a AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let mut fields: Vec<Element<'a, CipherDetailMessage, AppTheme>> = Vec::new();
    push_optional_field(
        &mut fields,
        fl!("detail-field-pp-given-name"),
        pp.given_name.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-pp-surname"),
        pp.surname.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-pp-date-of-birth"),
        pp.date_of_birth.map(|date| date.to_string()),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-pp-sex"),
        pp.sex.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-pp-birth-place"),
        pp.birth_place.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-pp-nationality"),
        pp.nationality.as_deref(),
        colors,
    );
    if let Some(num) = pp.passport_number.as_deref() {
        fields.push(reveal_field(
            fl!("detail-field-pp-passport-number"),
            num,
            None,
            colors,
        ));
    }
    push_optional_field(
        &mut fields,
        fl!("detail-field-pp-passport-type"),
        pp.passport_type.as_deref(),
        colors,
    );
    if let Some(num) = pp.national_identification_number.as_deref() {
        fields.push(reveal_field(
            fl!("detail-field-pp-national-id-number"),
            num,
            None,
            colors,
        ));
    }
    push_optional_field(
        &mut fields,
        fl!("detail-field-pp-issuing-country"),
        pp.issuing_country.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-pp-issuing-authority"),
        pp.issuing_authority.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-pp-issue-date"),
        pp.issue_date.map(|date| date.to_string()),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-pp-expiration-date"),
        pp.expiration_date.map(|date| date.to_string()),
        colors,
    );

    if fields.is_empty() {
        fields.push(
            text(fl!("detail-empty-passport"))
                .size(crate::text_size(14))
                .color(colors.text_muted)
                .into(),
        );
    }
    card_with_margin(styled_card(column(fields).spacing(12).width(Fill)))
}
