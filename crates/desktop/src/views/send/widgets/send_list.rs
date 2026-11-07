//! Send list widget: name + deletion date + 3-dots options column, plus the
//! search bar. Mirrors the vault item list but with its own column schema.

use std::sync::Arc;

use bitwarden_send::{SendType, SendView as SdkSendView};
use chrono::{DateTime, Utc};
use iced::{
    Alignment, Background, Border, Element, Fill, widget,
    widget::{
        Space, column, container, row, scrollable, text,
        text::{Ellipsis, Wrapping},
        text_input,
    },
};

use crate::{
    components::{self, buttons, icons, virtual_list},
    fl,
    theme::{AppColors, AppTheme, RADIUS_MD},
};

/// Shared widget-id for the send search input. Used by the File menu's
/// "Search sends" action to focus the field via `widget::operation::focus`.
pub const SEND_SEARCH_ID: widget::Id = widget::Id::new("send-search");

const ROW_HEIGHT: f32 = 49.0;

#[derive(Debug, Clone)]
pub enum SearchMessage {
    QueryChanged(String),
}

#[derive(Debug, Clone)]
pub enum SendListMessage {
    ItemSelected(usize),
    MoreOptions(#[expect(dead_code)] usize),
    Scrolled(scrollable::Viewport),
}

// ── Search bar ─────────────────────────────────────────────────────────────

pub fn search_view<'a>(query: &'a str) -> Element<'a, SearchMessage, AppTheme> {
    text_input(&fl!("send-search-placeholder"), query)
        .id(SEND_SEARCH_ID)
        .on_input(SearchMessage::QueryChanged)
        .icon(icons::SEARCH.input_icon(16.0, text_input::Side::Left))
        .padding([10, 12])
        .size(14)
        .style(|theme: &AppTheme, _status| text_input::Style {
            background: Background::Color(theme.colors.background),
            border: Border::default()
                .color(theme.colors.border)
                .width(1.0)
                .rounded(6),
            icon: theme.colors.text_secondary,
            placeholder: theme.colors.text_secondary,
            value: theme.colors.text_primary,
            selection: theme.colors.accent,
        })
        .into()
}

// ── List view ──────────────────────────────────────────────────────────────

pub const LIST_ID: iced::widget::Id = iced::widget::Id::new("send-item-list");

pub fn view<'a>(
    items: &'a [Arc<SdkSendView>],
    selected_index: Option<usize>,
    scroll: virtual_list::ScrollState,
    colors: &'a AppColors,
) -> Element<'a, SendListMessage, AppTheme> {
    let header = container(
        row![
            text(fl!("send-column-name"))
                .size(14)
                .color(colors.table_header)
                .font(crate::APP_FONT_BOLD)
                .width(Fill),
            text(fl!("send-column-deletion"))
                .size(14)
                .color(colors.table_header)
                .font(crate::APP_FONT_BOLD),
            Space::new().width(64.0),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([8, 24])
    .width(Fill);

    let divider = container(components::separator_h()).padding([0, 16]);

    let list = virtual_list::view(
        items,
        scroll,
        ROW_HEIGHT,
        |i, item| row_element(i, item, selected_index == Some(i), colors),
        SendListMessage::Scrolled,
    )
    .height(Fill)
    .id(LIST_ID)
    .style(components::rail_scroll_style);

    column![header, divider, list]
        .width(Fill)
        .height(Fill)
        .into()
}

/// Empty-state body shown when the user has no sends at all. Mirrors the
/// vault's behaviour of keeping the table header but letting the body
/// breathe.
pub fn empty_state<'a>(colors: &'a AppColors) -> Element<'a, SendListMessage, AppTheme> {
    container(
        text(fl!("send-empty-body"))
            .size(14)
            .color(colors.text_secondary),
    )
    .padding([48, 24])
    .width(Fill)
    .align_x(Alignment::Center)
    .into()
}

fn row_element<'a>(
    i: usize,
    item: &'a SdkSendView,
    is_selected: bool,
    colors: &'a AppColors,
) -> Element<'a, SendListMessage, AppTheme> {
    let type_icon = match item.r#type {
        SendType::Text => icons::FILE_TEXT,
        SendType::File => icons::FILE_EARMARK,
        SendType::Item => icons::FILE_EARMARK,
    }
    .render(20.0, colors.text_primary);

    let name = text(item.name.as_str())
        .size(14)
        .color(colors.text_primary)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End);

    let deletion = text(format_deletion_date(&item.deletion_date))
        .size(14)
        .color(colors.text_secondary)
        .wrapping(Wrapping::None);

    let more = buttons::ghost_icon(
        icons::THREE_DOTS_VERTICAL.render(14.0, colors.text_secondary),
        colors.item_hover,
    )
    .on_press(SendListMessage::MoreOptions(i))
    .padding([4, 6]);

    let content = row![
        type_icon,
        column![name].width(Fill),
        deletion,
        Space::new().width(8.0),
        more,
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let button = buttons::ghost(
        content,
        is_selected,
        colors.item_hover,
        colors.item_hover,
        RADIUS_MD,
    )
    .on_press(SendListMessage::ItemSelected(i))
    .padding([8, 8])
    .width(Fill);

    container(column![button, components::separator_h()].width(Fill))
        .padding([0, 16])
        .into()
}

/// Human-readable deletion date, e.g. "5/1/26, 6:09 PM". Matches the
/// screenshot reference without pulling in a full i18n formatter.
pub fn format_deletion_date(dt: &DateTime<Utc>) -> String {
    let local: DateTime<chrono::Local> = (*dt).into();
    local.format("%-m/%-d/%y, %-I:%M %p").to_string()
}
