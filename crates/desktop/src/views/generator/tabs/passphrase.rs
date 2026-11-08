use iced::{
    Element, Fill,
    widget::{Space, column},
};

use crate::{
    components::{self, inputs},
    fl,
    theme::{AppColors, AppTheme},
};

use super::{
    super::{GeneratorMessage, state::PassphraseForm},
    tab_checkbox,
};

pub(in super::super) fn view<'a>(
    form: &'a PassphraseForm,
    colors: &'a AppColors,
) -> Element<'a, GeneratorMessage, AppTheme> {
    let num_words = inputs::stepper_field(
        fl!("generator-num-words"),
        &form.num_words,
        GeneratorMessage::SetNumWords,
        GeneratorMessage::BumpNumWords(1),
        GeneratorMessage::BumpNumWords(-1),
        false,
        colors,
    );
    let num_hint = iced::widget::text(fl!("generator-num-words-hint"))
        .size(crate::text_size(12))
        .color(colors.text_muted);

    let separator = inputs::text_field(
        fl!("generator-word-separator"),
        &form.word_separator,
        colors,
    )
    .on_input(GeneratorMessage::SetWordSeparator);

    let capitalize = tab_checkbox(
        form.capitalize,
        fl!("generator-passphrase-capitalize"),
        GeneratorMessage::TogglePassphraseCapitalize,
    );
    let include_number = tab_checkbox(
        form.include_number,
        fl!("generator-passphrase-include-number"),
        GeneratorMessage::TogglePassphraseIncludeNumber,
    );

    let num_card =
        components::styled_card(column![num_words, Space::new().height(2), num_hint].width(Fill));

    let extras_card = components::styled_card(
        column![
            separator,
            Space::new().height(12),
            capitalize,
            Space::new().height(6),
            include_number,
        ]
        .width(Fill),
    );

    column![
        components::card_with_margin(num_card),
        components::card_with_margin(extras_card),
    ]
    .width(Fill)
    .into()
}
