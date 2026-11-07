//! Section cards shared by every cipher type:
//! - `item_details_card` (name + optional notes)
//! - `custom_fields_card` (text / hidden / boolean / linked)
//!
//! Plus the cross-section primitives (`field_with_action`,
//! `push_optional_field`) used by the per-type section modules.

use bitwarden_vault::{CipherType, CipherView, FieldType, FieldView};
use iced::{
    Alignment, Element, Fill,
    widget::{column, row},
};

use crate::{
    components::{
        buttons::icon_button,
        icons,
        inputs::{readonly_field_truncated, reveal_field},
    },
    fl,
    theme::{AppColors, AppTheme},
    views::vault::widgets::{
        cipher_detail::CipherDetailMessage,
        field_helpers::{card_with_margin, field_readonly, styled_card},
    },
};

pub(in super::super) fn item_details_card<'a>(
    item: &'a CipherView,
    colors: &AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let mut fields: Vec<Element<'a, CipherDetailMessage, AppTheme>> =
        vec![field_readonly(fl!("detail-field-name"), &item.name, colors)];
    if let Some(notes) = item.notes.as_deref()
        && !matches!(item.r#type, CipherType::SecureNote)
    {
        fields.push(field_readonly(fl!("detail-field-notes"), notes, colors));
    }
    card_with_margin(styled_card(column(fields).spacing(12).width(Fill)))
}

pub(in super::super) fn custom_fields_card<'a>(
    fields: &'a [FieldView],
    colors: &'a AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let rows: Vec<Element<'a, CipherDetailMessage, AppTheme>> = fields
        .iter()
        .enumerate()
        .map(|(idx, f)| custom_field(idx, f, colors))
        .collect();
    card_with_margin(styled_card(column(rows).spacing(12).width(Fill)))
}

fn custom_field<'a>(
    idx: usize,
    field: &'a FieldView,
    colors: &'a AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let label = field.name.as_deref().unwrap_or("").to_string();
    let raw_value = field.value.as_deref().unwrap_or("");
    match field.r#type {
        FieldType::Hidden => reveal_field(
            label,
            raw_value,
            Some(CipherDetailMessage::CopyCustomField(idx)),
            colors,
        ),
        FieldType::Boolean => {
            let v = if matches!(raw_value, "true") {
                fl!("detail-field-boolean-true")
            } else {
                fl!("detail-field-boolean-false")
            };
            field_with_action(
                label,
                v,
                &[icons::BWI_COPY],
                &[CipherDetailMessage::CopyCustomField(idx)],
                colors,
            )
        }
        FieldType::Text => field_with_action(
            label,
            raw_value,
            &[icons::BWI_COPY],
            &[CipherDetailMessage::CopyCustomField(idx)],
            colors,
        ),
        // Linked fields aren't usefully renderable without resolving the
        // target property name, matching the stance in `cipher_edit`.
        FieldType::Linked => field_readonly(label, raw_value, colors),
    }
}

/// Push a read-only field onto the vec only when `value` is `Some`. Used
/// by the card / identity sections to keep the per-field match compact.
pub(super) fn push_optional_field<'a>(
    fields: &mut Vec<Element<'a, CipherDetailMessage, AppTheme>>,
    label: impl Into<String>,
    value: Option<impl iced::widget::text::IntoFragment<'a>>,
    colors: &AppColors,
) {
    if let Some(v) = value {
        fields.push(field_readonly(label, v, colors));
    }
}

/// Label + truncated value + a trailing row of icon action buttons.
/// Shared by login / autofill / ssh / custom-field cards.
pub(super) fn field_with_action<'a>(
    label: impl Into<String>,
    value: impl iced::widget::text::IntoFragment<'a>,
    icons_list: &[icons::BwiIcon],
    msgs: &[CipherDetailMessage],
    colors: &AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let buttons: Vec<Element<'a, CipherDetailMessage, AppTheme>> = icons_list
        .iter()
        .zip(msgs.iter())
        .map(|(icon, msg)| icon_button(*icon, msg.clone(), colors))
        .collect();

    let buttons_row = row(buttons).spacing(2).align_y(Alignment::Center);

    row![readonly_field_truncated(label, value, colors), buttons_row,]
        .spacing(4)
        .width(Fill)
        .align_y(Alignment::Center)
        .into()
}
