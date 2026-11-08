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
pub(in crate::views::vault) const ROW_HEIGHT: f32 = 49.0;
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
                text(label)
                    .size(crate::text_size(14))
                    .color(color)
                    .font(crate::app_font_bold()),
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
                    .size(crate::text_size(14))
                    .color(ctx.colors.table_header)
                    .font(crate::app_font_bold())
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
        |i, item| {
            (
                ctx.active_user,
                item.id,
                if item.id.is_none() { i } else { 0 },
            )
        },
        |i, item| row_element(ctx, i, item, selected_index == Some(i)),
        ItemListMessage::Scrolled,
    )
    .height(Fill)
    .id(LIST_ID)
    .style(components::rail_scroll_style);

    let list = components::smooth_scroll::SmoothScroll::new(
        list,
        LIST_ID,
        (
            items.as_ptr() as usize,
            items.len(),
            ctx.sort.column as u8,
            ctx.sort.direction as u8,
        ),
    );

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
            .size(crate::text_size(14))
            .color(ctx.colors.text_primary)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End),
        text(subtitle)
            .size(crate::text_size(14))
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
                text(fl!("menu-edit-copy-username")).size(crate::text_size(12)),
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
                text(fl!("menu-edit-copy-password")).size(crate::text_size(12)),
                tooltip::Position::Top,
            )
            .into(),
        );
    }

    let actions_row = row(actions).spacing(2).align_y(Alignment::Center);

    let changed = item.revision_date.with_timezone(&chrono::Local);
    let changed_cell = column![
        text(changed.format("%Y-%m-%d").to_string())
            .size(crate::text_size(12))
            .color(ctx.colors.text_primary),
        text(changed.format("%H:%M").to_string())
            .size(crate::text_size(12))
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
        .size(crate::text_size(14))
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
    container(
        text(initial)
            .size(crate::text_size(14))
            .color(colors.text_primary),
    )
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

#[cfg(test)]
mod scroll_profile {
    use super::*;

    #[tokio::test]
    async fn pixel_scroll_keeps_position_and_extent_when_virtual_rows_are_replaced() {
        use crate::test_support::fixtures::{CipherSpec, make_cipher};
        use iced::advanced::{layout, widget::Tree};
        let app = crate::app::App::test();
        let items: Vec<_> = (0..100)
            .map(|_| make_cipher(CipherSpec::default()))
            .collect();
        let ctx = ListContext {
            sort: VaultSort::default(),
            active_user: crate::domain::UserId::new_v4(),
            colors: &app.theme.current.colors,
            show_favicons: false,
            favicon: &app.favicon,
        };
        let renderer = <iced::Renderer as iced::advanced::renderer::Headless>::new(
            iced::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        )
        .await
        .unwrap();
        let mut tree = Tree::empty();
        let mut scroll = virtual_list::ScrollState {
            offset_y: 0.0,
            viewport_height: 600.0,
        };
        let limits = layout::Limits::new(iced::Size::ZERO, iced::Size::new(900.0, 650.0));
        for pixel in 1..=1200 {
            let mut element = table_view(ctx, &items, None, scroll, 900.0);
            tree.diff(element.as_widget());
            let node = element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &limits);
            let mut messages = Vec::new();
            element.as_widget_mut().update(
                &mut tree,
                &iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
                    delta: iced::mouse::ScrollDelta::Pixels { x: 0.0, y: -1.0 },
                }),
                iced::advanced::Layout::new(&node),
                iced::mouse::Cursor::Available(iced::Point::new(400.0, 300.0)),
                &renderer,
                &mut iced::advanced::Shell::new(&mut messages),
                &iced::Rectangle::with_size(iced::Size::new(900.0, 650.0)),
            );
            let viewport = messages
                .into_iter()
                .find_map(|m| match m {
                    ItemListMessage::Scrolled(v) => Some(v),
                    _ => None,
                })
                .expect("pixel input must publish viewport");
            assert_eq!(
                viewport.absolute_offset().y,
                pixel as f32,
                "scroll reversed or jumped"
            );
            assert_eq!(
                viewport.content_bounds().height,
                100.0 * ROW_HEIGHT,
                "virtual extent changed"
            );
            scroll.track(viewport);
        }
        // Exercise the real wheel wrapper, rebuilding the virtualized table
        // on every intermediate frame, rather than just testing Motion alone.
        let started = iced::time::Instant::now();
        let mut intermediate = 0;
        for frame in 0..=20 {
            let mut element = table_view(ctx, &items, None, scroll, 900.0);
            tree.diff(element.as_widget());
            let node = element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &limits);
            let event = if frame == 0 {
                iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
                    delta: iced::mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
                })
            } else {
                iced::Event::Window(iced::window::Event::RedrawRequested(
                    started + std::time::Duration::from_millis(frame * 5),
                ))
            };
            let mut messages = Vec::new();
            element.as_widget_mut().update(
                &mut tree,
                &event,
                iced::advanced::Layout::new(&node),
                iced::mouse::Cursor::Available(iced::Point::new(400.0, 300.0)),
                &renderer,
                &mut iced::advanced::Shell::new(&mut messages),
                &iced::Rectangle::with_size(iced::Size::new(900.0, 650.0)),
            );
            for message in messages {
                if let ItemListMessage::Scrolled(viewport) = message {
                    let y = viewport.absolute_offset().y;
                    assert!(y >= scroll.offset_y, "single wheel notch moved backwards");
                    assert!(y <= 1260.01, "single wheel notch overshot");
                    assert_eq!(viewport.content_bounds().height, 100.0 * ROW_HEIGHT);
                    if y > 1201.0 && y < 1260.0 {
                        intermediate += 1;
                    }
                    scroll.track(viewport);
                }
            }
        }
        assert!((scroll.offset_y - 1260.0).abs() < 0.01);
        assert!(
            intermediate >= 5,
            "wheel step lacked intermediate pixel positions"
        );
    }

    #[tokio::test]
    #[ignore = "manual layout/raster benchmark; excludes compositor and input latency"]
    async fn profile_scrolling_layout() {
        use crate::test_support::fixtures::{CipherSpec, login_kind, make_cipher};
        use iced::advanced::{layout, widget::Tree};
        let mut app = crate::app::App::test();
        let mut items = Vec::new();
        for i in 0..10_000 {
            let mut item = make_cipher(CipherSpec {
                kind: login_kind(Some("https://example.invalid/path")),
                ..Default::default()
            });
            let view = Arc::get_mut(&mut item).unwrap();
            view.name = format!("Synthetic login {i:05}");
            view.subtitle = format!("synthetic-user-{i}");
            view.id = Some(bitwarden_vault::CipherId::new_v4());
            items.push(item);
        }
        let full_window = std::env::var_os("BW_SCROLL_PROFILE_FULL_WINDOW").is_some();
        let uid = crate::domain::UserId::new_v4();
        if full_window {
            app.client_manager = crate::services::sdk::test_support::unlocked_manager(uid);
            app.active_user = Some(uid);
            app.screen = crate::domain::Screen::Vault;
            app.refresh_accounts_cache();
            app.views.vault.items.insert(
                uid,
                crate::views::vault::state::ItemCache {
                    all: items.clone(),
                    cached: items.clone(),
                    ..Default::default()
                },
            );
        }
        {
            let mut fonts = iced::advanced::graphics::text::font_system()
                .write()
                .unwrap();
            for bytes in [
                crate::assets::FONT_MEDIUM,
                crate::assets::FONT_BOLD,
                crate::assets::BWI_FONT,
                crate::components::icons::FONT_BYTES,
            ] {
                fonts.load_font(std::borrow::Cow::Borrowed(bytes));
            }
        }
        let size = if full_window {
            crate::app::MAIN_WINDOW_SIZE
        } else {
            iced::Size::new(900.0, 650.0)
        };
        let mut renderer = <iced::Renderer as iced::advanced::renderer::Headless>::new(
            iced::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        )
        .await
        .expect("CPU benchmark renderer");
        let mut tree = Tree::empty();
        let limits = layout::Limits::new(iced::Size::ZERO, size);
        let mut samples = Vec::new();
        let draw_frames = std::env::var_os("BW_SCROLL_PROFILE_DRAW").is_some();
        let mut raster = Vec::new();
        for frame in 0..330 {
            let started = std::time::Instant::now();
            let scroll = virtual_list::ScrollState {
                offset_y: frame as f32 * 7.0,
                viewport_height: size.height - 130.0,
            };
            app.views.vault.list_scroll = scroll;
            let mut element = if full_window {
                app.view(app.main_window)
            } else {
                table_view(
                    ListContext {
                        sort: VaultSort::default(),
                        active_user: uid,
                        colors: &app.theme.current.colors,
                        show_favicons: false,
                        favicon: &app.favicon,
                    },
                    &items,
                    None,
                    scroll,
                    size.width,
                )
                .map(|m| {
                    crate::app::Message::View(crate::app::ViewMessage::Vault(
                        crate::views::vault::VaultMessage::ItemList(m),
                    ))
                })
            };
            tree.diff(element.as_widget());
            let node = element
                .as_widget_mut()
                .layout(&mut tree, &renderer, &limits);
            if frame >= 30 {
                samples.push(started.elapsed().as_secs_f64() * 1000.0);
            }
            if draw_frames {
                use iced::advanced::renderer::{Headless as _, Renderer as _};
                let mut scroll = iced::advanced::widget::operation::scrollable::scroll_to::<()>(
                    LIST_ID,
                    scrollable::AbsoluteOffset {
                        x: None,
                        y: Some(frame as f32 * 7.0),
                    },
                );
                element.as_widget_mut().operate(
                    &mut tree,
                    iced::advanced::Layout::new(&node),
                    &renderer,
                    &mut scroll,
                );
                let bounds = iced::Rectangle::with_size(size);
                let started = std::time::Instant::now();
                renderer.reset(bounds);
                element.as_widget().draw(
                    &tree,
                    &mut renderer,
                    &app.theme.current,
                    &iced::advanced::renderer::Style::default(),
                    iced::advanced::Layout::new(&node),
                    iced::advanced::mouse::Cursor::Unavailable,
                    &bounds,
                );
                let _rgba = renderer.screenshot(
                    iced::Size::new((size.width * 1.5) as u32, (size.height * 1.5) as u32),
                    1.5,
                    iced::Color::WHITE,
                );
                if frame >= 30 {
                    raster.push(started.elapsed().as_secs_f64() * 1000.0);
                }
            }
        }
        if !raster.is_empty() {
            raster.sort_by(f64::total_cmp);
            println!(
                "raster (full_window={full_window}) at 1.5x: median={:.3}ms p95={:.3}ms",
                raster[150], raster[285]
            );
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "scroll construction+diff+layout: median={:.3}ms p95={:.3}ms max={:.3}ms",
            samples[150], samples[285], samples[299]
        );
    }
}
