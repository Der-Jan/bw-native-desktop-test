use std::sync::Arc;

use bitwarden_vault::{CipherListView, CipherListViewType};
use iced::{
    Alignment, Color, Element, Fill, Length,
    widget::{
        column, container, responsive, row, scrollable, text,
        text::{Ellipsis, Wrapping},
        tooltip,
    },
};

use crate::{
    app::RenderCtx,
    components::{self, buttons, icons, virtual_list},
    fl,
    services::{
        favicon,
        vault_sort::{SortColumn, SortDirection, VaultSort, base_url_for_display},
    },
    theme::{AppColors, AppTheme, RADIUS_MD},
};

pub const LIST_ID: iced::widget::Id = iced::widget::Id::new("vault-item-list");

/// Total pixel height of a single vault row, including its trailing separator.
///
/// MUST match the actual rendered row height — `virtual_list` computes the
/// visible window in multiples of this constant, so drift would break the
/// scroll thumb ratio and cause rows to pop in or out at the wrong spot.
/// The row itself is `content_height + button padding (8+8) = 48`, plus a
/// 1px separator rendered as part of each row to keep the unit uniform.
const ROW_HEIGHT: f32 = 49.0;
const TABLE_MIN_WIDTH: f32 = 720.0;
const CHANGED_WIDTH: f32 = 128.0;
const ACTIONS_WIDTH: f32 = 90.0;

#[derive(Debug, Clone)]
pub enum ItemListMessage {
    ItemSelected(usize),
    SortBy(SortColumn),
    OpenExternal(#[expect(dead_code)] usize),
    CopyUsername(bitwarden_vault::CipherId),
    CopyPassword(bitwarden_vault::CipherId),
    /// Emitted on every scroll of the virtualized list. The vault handler
    /// forwards the viewport straight into `ScrollState::track`.
    Scrolled(scrollable::Viewport),
}

fn initial_color(name: &str) -> iced::Color {
    let hash: u32 = name
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let hue = (hash % 360) as f32;
    let s: f32 = 0.5;
    let l: f32 = 0.45;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match hue as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color::from_rgb(r + m, g + m, b + m)
}

#[derive(Clone, Copy)]
struct ListContext<'a> {
    sort: VaultSort,
    active_user: crate::domain::UserId,
    colors: &'a AppColors,
    show_favicons: bool,
    favicon: &'a favicon::FaviconService,
}

pub fn view<'a>(
    ctx: &RenderCtx<'a>,
    items: &'a [Arc<CipherListView>],
    selected_index: Option<usize>,
    scroll: virtual_list::ScrollState,
    sort: VaultSort,
) -> Element<'a, ItemListMessage, AppTheme> {
    let ctx = ListContext {
        sort,
        active_user: ctx
            .active_user
            .copied()
            .expect("item_list requires active_user"),
        colors: ctx.colors,
        show_favicons: ctx.show_favicons,
        favicon: ctx.favicon,
    };
    responsive(move |size| {
        table_view(
            ctx,
            items,
            selected_index,
            scroll,
            size.width.max(TABLE_MIN_WIDTH),
        )
    })
    .into()
}

