//! History panel for the Generator modal. Renders the flat
//! `Vec<PasswordHistoryEntry>` in reverse-chronological order.

use chrono::{DateTime, Utc};
use iced::{
    Alignment, Border, Element, Fill, Padding,
    widget::{Space, column, container, row, text},
};

use crate::{
    components::{buttons, icons},
    fl,
    services::sdk::PasswordHistoryEntry,
    theme::{AppColors, AppTheme, RADIUS_MD},
};

use super::GeneratorMessage;

pub(super) fn view<'a>(
    history: &'a [PasswordHistoryEntry],
    colors: &'a AppColors,
) -> Element<'a, GeneratorMessage, AppTheme> {
    let back = buttons::ghost_icon(
        icons::CHEVRON_LEFT.render(16.0, colors.text_primary),
        colors.item_hover,
    )
    .padding([6, 6])
    .on_press(GeneratorMessage::BackToGenerator);

    let header = row![
        back,
        text(fl!("generator-history-heading"))
            .size(crate::text_size(14))
            .color(colors.text_secondary),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let body: Element<'a, GeneratorMessage, AppTheme> = if history.is_empty() {
        container(
            text(fl!("generator-history-empty"))
                .size(crate::text_size(14))
                .color(colors.text_muted),
        )
        .padding(Padding::from([24, 16]))
        .into()
    } else {
        let now = Utc::now();
        let mut col = column![].spacing(6);
        for (idx, entry) in history.iter().rev().enumerate() {
            col = col.push(history_row(idx, entry, now, colors));
        }
        col.into()
    };

    let clear = buttons::secondary(text(fl!("generator-history-clear")).size(crate::text_size(14)))
        .padding([8, 16])
        .on_press(GeneratorMessage::ClearHistory);

    column![
        header,
        Space::new().height(12),
        body,
        Space::new().height(16),
        clear,
    ]
    .width(Fill)
    .into()
}

fn history_row<'a>(
    idx: usize,
    entry: &'a PasswordHistoryEntry,
    now: DateTime<Utc>,
    colors: &'a AppColors,
) -> Element<'a, GeneratorMessage, AppTheme> {
    let copy = buttons::ghost_icon(
        icons::BWI_COPY.render(16.0, colors.text_primary),
        colors.item_hover,
    )
    .padding([6, 6])
    .on_press(GeneratorMessage::CopyHistoryEntry(idx));

    let value = text(entry.value.clone())
        .size(crate::text_size(14))
        .font(iced::Font::MONOSPACE)
        .color(colors.text_primary)
        .wrapping(iced::widget::text::Wrapping::None);

    let age = text(format_relative(now.signed_duration_since(entry.created)))
        .size(crate::text_size(12))
        .color(colors.text_muted);

    container(
        row![column![value, age].spacing(2).width(Fill), copy,]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .padding(Padding::from([8, 12]))
    .width(Fill)
    .style(|theme: &AppTheme| {
        container::Style::default()
            .background(theme.colors.card_bg)
            .border(Border::default().rounded(RADIUS_MD))
    })
    .into()
}

fn format_relative(delta: chrono::Duration) -> String {
    let secs = delta.num_seconds().max(0);
    if secs < 60 {
        fl!("generator-history-just-now")
    } else if secs < 3600 {
        let m = secs / 60;
        fl!("generator-history-minutes-ago", count = m)
    } else if secs < 86_400 {
        let h = secs / 3600;
        fl!("generator-history-hours-ago", count = h)
    } else {
        let d = secs / 86_400;
        fl!("generator-history-days-ago", count = d)
    }
}
