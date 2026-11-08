//! Inline keybind chips and hint rows. Reused by the searching footer and
//! the locked-state pill — same visual style, different content.

use iced::{
    Alignment, Background, Border, Color, Element,
    widget::{container, row, text},
};

use crate::{
    theme::{AppColors, AppTheme, MAGNIFY_OVERLAY_ALPHA_FAINT, RADIUS_SM},
    views::magnify::message::MagnifyMessage,
};

/// Keybind indicator chip — a small bordered pill containing either text
/// (`"Ctrl+C"`, `"esc"`) or a glyph (the return-arrow icon for `Enter`).
/// `content` is generic over `Into<Element>` so call sites can pass either.
pub fn keybind<'a>(
    content: impl Into<Element<'a, MagnifyMessage, AppTheme>>,
) -> Element<'a, MagnifyMessage, AppTheme> {
    container(content)
        .padding([2, 6])
        .style(|theme: &AppTheme| {
            container::Style::default()
                .background(Background::Color(Color {
                    a: MAGNIFY_OVERLAY_ALPHA_FAINT,
                    ..Color::WHITE
                }))
                .border(
                    Border::default()
                        .color(theme.colors.border)
                        .width(1.0)
                        .rounded(RADIUS_SM),
                )
        })
        .into()
}

/// Convenience: `keybind` wrapping a 12-pt text label.
pub fn keybind_text<'a>(
    label: &'static str,
    colors: &'a AppColors,
) -> Element<'a, MagnifyMessage, AppTheme> {
    keybind(
        text(label)
            .size(crate::text_size(12))
            .color(colors.text_primary),
    )
}

/// One inline hint: keybind chip + plain-text description, used by the
/// footer ("Ctrl+C  Copy password") and the locked-state pill
/// ("↵  Open Bitwarden").
pub fn hint<'a>(
    chip: Element<'a, MagnifyMessage, AppTheme>,
    description: String,
    colors: &'a AppColors,
) -> Element<'a, MagnifyMessage, AppTheme> {
    row![
        chip,
        text(description)
            .size(crate::text_size(12))
            .color(colors.text_secondary),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}
