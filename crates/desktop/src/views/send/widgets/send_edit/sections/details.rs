//! "Details" card — name, type-specific body (text / file), deletion date,
//! access type, and the optional password / emails / send-link fields that
//! the chosen access type turns on.

use bitwarden_send::SendType;
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Length,
    widget::{Space, checkbox, column, row, text, text_editor, text_input},
};

use crate::{
    components::{buttons, icons, inputs},
    fl,
    theme::{AppColors, AppTheme},
    views::send::widgets::send_edit::{
        SendEditMessage, SendForm,
        state::{AccessType, DeletionPreset},
    },
};

use crate::components::section_card;

pub(in super::super) fn details_card<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    let mut items: Vec<Element<'a, SendEditMessage, AppTheme>> = Vec::new();

    items.push(name_field(form, colors));

    match form.send_type {
        SendType::Text => {
            items.push(text_to_share_field(form, colors));
            items.push(
                checkbox(form.text_hidden)
                    .label(fl!("send-form-hide-text"))
                    .on_toggle(SendEditMessage::HideTextToggled)
                    .size(18)
                    .spacing(8)
                    .into(),
            );
        }
        SendType::File => {
            items.push(file_section(form, colors));
        }
        SendType::Item => {
            items.push(text(fl!("send-item-unsupported")).into());
        }
    }

    items.push(deletion_date_field(form, colors));
    items.push(
        text(fl!(
            "send-form-deletion-hint",
            date =
                crate::views::send::widgets::send_list::format_deletion_date(&form.deletion_date)
        ))
        .size(12)
        .color(colors.text_secondary)
        .into(),
    );

    items.push(access_type_field(form, colors));

    if matches!(form.access_type, AccessType::Password) {
        items.push(
            text(fl!("send-form-password-hint"))
                .size(12)
                .color(colors.text_secondary)
                .into(),
        );
        items.push(password_field(form, colors));
    }

    if matches!(form.access_type, AccessType::People) {
        items.push(emails_field(form, colors));
    }

    if let Some(link) = form.send_link() {
        items.push(send_link_field(link, colors));
    }

    section_card(
        fl!("send-form-details-heading"),
        iced::widget::column(items).spacing(12),
        colors,
    )
}

// ── Fields ────────────────────────────────────────────────────────────────

fn name_field<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    let mut input = inputs::bare_text_input(&form.name);
    if !form.saving {
        input = input.on_input(SendEditMessage::NameChanged);
    }
    inputs::field_frame(fl!("send-form-name"), input, colors)
}

fn text_to_share_field<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    let mut editor = text_editor(&form.text_content)
        .padding([10, 12])
        .height(Length::Shrink)
        .min_height(80.0)
        .max_height(240.0);
    if !form.saving {
        editor = editor.on_action(SendEditMessage::TextAction);
    }
    inputs::field_frame(fl!("send-form-text"), editor, colors)
}

fn file_section<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    let is_new = form.id.is_none();
    if is_new {
        // New file send: the filename + size are "chosen" via a picker.
        // The picker is stubbed — see docs/todo.md.
        let placeholder = text(fl!("send-form-file-choose-placeholder"))
            .size(14)
            .color(colors.text_secondary);
        let choose_btn = buttons::secondary(text(fl!("send-form-file-choose")).size(14))
            .on_press(SendEditMessage::ChooseFilePressed)
            .padding([8, 16]);
        row![placeholder, Space::new().width(Fill), choose_btn]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
    } else {
        // Editing existing file: show name + size, non-editable. Skipping
        // `.on_input` is what makes the field readonly — no placeholder
        // closure needed.
        let name =
            inputs::text_field(fl!("send-form-file-name"), &form.file_name, colors).disabled(true);
        let size = inputs::text_field(
            fl!("send-form-file-size"),
            form.file_size_name.as_deref().unwrap_or(""),
            colors,
        )
        .disabled(true);
        column![name, size].spacing(12).into()
    }
}

