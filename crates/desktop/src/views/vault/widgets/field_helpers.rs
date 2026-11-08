//! Small visual primitives shared between `cipher_detail` (read-only view)
//! and `cipher_edit` (editable view). Extracted so both can wear the same
//! section/card/label styling without duplicating the style closures.

use iced::{
    Alignment, Element,
    widget::{row, text},
};

use crate::{
    components::{self, buttons, icons},
    theme::{AppColors, AppTheme},
};

// Re-exports for callers that already import `field_helpers`. `styled_card`
// keeps that name (instead of `card`) to avoid shadowing in scopes that use
// `card` as a parameter name (e.g. `cipher_detail::card_details_card`).
pub use components::{card_with_margin, section_label, styled_card};

/// "+ Add X" secondary button used inside cipher-edit cards (URIs,
/// passkeys, custom fields). 14 px label + 14 px PLUS icon at accent color.
pub fn add_item_button<'a, M: 'a + Clone>(
    label: impl Into<String>,
    msg: M,
    colors: &AppColors,
) -> Element<'a, M, AppTheme> {
    buttons::secondary(
        row![
            icons::PLUS.render(14.0, colors.accent),
            text(label.into()).size(crate::text_size(14)),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(msg)
    .padding([6, 12])
    .into()
}

/// Read-only `{label, value}` stacked pair. Used for displayed-only fields
/// that can wrap naturally — names, notes, multi-line addresses.
pub fn field_readonly<'a, M: 'a>(
    label: impl Into<String>,
    value: impl iced::widget::text::IntoFragment<'a>,
    colors: &AppColors,
) -> Element<'a, M, AppTheme> {
    iced::widget::column![
        text(label.into())
            .size(crate::text_size(12))
            .color(colors.text_muted),
        text(value)
            .size(crate::text_size(14))
            .color(colors.text_primary),
    ]
    .spacing(2)
    .into()
}

/// Format a passkey's creation timestamp in local time using a short,
/// US-style date + 12-hour clock ("1/29/26, 9:26 PM") — matches the
/// convention the official Bitwarden clients use. Shared by both the
/// detail pane (readonly row) and the cipher form (editable row).
pub fn format_passkey_date(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.with_timezone(&chrono::Local)
        .format("%-m/%-d/%y, %-I:%M %p")
        .to_string()
}
