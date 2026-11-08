//! Two-column tile used by the New-item picker modal. An icon chip on the
//! left (square, blue-tinted), a bold title and secondary subtitle in the
//! middle, and a trailing chevron. The tile sits as a card on the modal's
//! `card_bg` surface — its own background uses the brighter `background`
//! token so each row reads as a discrete clickable card; hover lifts it to
//! `item_hover`.

use iced::{
    Alignment, Background, Border, Element, Length, Shadow,
    widget::{Space, button, column, container, row, text},
};

use crate::{
    components::icons,
    theme::{AppColors, AppTheme, RADIUS_LG, RADIUS_MD},
};

/// Side length of the square icon chip, in px.
const CHIP_SIZE: f32 = 40.0;
/// Icon-glyph point size inside the chip.
const CHIP_ICON_SIZE: f32 = 18.0;
/// Accent-tint alpha for the chip background. The chip sits on the tile's
/// `background` surface so a low alpha reads as a darker blue panel rather
/// than a coloured wash.
const CHIP_BG_ALPHA: f32 = 0.18;

/// Picker-tile row. Caller supplies the icon, the bold title, the subtitle,
/// and the press message. The chevron is drawn from `BWI_ANGLE_RIGHT`.
pub fn view<'a, M: 'a + Clone>(
    icon: icons::BwiIcon,
    title: impl Into<String>,
    subtitle: impl Into<String>,
    on_press: M,
    colors: &'a AppColors,
) -> Element<'a, M, AppTheme> {
    let chip_bg = iced::Color {
        a: CHIP_BG_ALPHA,
        ..colors.accent
    };
    let chip = container(icon.render(CHIP_ICON_SIZE, colors.accent))
        .width(Length::Fixed(CHIP_SIZE))
        .height(Length::Fixed(CHIP_SIZE))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_theme: &AppTheme| container::Style {
            background: Some(Background::Color(chip_bg)),
            border: Border::default().rounded(RADIUS_MD),
            ..Default::default()
        });

    let title_el = text(title.into())
        .size(crate::text_size(14))
        .font(crate::app_font_bold())
        .color(colors.text_primary);
    let subtitle_el = text(subtitle.into())
        .size(crate::text_size(12))
        .color(colors.text_secondary);

    let body = row![
        chip,
        column![title_el, subtitle_el].spacing(2),
        Space::new().width(Length::Fill),
        icons::BWI_ANGLE_RIGHT.render(14.0, colors.text_secondary),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    button(body)
        .on_press(on_press)
        .padding([10, 14])
        .width(Length::Fill)
        .style(|theme: &AppTheme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => theme.colors.item_hover,
                _ => theme.colors.background,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: theme.colors.text_primary,
                border: Border::default().rounded(RADIUS_LG),
                shadow: Shadow::default(),
                snap: false,
            }
        })
        .into()
}
