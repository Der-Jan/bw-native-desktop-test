use iced::{
    Alignment, Background, Border, Color, Element, Fill, Shadow,
    widget::{button, column, container, mouse_area, row, text},
};

use crate::{
    components,
    services::{
        menu::{MenuEntry, MenuState},
        sdk::AccountEntry,
    },
    theme::{AppColors, AppTheme},
};

use super::{DROPDOWN_WIDTH, ITEM_PADDING, PANEL_RADIUS, SUBMENU_WIDTH, TitleBarMessage};

// Layout constants for the submenu-offset math. Tuned empirically against
// the rendered output — 32 placed submenus visibly above the parent row,
// 34 placed them visibly below; 33 lands roughly flush.
const ITEM_HEIGHT: f32 = 33.0;
const SEPARATOR_HEIGHT: f32 = 5.0;
const COLUMN_SPACING: f32 = 2.0;
const PANEL_TOP_PADDING: f32 = 6.0;

/// Build the overlay content for a single menu's dropdown.
/// When a submenu is open, returns a row with the main panel and submenu side-by-side.
///
/// `accounts` feeds [`MenuEntry::effective_children`] so dynamic submenus
/// (per-account Lock / Log out) expand from live state.
pub fn menu_panel<'a>(
    entries: &[MenuEntry],
    menu_index: usize,
    open_submenu: Option<usize>,
    accounts: &[AccountEntry],
    state: &MenuState,
    colors: &AppColors,
) -> Element<'a, TitleBarMessage, AppTheme> {
    let items = render_entries(entries, menu_index, open_submenu, state, colors);
    let main_panel = styled_panel(column(items).spacing(2).width(DROPDOWN_WIDTH));

    if let Some(sub_idx) = open_submenu
        && let Some(parent) = entries.get(sub_idx)
    {
        let children = parent.effective_children(accounts);
        if !children.is_empty() {
            // Place the submenu's panel top at the parent row's panel-top
            // y-coordinate. Three contributions: each preceding entry's
            // height, the column spacing between every pair (sub_idx of
            // them above the parent), and the panel's top padding.
            let v_offset: f32 = entries[..sub_idx]
                .iter()
                .map(|e| {
                    if e.is_separator() {
                        SEPARATOR_HEIGHT
                    } else {
                        ITEM_HEIGHT
                    }
                })
                .sum::<f32>()
                + sub_idx as f32 * COLUMN_SPACING
                + PANEL_TOP_PADDING;

            let sub_items: Vec<Element<'_, TitleBarMessage, AppTheme>> = children
                .iter()
                .map(|sub_entry| render_submenu_item(sub_entry, state, colors))
                .collect();

            let sub_panel = styled_panel(column(sub_items).spacing(2).width(SUBMENU_WIDTH));
            let sub_with_offset = column![iced::widget::Space::new().height(v_offset), sub_panel];

            return row![main_panel, sub_with_offset].spacing(4).into();
        }
    }

    main_panel
}

fn styled_panel<'a>(
    content: impl Into<Element<'a, TitleBarMessage, AppTheme>>,
) -> Element<'a, TitleBarMessage, AppTheme> {
    container(content)
        .padding([6, 0])
        .style(|theme: &AppTheme| {
            container::Style::default()
                .background(theme.colors.card_bg)
                .border(
                    Border::default()
                        .color(theme.colors.border)
                        .width(1.0)
                        .rounded(PANEL_RADIUS),
                )
        })
        .into()
}

fn render_entries<'a>(
    entries: &[MenuEntry],
    menu_index: usize,
    open_submenu: Option<usize>,
    state: &MenuState,
    colors: &AppColors,
) -> Vec<Element<'a, TitleBarMessage, AppTheme>> {
    entries
        .iter()
        .enumerate()
        .map(|(item_i, entry)| render_entry(entry, menu_index, item_i, open_submenu, state, colors))
        .collect()
}