fn deletion_date_field<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    inputs::select_field(
        fl!("send-form-deletion-date"),
        Some(form.deletion_preset),
        DeletionPreset::ALL.to_vec(),
        |preset| preset_label(*preset),
        SendEditMessage::DeletionPresetChosen,
        colors,
    )
}

fn access_type_field<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    inputs::select_field(
        fl!("send-form-who-can-view"),
        Some(form.access_type),
        AccessType::ALL.to_vec(),
        |t| access_type_label(*t),
        SendEditMessage::AccessTypeChosen,
        colors,
    )
}

fn password_field<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    let mut input =
        inputs::bare_text_input(&form.password).on_input(SendEditMessage::PasswordChanged);
    if !form.password_revealed {
        input = input.secure(true);
    }

    let eye_icon = if form.password_revealed {
        icons::EYE
    } else {
        icons::EYE_SLASH
    };
    let eye_btn = buttons::ghost_icon(
        eye_icon.render(16.0, colors.text_secondary),
        colors.item_hover,
    )
    .on_press(SendEditMessage::PasswordRevealToggled)
    .padding([10, 8]);

    let regen_btn = buttons::ghost_icon(
        icons::ARROW_CLOCKWISE.render(16.0, colors.text_secondary),
        colors.item_hover,
    )
    .on_press(SendEditMessage::PasswordRegenerate)
    .padding([10, 8]);

    let copy_btn = buttons::ghost_icon(
        icons::COPY.render(16.0, colors.text_secondary),
        colors.item_hover,
    )
    .on_press(SendEditMessage::PasswordCopy)
    .padding([10, 8]);

    let row_el = row![input, eye_btn, regen_btn, copy_btn].align_y(Alignment::Center);

    inputs::field_frame(fl!("send-form-password"), row_el, colors)
}

fn emails_field<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    let mut editor = text_editor(&form.emails_content)
        .padding([10, 12])
        .height(Length::Shrink)
        .min_height(60.0)
        .max_height(140.0);
    if !form.saving {
        editor = editor.on_action(SendEditMessage::EmailsAction);
    }
    inputs::field_frame(fl!("send-form-emails"), editor, colors)
}

fn send_link_field<'a>(
    url: String,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    let input = text_input("", &url)
        .size(14)
        .padding([10, 12])
        .width(Fill)
        .style(|theme: &AppTheme, _status| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: theme.colors.text_muted,
            placeholder: theme.colors.text_secondary,
            value: theme.colors.text_primary,
            selection: theme.colors.accent,
        });
    let copy = buttons::ghost_icon(
        icons::COPY.render(16.0, colors.text_secondary),
        colors.item_hover,
    )
    .on_press(SendEditMessage::CopyLinkPressed)
    .padding([10, 8]);
    let row_el = row![input, copy].align_y(Alignment::Center);
    inputs::field_frame(fl!("send-form-send-link"), row_el, colors)
}

// ── Label helpers ─────────────────────────────────────────────────────────

fn access_type_label(t: AccessType) -> String {
    match t {
        AccessType::Link => fl!("send-form-access-link"),
        AccessType::People => fl!("send-form-access-people"),
        AccessType::Password => fl!("send-form-access-password"),
    }
}

fn preset_label(p: DeletionPreset) -> String {
    match p {
        DeletionPreset::OneHour => fl!("send-form-preset-1h"),
        DeletionPreset::OneDay => fl!("send-form-preset-1d"),
        DeletionPreset::TwoDays => fl!("send-form-preset-2d"),
        DeletionPreset::ThreeDays => fl!("send-form-preset-3d"),
        DeletionPreset::SevenDays => fl!("send-form-preset-7d"),
        DeletionPreset::FourteenDays => fl!("send-form-preset-14d"),
        DeletionPreset::ThirtyDays => fl!("send-form-preset-30d"),
    }
}
