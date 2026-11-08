//! Identity-type read-only section. One combined card — the editable side
//! splits identity across four cards (personal / identification / contact /
//! address); here everything flattens into a single read-only list since
//! the fields are non-interactive.

use bitwarden_vault::IdentityView;
use iced::{
    Element, Fill,
    widget::{column, text},
};

use crate::{
    fl,
    theme::{AppColors, AppTheme},
    views::vault::widgets::{
        cipher_detail::CipherDetailMessage,
        field_helpers::{card_with_margin, field_readonly, styled_card},
    },
};

use super::shared::push_optional_field;

pub(in super::super) fn identity_card<'a>(
    identity: &'a IdentityView,
    colors: &AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let mut fields: Vec<Element<'a, CipherDetailMessage, AppTheme>> = Vec::new();

    let full_name = [
        identity.title.as_deref(),
        identity.first_name.as_deref(),
        identity.middle_name.as_deref(),
        identity.last_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    if !full_name.is_empty() {
        fields.push(field_readonly(fl!("detail-field-name"), full_name, colors));
    }

    push_optional_field(
        &mut fields,
        fl!("detail-field-email"),
        identity.email.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-phone"),
        identity.phone.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-company"),
        identity.company.as_deref(),
        colors,
    );

    let address_lines = [
        identity.address1.as_deref(),
        identity.address2.as_deref(),
        identity.address3.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ");
    if !address_lines.is_empty() {
        fields.push(field_readonly(
            fl!("detail-field-address"),
            address_lines,
            colors,
        ));
    }

    let locality = [
        identity.city.as_deref(),
        identity.state.as_deref(),
        identity.postal_code.as_deref(),
        identity.country.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ");
    if !locality.is_empty() {
        fields.push(field_readonly(
            fl!("detail-field-city-region"),
            locality,
            colors,
        ));
    }

    if fields.is_empty() {
        fields.push(
            text(fl!("detail-empty-identity"))
                .size(crate::text_size(14))
                .color(colors.text_muted)
                .into(),
        );
    }
    card_with_margin(styled_card(column(fields).spacing(12).width(Fill)))
}
