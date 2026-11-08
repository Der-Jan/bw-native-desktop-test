//! Card-type read-only section.

use bitwarden_vault::CardView;
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
        field_helpers::{card_with_margin, field_readonly, styled_card},
    },
};

use super::shared::push_optional_field;

pub(in super::super) fn card_details_card<'a>(
    card: &'a CardView,
    colors: &'a AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let mut fields: Vec<Element<'a, CipherDetailMessage, AppTheme>> = Vec::new();
    push_optional_field(
        &mut fields,
        fl!("detail-field-cardholder-name"),
        card.cardholder_name.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-brand"),
        card.brand.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-number"),
        card.number.as_deref(),
        colors,
    );

    let expiration = match (card.exp_month.as_deref(), card.exp_year.as_deref()) {
        (Some(m), Some(y)) => Some(format!("{m}/{y}")),
        (Some(m), None) => Some(m.to_string()),
        (None, Some(y)) => Some(y.to_string()),
        (None, None) => None,
    };
    if let Some(exp) = expiration {
        fields.push(field_readonly(fl!("detail-field-expiration"), exp, colors));
    }

    if let Some(code) = card.code.as_deref() {
        fields.push(reveal_field(
            fl!("detail-field-security-code"),
            code,
            None,
            colors,
        ));
    }

    if fields.is_empty() {
        fields.push(
            text(fl!("detail-empty-card"))
                .size(crate::text_size(14))
                .color(colors.text_muted)
                .into(),
        );
    }
    card_with_margin(styled_card(column(fields).spacing(12).width(Fill)))
}
