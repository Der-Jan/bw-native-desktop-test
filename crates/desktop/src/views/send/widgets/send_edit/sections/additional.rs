//! "Additional options" card — view limit, hide-email flag, private notes.

use iced::{
    Element, Length,
    widget::{checkbox, text, text_editor},
};

use crate::{
    components::inputs,
    fl,
    theme::{AppColors, AppTheme},
    views::send::widgets::send_edit::{SendEditMessage, SendForm},
};

use crate::components::section_card;
use iced::widget::column;

pub(in super::super) fn additional_options_card<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    let mut items: Vec<Element<'a, SendEditMessage, AppTheme>> = Vec::new();
    items.push(limit_views_field(form, colors));
    items.push(views_left_hint(form, colors).unwrap_or_else(|| {
        text(fl!("send-form-limit-views-hint"))
            .size(crate::text_size(12))
            .color(colors.text_secondary)
            .into()
    }));
    items.push(
        checkbox(form.hide_email)
            .label(fl!("send-form-hide-email"))
            .on_toggle(SendEditMessage::HideEmailToggled)
            .size(crate::text_size(18))
            .spacing(8)
            .into(),
    );
    items.push(notes_field(form, colors));

    section_card(
        fl!("send-form-additional-heading"),
        column(items).spacing(12),
        colors,
    )
}

// ── Fields ────────────────────────────────────────────────────────────────

fn limit_views_field<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    inputs::stepper_field(
        fl!("send-form-limit-views"),
        &form.max_access_count_raw,
        SendEditMessage::MaxAccessCountChanged,
        SendEditMessage::MaxAccessCountIncrement,
        SendEditMessage::MaxAccessCountDecrement,
        false,
        colors,
    )
}

fn views_left_hint<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Option<Element<'a, SendEditMessage, AppTheme>> {
    let left = form.views_left()?;
    Some(
        text(fl!("send-form-views-left", count = left.to_string()))
            .size(crate::text_size(12))
            .color(colors.text_secondary)
            .into(),
    )
}

fn notes_field<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    let mut editor = text_editor(&form.notes_content)
        .padding([10, 12])
        .height(Length::Shrink)
        .min_height(80.0)
        .max_height(240.0);
    if !form.saving {
        editor = editor.on_action(SendEditMessage::NotesAction);
    }
    inputs::field_frame(fl!("send-form-private-note"), editor, colors)
}
