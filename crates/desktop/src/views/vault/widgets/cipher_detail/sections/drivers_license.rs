//! Driver's license read-only section.

use bitwarden_vault::DriversLicenseView;
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

pub(in super::super) fn drivers_license_card<'a>(
    dl: &'a DriversLicenseView,
    colors: &'a AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let mut fields: Vec<Element<'a, CipherDetailMessage, AppTheme>> = Vec::new();
    push_optional_field(
        &mut fields,
        fl!("detail-field-dl-first-name"),
        dl.first_name.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-dl-middle-name"),
        dl.middle_name.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-dl-last-name"),
        dl.last_name.as_deref(),
        colors,
    );
    if let Some(num) = dl.license_number.as_deref() {
        fields.push(reveal_field(
            fl!("detail-field-dl-license-number"),
            num,
            None,
            colors,
        ));
    }
    push_optional_field(
        &mut fields,
        fl!("detail-field-dl-date-of-birth"),
        dl.date_of_birth.map(|date| date.to_string()),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-dl-issuing-country"),
        dl.issuing_country.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-dl-issuing-state"),
        dl.issuing_state.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-dl-issuing-authority"),
        dl.issuing_authority.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-dl-issue-date"),
        dl.issue_date.map(|date| date.to_string()),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-dl-expiration-date"),
        dl.expiration_date.map(|date| date.to_string()),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-dl-license-class"),
        dl.license_class.as_deref(),
        colors,
    );

    if fields.is_empty() {
        fields.push(
            text(fl!("detail-empty-drivers-license"))
                .size(14)
                .color(colors.text_muted)
                .into(),
        );
    }
    card_with_margin(styled_card(column(fields).spacing(12).width(Fill)))
}
