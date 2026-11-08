use bitwarden_vault::{CipherListView, CipherListViewType};
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Length,
    widget::{
        column, container, mouse_area, row, text,
        text::{Ellipsis, Wrapping},
    },
};

use crate::{
    components::icons,
    services::favicon::FaviconService,
    theme::{AppColors, AppTheme, MAGNIFY_OVERLAY_ALPHA_MEDIUM, RADIUS_PILL, RADIUS_SM},
    views::magnify::{MagnifyMessage, dims::ROW_HEIGHT, widgets::chip::keybind},
};

/// Horizontal inset between the selection-highlighted area and the window
/// edge / scrollbar, so the bright-blue selected row never visually touches
/// the surrounding chrome.
const SELECTION_INSET_X: f32 = 8.0;
/// Vertical inset between consecutive rows.
const SELECTION_INSET_Y: f32 = 2.0;

/// Render a single result row. The full-width `mouse_area` is the click
/// target; an inset inner container holds the colored-on-selection
/// background, so the highlight reads as a pill that doesn't bleed to the
/// scrollbar or the rounded window edge.
pub fn view<'a>(
    index: usize,
    item: &'a CipherListView,
    is_selected: bool,
    favicon: &'a FaviconService,
    show_favicons: bool,
    active_user: Option<&'a crate::domain::UserId>,
    colors: &'a AppColors,
) -> Element<'a, MagnifyMessage, AppTheme> {
    let icon_el = icon_for(item, favicon, show_favicons, active_user, colors);

    // Selected row uses theme-aware text colors so the bright-blue selection
    // pill stays AA-readable on the light theme. Dark theme's `text_primary`
    // is `Color::WHITE` so this is a no-op there; on light theme this swaps
    // to the dark foreground (~13:1 on `#53A3FA` vs. ~2.9:1 for white).
    let (text_primary, text_secondary) = if is_selected {
        (colors.text_primary, colors.text_primary)
    } else {
        (colors.text_primary, colors.text_secondary)
    };

    // Both lines clip with an end ellipsis. Without `Wrapping::None`, an
    // SSH-key fingerprint or other long subtitle wraps over and visually
    // collides with the action pills on the selected row.
    let info = column![
        text(item.name.as_str())
            .size(crate::text_size(14))
            .color(text_primary)
            .font(crate::app_font_bold())
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End),
        text(item.subtitle.as_str())
            .size(crate::text_size(14))
            .color(text_secondary)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End),
    ]
    .spacing(2)
    .width(Fill);

    let mut content = row![icon_el, info].spacing(12).align_y(Alignment::Center);

    // Action pills are display-only — visual indication of the keyboard
    // shortcuts. The user triggers them with Ctrl+C / Ctrl+Shift+C.
    if is_selected {
        content = content
            .push(action_pill(
                crate::fl!("magnify-copy-password"),
                "Ctrl+C",
                colors,
            ))
            .push(action_pill(
                crate::fl!("magnify-copy-username"),
                "Ctrl+\u{21E7}C",
                colors,
            ));
    }

    let inner = container(content)
        .padding([0, 12])
        .width(Fill)
        .height(Fill)
        .align_y(Alignment::Center)
        .style(move |theme: &AppTheme| {
            let bg = if is_selected {
                Background::Color(theme.colors.magnify_selected)
            } else {
                Background::Color(Color::TRANSPARENT)
            };
            container::Style::default()
                .background(bg)
                .border(Border::default().rounded(RADIUS_SM))
        });

    let outer = container(inner)
        .padding([SELECTION_INSET_Y, SELECTION_INSET_X])
        .width(Fill)
        .height(Length::Fixed(ROW_HEIGHT));

    mouse_area(outer)
        .on_press(MagnifyMessage::RowClicked(index))
        .into()
}

/// Pill shown inside the selected row indicating one of the copy
/// shortcuts. Visual only — the actual binding fires from the global
/// key dispatch in `app::handlers::magnify`. Layout: `[label  [shortcut]]`,
/// where the shortcut sits in an embedded keybind chip rendered in the
/// same white as the label so the whole pill reads uniformly against the
/// blue selected row.
fn action_pill<'a>(
    label: String,
    shortcut: &'static str,
    colors: &'a AppColors,
) -> Element<'a, MagnifyMessage, AppTheme> {
    container(
        row![
            text(label)
                .size(crate::text_size(12))
                .color(colors.text_primary),
            keybind(
                text(shortcut)
                    .size(crate::text_size(12))
                    .color(colors.text_primary)
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(|_theme: &AppTheme| {
        container::Style::default()
            .background(Background::Color(Color {
                a: MAGNIFY_OVERLAY_ALPHA_MEDIUM,
                ..Color::WHITE
            }))
            .border(Border::default().rounded(RADIUS_PILL))
    })
    .into()
}

fn icon_for<'a>(
    item: &'a CipherListView,
    favicon: &'a FaviconService,
    show_favicons: bool,
    active_user: Option<&'a crate::domain::UserId>,
    colors: &'a AppColors,
) -> Element<'a, MagnifyMessage, AppTheme> {
    let inner: Element<'a, MagnifyMessage, AppTheme> = if let CipherListViewType::Login(login) =
        &item.r#type
        && show_favicons
        && let Some(uid) = active_user
    {
        let uri = login
            .uris
            .as_ref()
            .and_then(|u| u.first())
            .and_then(|u| u.uri.as_deref());
        crate::components::favicon_icon(favicon.handle_for_login_uri(uid, uri))
    } else {
        let icon = match item.r#type {
            CipherListViewType::Card(_) => icons::BWI_CREDIT_CARD,
            CipherListViewType::Identity => icons::BWI_IDENTITY,
            CipherListViewType::SecureNote => icons::BWI_NOTE,
            CipherListViewType::SshKey => icons::BWI_KEY,
            CipherListViewType::Login(_) => icons::BWI_LOGIN,
            CipherListViewType::BankAccount(_) => icons::BWI_BANK,
            CipherListViewType::DriversLicense => icons::BWI_DRIVERS_LICENSE,
            CipherListViewType::Passport => icons::BWI_PASSPORT,
        };
        container(icon.render(20.0, colors.text_primary))
            .width(32)
            .height(32)
            .center_x(32)
            .center_y(32)
            .into()
    };

    icon_backdrop(inner)
}

/// Wrap the icon in a slightly larger semi-transparent white square so it
/// stays legible against either the launcher's translucent dark surface or
/// the bright-blue selected row. Matches `designs/magnify/searching.jpg`.
fn icon_backdrop<'a>(
    inner: impl Into<Element<'a, MagnifyMessage, AppTheme>>,
) -> Element<'a, MagnifyMessage, AppTheme> {
    container(inner)
        .padding(2)
        .style(|_theme: &AppTheme| {
            container::Style::default()
                .background(Background::Color(Color {
                    a: MAGNIFY_OVERLAY_ALPHA_MEDIUM,
                    ..Color::WHITE
                }))
                .border(Border::default().rounded(RADIUS_SM))
        })
        .into()
}
