pub(crate) mod account_switcher;
pub(crate) mod bottom_sheet;
pub(crate) mod buttons;
pub(crate) mod collapsible_pane;
pub(crate) mod drop_down;
pub(crate) mod fade_in_out;
pub(crate) mod icons;
pub(crate) mod inputs;
pub(crate) mod list_pane;
pub(crate) mod modal;
pub(crate) mod picker_tile;
pub(crate) mod shell_scope;
pub(crate) mod sidebar;
pub(crate) mod spinner;
pub(crate) mod toast;
pub(crate) mod totp;
pub(crate) mod virtual_list;

pub(crate) use fade_in_out::FadeInOut;

use iced::{
    Background, Border, Color, Element, Fill, Padding, Shadow,
    widget::{container, image, rule, scrollable, text},
};

use crate::theme::{AppColors, AppTheme, RADIUS_LG};

/// Backdrop opacity used by the modal and bottom-sheet shells when fully
/// open. The actual paint multiplies this by the fade-in progress so the
/// scrim cross-fades with the foreground.
pub const BACKDROP_ALPHA: f32 = 0.45;

/// Right-pane shell shared by the cipher detail, cipher edit, and send edit
/// panes: full-fill container painted with `card_bg` and rounded only at the
/// top so it sits cleanly under the title bar without a visible top edge
/// while the bottom flushes against the window. Caller composes the
/// header + body + footer column inside.
pub fn rounded_top_pane<'a, M: 'a>(
    content: impl Into<Element<'a, M, AppTheme>>,
    top_radius: f32,
) -> Element<'a, M, AppTheme> {
    container(content)
        .width(Fill)
        .height(Fill)
        .style(move |theme: &AppTheme| {
            container::Style::default()
                .background(theme.colors.card_bg)
                .border(Border::default().rounded(iced::border::top(top_radius)))
        })
        .into()
}

/// Style fn for `scrollable`'s muted-rail look used by every list/scroller in
/// the main window: transparent rail, accent-on-hover thumb tinted from
/// `item_hover`, no overscroll affordance. Use as
/// `.style(components::rail_scroll_style)`.
pub fn rail_scroll_style(theme: &AppTheme, _status: scrollable::Status) -> scrollable::Style {
    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(theme.colors.scrollbar_thumb),
            border: Border::default().rounded(4),
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            shadow: Shadow::default(),
            icon: Color::TRANSPARENT,
        },
    }
}

pub fn separator_h<'a, M: 'a>() -> Element<'a, M, AppTheme> {
    rule::horizontal(1)
        .style(|theme: &AppTheme| rule::Style {
            color: theme.colors.border,
            radius: 0.0.into(),
            fill_mode: rule::FillMode::Full,
            snap: false,
        })
        .into()
}

pub fn separator_v<'a, M: 'a>() -> Element<'a, M, AppTheme> {
    rule::vertical(1)
        .style(|theme: &AppTheme| rule::Style {
            color: theme.colors.border,
            radius: 0.0.into(),
            fill_mode: rule::FillMode::Full,
            snap: false,
        })
        .into()
}

pub fn styled_card<'a, M: 'a>(
    content: impl Into<Element<'a, M, AppTheme>>,
) -> Element<'a, M, AppTheme> {
    container(content)
        .padding([12, 16])
        .width(Fill)
        .style(|theme: &AppTheme| {
            container::Style::default()
                .background(theme.colors.background)
                .border(
                    Border::default()
                        .color(theme.colors.border)
                        .width(1.0)
                        .rounded(RADIUS_LG),
                )
        })
        .into()
}

pub fn card_with_margin<'a, M: 'a>(
    card: impl Into<Element<'a, M, AppTheme>>,
) -> Element<'a, M, AppTheme> {
    container(card)
        .padding(Padding {
            top: 0.0,
            right: 0.0,
            bottom: 8.0,
            left: 0.0,
        })
        .into()
}

