use iced::{
    Element, Fill,
    widget::{Space, column, row},
};

use crate::{
    components::{self, inputs},
    fl,
    theme::{AppColors, AppTheme},
};

use super::{
    super::{GeneratorMessage, state::PasswordForm, view::section_heading},
    tab_checkbox,
};

pub(in super::super) fn view<'a>(
    form: &'a PasswordForm,
    colors: &'a AppColors,
) -> Element<'a, GeneratorMessage, AppTheme> {
    let length = inputs::stepper_field(
        fl!("generator-length"),
        &form.length,
        GeneratorMessage::SetLength,
        GeneratorMessage::BumpLength(1),
        GeneratorMessage::BumpLength(-1),
        false,
        colors,
    );

    let length_hint = iced::widget::text(fl!("generator-length-hint"))
        .size(crate::text_size(12))
        .color(colors.text_muted);

    let include_heading = section_heading(fl!("generator-include"), colors);
    let include_row = row![
        tab_checkbox(
            form.uppercase,
            fl!("generator-include-uppercase"),
            GeneratorMessage::ToggleUppercase,
        ),
        tab_checkbox(
            form.lowercase,
            fl!("generator-include-lowercase"),
            GeneratorMessage::ToggleLowercase,
        ),
        tab_checkbox(
            form.numbers,
            fl!("generator-include-numbers"),
            GeneratorMessage::ToggleNumbers,
        ),
        tab_checkbox(
            form.special,
            fl!("generator-include-special"),
            GeneratorMessage::ToggleSpecial,
        ),
    ]
    .spacing(16)
    .width(Fill);

    let min_number = inputs::stepper_field(
        fl!("generator-min-number"),
        &form.min_number,
        GeneratorMessage::SetMinNumber,
        GeneratorMessage::BumpMinNumber(1),
        GeneratorMessage::BumpMinNumber(-1),
        !form.numbers,
        colors,
    );
    let min_special = inputs::stepper_field(
        fl!("generator-min-special"),
        &form.min_special,
        GeneratorMessage::SetMinSpecial,
        GeneratorMessage::BumpMinSpecial(1),
        GeneratorMessage::BumpMinSpecial(-1),
        !form.special,
        colors,
    );

    let avoid = tab_checkbox(
        form.avoid_ambiguous,
        fl!("generator-avoid-ambiguous"),
        GeneratorMessage::ToggleAvoidAmbiguous,
    );

    let length_card =
        components::styled_card(column![length, Space::new().height(2), length_hint].width(Fill));

    let include_card = components::styled_card(
        column![
            include_heading,
            Space::new().height(6),
            include_row,
            Space::new().height(12),
            row![min_number, min_special].spacing(12).width(Fill),
            Space::new().height(10),
            avoid,
        ]
        .width(Fill),
    );

    column![
        components::card_with_margin(length_card),
        components::card_with_margin(include_card),
    ]
    .width(Fill)
    .into()
}
