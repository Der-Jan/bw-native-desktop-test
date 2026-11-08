//! Section cards shared by every cipher type:
//! - `item_details_card` (name, favorite, folder, organization, collections)
//! - `additional_options_card` (notes + master-password reprompt toggle)
//! - `custom_fields_card` (text / hidden / boolean fields, add/remove)

use bitwarden_vault::{CipherRepromptType, FieldType, FieldView};
use iced::{
    Alignment, Element, Fill, Length,
    widget::{checkbox, column, container, row, text, text_editor},
};

use crate::{
    components::{
        buttons,
        inputs::{reveal_text_field, select_field, text_field},
    },
    fl,
    theme::{AppColors, AppTheme},
    views::vault::widgets::{
        cipher_edit::{CipherEditMessage, CipherForm, NAME_INPUT_ID, selectors},
        field_helpers::{card_with_margin, styled_card},
    },
};

pub(in super::super) fn item_details_card<'a>(
    form: &'a CipherForm,
    colors: &'a AppColors,
) -> Element<'a, CipherEditMessage, AppTheme> {
    let favorite_checkbox = checkbox(form.modified.favorite)
        .label(fl!("form-favorite"))
        .on_toggle(|_| CipherEditMessage::FavoriteToggled)
        .size(crate::text_size(18))
        .spacing(8);

    let mut body = column![
        text_field(fl!("form-name"), &form.modified.name, colors)
            .id(NAME_INPUT_ID.clone())
            .on_input(CipherEditMessage::NameChanged)
            .disabled(form.saving),
        favorite_checkbox,
        // Folder dropdown (personal vault only — orgs own their own folder concept)
        selectors::folder_selector(form, colors),
    ]
    .spacing(12);

    if !form.organizations.is_empty() {
        body = body.push(selectors::org_selector(form, colors));

        if form.modified.organization_id.is_some() {
            body = body.push(selectors::collections_selector(form, colors));
        }
    }

    card_with_margin(styled_card(body))
}

pub(in super::super) fn additional_options_card<'a>(
    form: &'a CipherForm,
    colors: &'a AppColors,
) -> Element<'a, CipherEditMessage, AppTheme> {
    // Multi-line notes field — `text_editor` grows with content between
    // `min_height` and `max_height`. Border/label come from `field_frame`
    // so the visual matches the single-line inputs; we strip the editor's
    // own border via `text_editor::Catalog`'s default style.
    let mut notes_editor = text_editor(&form.notes_content)
        .padding([10, 12])
        .height(Length::Shrink)
        .min_height(80.0)
        .max_height(600.0);
    if !form.saving {
        notes_editor = notes_editor.on_action(CipherEditMessage::NotesAction);
    }
    let notes = crate::components::inputs::field_frame(fl!("form-notes"), notes_editor, colors);

    let reprompt_checkbox = checkbox(matches!(
        form.modified.reprompt,
        CipherRepromptType::Password
    ))
    .label(fl!("form-reprompt"))
    .on_toggle(|_| CipherEditMessage::RepromptToggled)
    .size(crate::text_size(18))
    .spacing(8);

    let body = column![notes, reprompt_checkbox].spacing(12);

    card_with_margin(styled_card(body))
}

pub(in super::super) fn custom_fields_card<'a>(
    form: &'a CipherForm,
    colors: &'a AppColors,
) -> Element<'a, CipherEditMessage, AppTheme> {
    let fields = form.modified.fields.as_deref().unwrap_or(&[]);

    let mut body = column![].spacing(12);

    if fields.is_empty() {
        body = body.push(
            text(fl!("form-custom-field-empty"))
                .size(crate::text_size(14))
                .color(colors.text_muted),
        );
    } else {
        for (idx, f) in fields.iter().enumerate() {
            body = body.push(custom_field_row(idx, f, form, colors));
        }
    }

    body = body.push(
        crate::views::vault::widgets::field_helpers::add_item_button(
            fl!("form-add-custom-field"),
            CipherEditMessage::CustomFieldAdded,
            colors,
        ),
    );

    card_with_margin(styled_card(body))
}

fn custom_field_row<'a>(
    idx: usize,
    f: &'a FieldView,
    form: &'a CipherForm,
    colors: &'a AppColors,
) -> Element<'a, CipherEditMessage, AppTheme> {
    let type_picker: Element<'a, CipherEditMessage, AppTheme> = container(select_field(
        fl!("form-custom-field-type"),
        Some(f.r#type),
        vec![FieldType::Text, FieldType::Hidden, FieldType::Boolean],
        |ty: &FieldType| match ty {
            FieldType::Text => fl!("form-custom-field-type-text"),
            FieldType::Hidden => fl!("form-custom-field-type-hidden"),
            FieldType::Boolean => fl!("form-custom-field-type-boolean"),
            FieldType::Linked => fl!("form-custom-field-type-linked"),
        },
        move |ty| CipherEditMessage::CustomFieldTypeSelected(idx, ty),
        colors,
    ))
    .width(140)
    .into();

    let name_input = text_field(
        fl!("form-custom-field-name"),
        f.name.as_deref().unwrap_or(""),
        colors,
    )
    .on_input(move |s| CipherEditMessage::CustomFieldNameChanged(idx, s))
    .disabled(form.saving);

    let value_widget: Element<'a, CipherEditMessage, AppTheme> = match f.r#type {
        FieldType::Text => text_field(
            fl!("form-custom-field-value"),
            f.value.as_deref().unwrap_or(""),
            colors,
        )
        .on_input(move |s| CipherEditMessage::CustomFieldValueChanged(idx, s))
        .disabled(form.saving)
        .into(),
        FieldType::Hidden => reveal_text_field(
            fl!("form-custom-field-value"),
            f.value.as_deref().unwrap_or(""),
            move |s| CipherEditMessage::CustomFieldValueChanged(idx, s),
            form.saving,
            colors,
        ),
        FieldType::Boolean => {
            let checked = matches!(f.value.as_deref(), Some("true"));
            checkbox(checked)
                .label(fl!("form-custom-field-enabled"))
                .on_toggle(move |_| CipherEditMessage::CustomFieldBoolToggled(idx))
                .size(crate::text_size(18))
                .spacing(8)
                .into()
        }
        FieldType::Linked => text(fl!("form-custom-field-linked-unsupported"))
            .size(crate::text_size(12))
            .color(colors.text_muted)
            .into(),
    };

    let remove_btn =
        buttons::delete_icon_button(CipherEditMessage::CustomFieldRemoved(idx), colors);

    row![
        type_picker,
        container(name_input).width(Fill),
        container(value_widget).width(Fill),
        remove_btn,
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}