/// 14 px primary-text label that sits above a card or grouped field.
pub fn section_label<'a, M: 'a>(
    label: impl Into<String>,
    colors: &AppColors,
) -> Element<'a, M, AppTheme> {
    text(label.into())
        .size(crate::text_size(14))
        .color(colors.text_primary)
        .into()
}

/// 16 px bold primary-text heading that opens a settings tab section or a
/// generator subgroup.
pub fn section_heading<'a, M: 'a>(
    label: impl Into<String>,
    colors: &'a AppColors,
) -> Element<'a, M, AppTheme> {
    text(label.into())
        .size(crate::text_size(16))
        .font(crate::app_font_bold())
        .color(colors.text_primary)
        .into()
}

/// Right-pane header: 18 px title + close-X, separator below. Shared by
/// the cipher detail / edit panes. Send-edit's header has its own bolder /
/// card-bg style and stays inline.
pub fn pane_header<'a, M: Clone + 'a>(
    title: impl Into<String>,
    on_close: M,
    colors: &'a AppColors,
) -> Element<'a, M, AppTheme> {
    use iced::widget::{Space, row};
    let title_el = text(title.into())
        .size(crate::text_size(18))
        .color(colors.text_primary);
    let close_btn = self::buttons::ghost_icon(
        self::icons::BWI_CLOSE.render(32.0, colors.text_secondary),
        colors.item_hover,
    )
    .on_press(on_close)
    .padding([1, 1]);
    let header = container(
        row![title_el, Space::new().width(Fill), close_btn].align_y(iced::Alignment::Center),
    )
    .padding([8, 20]);
    iced::widget::column![header, separator_h()]
        .spacing(0)
        .into()
}

/// Right-pane footer: separator above + `background`-coloured action bar.
/// Pair with [`pane_header`].
pub fn pane_footer<'a, M: 'a>(
    content: impl Into<Element<'a, M, AppTheme>>,
) -> Element<'a, M, AppTheme> {
    let bar = container(content)
        .width(Fill)
        .padding([8, 20])
        .style(|theme: &AppTheme| container::Style::default().background(theme.colors.background));
    iced::widget::column![separator_h(), bar].spacing(0).into()
}

/// Bold 14 px heading stacked above a [`styled_card`] body. Used by
/// import / send-edit to group fields under a labeled card.
pub fn section_card<'a, M: 'a>(
    heading: impl Into<String>,
    body: impl Into<Element<'a, M, AppTheme>>,
    colors: &'a AppColors,
) -> Element<'a, M, AppTheme> {
    iced::widget::column![
        text(heading.into())
            .size(crate::text_size(14))
            .font(crate::app_font_bold())
            .color(colors.text_primary),
        styled_card(body),
    ]
    .spacing(8)
    .into()
}

/// 32×32 favicon slot for vault list / magnify rows. The image carries its
/// rounded-rect alpha mask baked in by [`crate::services::favicon`], so iced
/// just blits the texture — no container clipping needed.
pub fn favicon_icon<'a, M: 'a>(handle: image::Handle) -> Element<'a, M, AppTheme> {
    container(image::Image::new(handle).width(32).height(32))
        .width(32)
        .height(32)
        .into()
}

/// Standard 18 px / 8 sp labeled checkbox used across settings + generator
/// tabs and other tab-style toggle rows.
pub fn labeled_checkbox<'a, F, M>(
    checked: bool,
    label: impl Into<String>,
    on_toggle: F,
) -> iced::widget::Checkbox<'a, M, AppTheme>
where
    F: 'a + Fn(bool) -> M,
{
    iced::widget::checkbox(checked)
        .label(label.into())
        .size(crate::text_size(18))
        .spacing(8)
        .on_toggle(on_toggle)
}

pub mod ssh_approval;

pub mod focus_button;

pub mod keyed_rows;

pub mod smooth_scroll;