fn render_entry<'a>(
    entry: &MenuEntry,
    menu_index: usize,
    item_index: usize,
    open_submenu: Option<usize>,
    state: &MenuState,
    colors: &AppColors,
) -> Element<'a, TitleBarMessage, AppTheme> {
    if entry.is_separator() {
        return container(components::separator_h()).padding([2, 8]).into();
    }

    let enabled = entry.is_enabled(state);
    let is_sub = entry.is_submenu();
    let sub_is_open = is_sub && open_submenu == Some(item_index);

    let label_color = if enabled {
        colors.text_primary
    } else {
        colors.text_muted
    };
    let shortcut_color = if enabled {
        colors.text_secondary
    } else {
        colors.text_muted
    };

    let label_text = text(entry.label.clone()).size(14).color(label_color);

    let content: Element<'_, TitleBarMessage, AppTheme> = if is_sub {
        row![
            label_text,
            iced::widget::Space::new().width(Fill),
            crate::components::icons::CHEVRON_RIGHT.render(11.0, shortcut_color),
        ]
        .spacing(16)
        .align_y(Alignment::Center)
        .into()
    } else if let Some(shortcut_text) = entry.shortcut_display() {
        row![
            label_text,
            iced::widget::Space::new().width(Fill),
            text(shortcut_text).size(12).color(shortcut_color),
        ]
        .spacing(16)
        .align_y(Alignment::Center)
        .into()
    } else {
        label_text.into()
    };

    let mut btn =
        button(content)
            .padding(ITEM_PADDING)
            .width(Fill)
            .style(move |theme: &AppTheme, status| {
                let bg = if sub_is_open {
                    theme.colors.item_hover
                } else if enabled {
                    match status {
                        button::Status::Hovered => theme.colors.item_hover,
                        _ => Color::TRANSPARENT,
                    }
                } else {
                    Color::TRANSPARENT
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: label_color,
                    border: Border::default(),
                    shadow: Shadow::default(),
                    snap: false,
                }
            });

    // Leaf clicks fire the resolved `MenuAction`; submenu parents have no
    // action and only open the submenu via hover.
    if enabled
        && !is_sub
        && let Some(action) = entry.action
    {
        btn = btn.on_press(TitleBarMessage::ItemAction(action));
    }

    if is_sub && enabled {
        mouse_area(btn)
            .on_enter(TitleBarMessage::SubMenuHovered(menu_index, item_index))
            .into()
    } else if enabled {
        mouse_area(btn)
            .on_enter(TitleBarMessage::SubMenuHovered(menu_index, usize::MAX))
            .into()
    } else {
        btn.into()
    }
}

fn render_submenu_item<'a>(
    entry: &MenuEntry,
    state: &MenuState,
    colors: &AppColors,
) -> Element<'a, TitleBarMessage, AppTheme> {
    if entry.is_separator() {
        return container(components::separator_h()).padding([2, 8]).into();
    }

    let enabled = entry.is_enabled(state);
    let label_color = if enabled {
        colors.text_primary
    } else {
        colors.text_muted
    };

    let label_text = text(entry.label.clone()).size(14).color(label_color);

    let content: Element<'_, TitleBarMessage, AppTheme> =
        if let Some(shortcut_text) = entry.shortcut_display() {
            row![
                label_text,
                iced::widget::Space::new().width(Fill),
                text(shortcut_text).size(12).color(if enabled {
                    colors.text_secondary
                } else {
                    colors.text_muted
                }),
            ]
            .spacing(16)
            .align_y(Alignment::Center)
            .into()
        } else {
            label_text.into()
        };

    let mut btn =
        button(content)
            .padding(ITEM_PADDING)
            .width(Fill)
            .style(move |theme: &AppTheme, status| {
                let bg = if enabled {
                    match status {
                        button::Status::Hovered => theme.colors.item_hover,
                        _ => Color::TRANSPARENT,
                    }
                } else {
                    Color::TRANSPARENT
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: label_color,
                    border: Border::default(),
                    shadow: Shadow::default(),
                    snap: false,
                }
            });

    if enabled && let Some(action) = entry.action {
        btn = btn.on_press(TitleBarMessage::ItemAction(action));
    }

    btn.into()
}