fn table_view<'a>(
    ctx: ListContext<'a>,
    items: &'a [Arc<CipherListView>],
    selected_index: Option<usize>,
    scroll: virtual_list::ScrollState,
    table_width: f32,
) -> Element<'a, ItemListMessage, AppTheme> {
    let header = |label: String, column: SortColumn| -> Element<'a, ItemListMessage, AppTheme> {
        let selected = ctx.sort.column == column;
        let color = if selected {
            ctx.colors.text_primary
        } else {
            ctx.colors.table_header
        };
        let arrow = if !selected {
            icons::ARROW_DOWN_UP
        } else if ctx.sort.direction == SortDirection::Ascending {
            icons::ARROW_UP
        } else {
            icons::ARROW_DOWN
        };
        buttons::ghost(
            row![
                text(label).size(14).color(color).font(crate::APP_FONT_BOLD),
                arrow.render(11.0, color)
            ]
            .spacing(4)
            .align_y(Alignment::Center),
            false,
            ctx.colors.item_hover,
            ctx.colors.item_hover,
            crate::theme::RADIUS_SM,
        )
        .on_press(ItemListMessage::SortBy(column))
        .padding([4, 0])
        .width(Fill)
        .into()
    };
    let table_header = container(
        row![
            container(header(fl!("vault-column-name"), SortColumn::Name))
                .width(Length::FillPortion(3)),
            container(header(
                fl!("vault-column-last-changed"),
                SortColumn::LastChanged
            ))
            .width(CHANGED_WIDTH),
            container(header(fl!("vault-column-base-url"), SortColumn::BaseUrl))
                .width(Length::FillPortion(2)),
            container(
                text(fl!("vault-column-options"))
                    .size(14)
                    .color(ctx.colors.table_header)
                    .font(crate::APP_FONT_BOLD)
            )
            .width(ACTIONS_WIDTH)
            .align_x(Alignment::End),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([4, 24])
    .width(Fill);

    let header_divider = container(components::separator_h()).padding([0, 16]);

    // Virtualized body: only items inside the visible window (+ an auto
    // overscan buffer) become widgets. Top/bottom `Space` fillers preserve
    // the scroll thumb so it looks and feels like a real N-item list.
    let list = virtual_list::view(
        items,
        scroll,
        ROW_HEIGHT,
        |i, item| row_element(ctx, i, item, selected_index == Some(i)),
        ItemListMessage::Scrolled,
    )
    .height(Fill)
    .id(LIST_ID)
    .style(components::rail_scroll_style);

    // Share a single horizontal viewport between header and body. The inner
    // vertical list stays virtualized when a detail pane narrows the table.
    container(
        scrollable(
            container(
                column![table_header, header_divider, list]
                    .width(Fill)
                    .height(Fill),
            )
            .width(table_width)
            .height(Fill),
        )
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .width(Fill)
        .height(Fill)
        .style(components::rail_scroll_style),
    )
    .padding(iced::Padding {
        bottom: 8.0,
        ..iced::Padding::ZERO
    })
    .height(Fill)
    .width(Fill)
    .into()
}

/// Build a single row Element at the given global index. `virtual_list`
/// wraps the returned Element in a `Length::Fixed(ROW_HEIGHT)` container
/// itself, so no height concern leaks into here.
fn row_element<'a>(
    ctx: ListContext<'a>,
    i: usize,
    item: &'a CipherListView,
    is_selected: bool,
) -> Element<'a, ItemListMessage, AppTheme> {
    let active_user = ctx.active_user;
    let subtitle = item.subtitle.as_str();
    let (has_username, has_uri) = match &item.r#type {
        CipherListViewType::Login(login) => (
            login.username.is_some(),
            login
                .uris
                .as_ref()
                .and_then(|u| u.first())
                .and_then(|u| u.uri.as_deref())
                .is_some(),
        ),
        _ => (false, false),
    };

    let icon: Element<'a, ItemListMessage, AppTheme> = if !ctx.show_favicons {
        initial_circle(&item.name, ctx.colors)
    } else {
        let handle = match &item.r#type {
            CipherListViewType::Login(login) => {
                let uri = login
                    .uris
                    .as_ref()
                    .and_then(|u| u.first())
                    .and_then(|u| u.uri.as_deref());
                ctx.favicon.handle_for_login_uri(&active_user, uri)
            }
            // Non-login ciphers fall back to the generic globe; per-type
            // BWI glyphs are tracked in docs/todo.md.
            _ => favicon::globe_handle(),
        };
        crate::components::favicon_icon(handle)
    };

    let info = column![
        text(&item.name)
            .size(14)
            .color(ctx.colors.text_primary)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End),
        text(subtitle)
            .size(14)
            .color(ctx.colors.text_secondary)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End),
    ]
    .spacing(2)
    .width(Fill);

    let mut actions: Vec<Element<'a, ItemListMessage, AppTheme>> = Vec::new();
    if has_uri {
        actions.push(action_icon(
            icons::BOX_ARROW_UP_RIGHT,
            ItemListMessage::OpenExternal(i),
            ctx.colors,
        ));
    }
    if has_username && let Some(id) = item.id {
        actions.push(
            tooltip(
                action_icon(icons::COPY, ItemListMessage::CopyUsername(id), ctx.colors),
                text(fl!("menu-edit-copy-username")).size(12),
                tooltip::Position::Top,
            )
            .into(),
        );
    }
    if matches!(item.r#type, CipherListViewType::Login(_))
        && let Some(id) = item.id
    {
        actions.push(
            tooltip(
                buttons::ghost_icon(
                    icons::BWI_KEY.render(14.0, ctx.colors.text_secondary),
                    ctx.colors.item_hover,
                )
                .padding([4, 6])
                .on_press(ItemListMessage::CopyPassword(id)),
                text(fl!("menu-edit-copy-password")).size(12),
                tooltip::Position::Top,
            )
            .into(),
        );
    }

    let actions_row = row(actions).spacing(2).align_y(Alignment::Center);

    let changed = item.revision_date.with_timezone(&chrono::Local);
    let changed_cell = column![
        text(changed.format("%Y-%m-%d").to_string())
            .size(12)
            .color(ctx.colors.text_primary),
        text(changed.format("%H:%M").to_string())
            .size(12)
            .color(ctx.colors.text_secondary),
    ]
    .spacing(2)
    .width(CHANGED_WIDTH);
    let base_url = match &item.r#type {
        CipherListViewType::Login(login) => login.uris.as_ref().and_then(|uris| {
            uris.iter()
                .filter_map(|uri| uri.uri.as_deref())
                .find_map(base_url_for_display)
        }),
        _ => None,
    };
    let url_cell = text(base_url.unwrap_or_else(|| "—".into()))
        .size(14)
        .color(ctx.colors.text_secondary)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End)
        .width(Length::FillPortion(2));
    let content = row![
        row![icon, info]
            .spacing(12)
            .align_y(Alignment::Center)
            .width(Length::FillPortion(3)),
        changed_cell,
        url_cell,
        container(actions_row)
            .width(ACTIONS_WIDTH)
            .align_x(Alignment::End),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    // The button itself is 48 tall (32 icon + 8+8 padding) and the
    // separator is 1 tall, summing to ROW_HEIGHT. `virtual_list` enforces
    // the uniform height on the outer wrapper — we only care about the
    // inner content and horizontal padding here.
    let button = buttons::ghost(
        content,
        is_selected,
        ctx.colors.item_hover,
        ctx.colors.item_hover,
        RADIUS_MD,
    )
    .on_press(ItemListMessage::ItemSelected(i))
    .padding([8, 8])
    .width(Fill);

    container(column![button, components::separator_h()].width(Fill))
        .padding([0, 16])
        .into()
}

fn action_icon<'a>(
    icon: icons::Icon,
    message: ItemListMessage,
    colors: &AppColors,
) -> Element<'a, ItemListMessage, AppTheme> {
    buttons::ghost_icon(icon.render(14.0, colors.text_secondary), colors.item_hover)
        .on_press(message)
        .padding([4, 6])
        .into()
}

/// Deterministic-colour initial-letter circle used when `show_favicons` is
/// off — preserves the legacy visual for users who prefer it.
fn initial_circle<'a>(name: &str, colors: &'a AppColors) -> Element<'a, ItemListMessage, AppTheme> {
    let initial = name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    let circle_color = initial_color(name);
    container(text(initial).size(14).color(colors.text_primary))
        .width(32)
        .height(32)
        .center_x(32)
        .center_y(32)
        .style(move |_theme: &AppTheme| {
            container::Style::default()
                .background(circle_color)
                .border(iced::border::rounded(16))
        })
        .into()
}
