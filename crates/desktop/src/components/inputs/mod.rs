//! Reusable labeled input primitives shared across views.
//!
//! Naming convention: every public helper ends in `_field` (or is
//! [`field_frame`], the shared visual primitive). Fields with a show/hide
//! eye toggle are prefixed `reveal_*` — `reveal_text_field` for editable
//! inputs and `reveal_field` for read-only display. All widgets compose
//! through [`field_frame`] so the floating-label chip looks identical
//! regardless of what's inside.
//!
//! ## Layout
//!
//! - This file: frame primitives ([`field_frame`], [`field_frame_on`],
//!   [`errored_field_frame`], [`errored_field_frame_on`], [`field_error_row`],
//!   [`bare_text_input`], [`readonly_field_truncated`]).
//! - [`text`] — [`text_field`] / [`stepper_field`].
//! - [`select`] — [`select_field`] / [`search_select_field`] /
//!   [`multi_select_field`].
//! - [`reveal`] — [`reveal_text_field`] / [`reveal_field`] (eye toggle).

#![expect(
    deprecated,
    reason = "`Component` is deprecation-tagged upstream; see `reveal_text_field` docs."
)]

mod entry;
mod reveal;
mod select;

use iced::{
    Alignment, Background, Border, Color, Element, Fill, Length,
    widget::{
        Space, TextInput, column, container, row, stack, text,
        text::{Ellipsis, Wrapping},
        text_input,
    },
};

use crate::{
    components::icons,
    theme::{AppColors, AppTheme},
};

pub use entry::{stepper_field, text_field};
pub use reveal::{reveal_field, reveal_text_field, reveal_text_field_with_submit};
pub use select::{multi_select_field, search_select_field, select_field, select_field_on};

/// Preconfigured `text_input` matching our floating-label look:
/// transparent background + no border (the wrapping [`field_frame`] draws
/// the border).
pub fn bare_text_input<'a, M: Clone + 'a>(value: &'a str) -> TextInput<'a, M, AppTheme> {
    text_input("", value)
        .size(crate::text_size(16))
        .padding([10, 12])
        .width(Fill)
        .style(|theme: &AppTheme, _status| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: theme.colors.text_muted,
            placeholder: theme.colors.text_secondary,
            value: theme.colors.text_primary,
            selection: theme.colors.accent,
        })
}

/// Stacked label + single-line truncated value. Read-only display for long
/// unbroken strings (SSH keys, URIs, hashes, fingerprints) — a normal
/// `text()` would wrap on word boundaries or overflow the container. The
/// caller should provide a copy action alongside.
pub fn readonly_field_truncated<'a, M: 'a>(
    label: impl Into<String>,
    value: impl iced::widget::text::IntoFragment<'a>,
    colors: &AppColors,
) -> Element<'a, M, AppTheme> {
    column![
        text(label.into())
            .size(crate::text_size(12))
            .color(colors.text_muted),
        text(value)
            .size(crate::text_size(14))
            .color(colors.text_primary)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End),
    ]
    .spacing(2)
    // `Fill` is required for ellipsis to kick in — without it the text
    // widget is content-sized and lays out the full un-truncated string.
    .width(Fill)
    .into()
}

/// Wrap any content in the floating-label frame: a bordered container with
/// a small label chip stacked on top of the border. The chip's background
/// must match whatever surface sits behind the field so the border visually
/// breaks behind it — pass [`field_frame_on`] if the field doesn't sit on
/// `colors.background`.
///
/// The output is wrapped in a [`crate::components::shell_scope::ShellScope`]
/// so each field gets its own private event-status flag — without it, two
/// `pick_list`-bearing `field_frame`s as siblings interact badly: iced's
/// `Stack` short-circuits its between-children iteration on the *shared*
/// shell's `is_event_captured()`, so a sibling's earlier capture stops the
/// later sibling from receiving the event. See CLAUDE.md → "Shell Capture
/// Isolation (ShellScope)".
pub fn field_frame<'a, M: 'a>(
    label: impl Into<String>,
    content: impl Into<Element<'a, M, AppTheme>>,
    colors: &'a AppColors,
) -> Element<'a, M, AppTheme> {
    field_frame_on(label, content, |c| c.background, colors)
}

/// [`field_frame`] with a caller-chosen chip background. Use when the
/// field sits on a surface other than `colors.background` (e.g. directly
/// on a dialog whose body uses `card_bg`); pass the same colour token the
/// surrounding container paints.
///
/// `chip_bg` is a closure rather than a pre-resolved `Color` so the chip
/// re-themes correctly on light/dark switches.
pub fn field_frame_on<'a, M: 'a>(
    label: impl Into<String>,
    content: impl Into<Element<'a, M, AppTheme>>,
    chip_bg: impl Fn(&AppColors) -> Color + 'static,
    colors: &'a AppColors,
) -> Element<'a, M, AppTheme> {
    field_frame_styled(label, content, chip_bg, false, colors)
}

/// [`field_frame`] in the validation-error state: red border + red label
/// chip. Pair with an inline error message rendered separately (e.g. via
/// [`field_error_row`]) so the user knows what failed validation.
pub fn errored_field_frame<'a, M: 'a>(
    label: impl Into<String>,
    content: impl Into<Element<'a, M, AppTheme>>,
    colors: &'a AppColors,
) -> Element<'a, M, AppTheme> {
    field_frame_styled(label, content, |c| c.background, true, colors)
}

/// [`errored_field_frame`] with a caller-chosen chip background. Use when
/// the errored field sits on a surface other than `colors.background`.
pub fn errored_field_frame_on<'a, M: 'a>(
    label: impl Into<String>,
    content: impl Into<Element<'a, M, AppTheme>>,
    chip_bg: impl Fn(&AppColors) -> Color + 'static,
    colors: &'a AppColors,
) -> Element<'a, M, AppTheme> {
    field_frame_styled(label, content, chip_bg, true, colors)
}

fn field_frame_styled<'a, M: 'a>(
    label: impl Into<String>,
    content: impl Into<Element<'a, M, AppTheme>>,
    chip_bg: impl Fn(&AppColors) -> Color + 'static,
    errored: bool,
    colors: &'a AppColors,
) -> Element<'a, M, AppTheme> {
    let label_color = if errored {
        colors.danger
    } else {
        colors.text_secondary
    };
    let floating_label = container(
        text(label.into())
            .size(crate::text_size(14))
            .color(label_color),
    )
    .padding([0, 4])
    .style(move |theme: &AppTheme| container::Style::default().background(chip_bg(&theme.colors)));

    let bordered = container(content)
        .width(Fill)
        .style(move |theme: &AppTheme| {
            let border_color = if errored {
                theme.colors.danger
            } else {
                theme.colors.border
            };
            container::Style::default()
                .border(Border::default().color(border_color).width(1.0).rounded(4))
        });

    crate::components::shell_scope::ShellScope::new(stack![
        column![Space::new().height(Length::Fixed(8.0)), bordered],
        container(floating_label).padding([0, 12]),
    ])
    .into()
}

/// Inline validation-error row: filled X-circle icon + message text, both in
/// `colors.danger`. Render directly below an [`errored_field_frame`].
pub fn field_error_row<'a, M: 'a>(
    message: impl Into<String>,
    colors: &'a AppColors,
) -> Element<'a, M, AppTheme> {
    row![
        icons::X_CIRCLE_FILL.render(14.0, colors.danger),
        text(message.into())
            .size(crate::text_size(12))
            .color(colors.danger),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}
