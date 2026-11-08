use iced::{
    Alignment, Background, Border, Color, Element, Fill, Padding, Shadow,
    widget::{button, container, mouse_area, row, text},
};

use crate::{components::shell_scope::ShellScope, theme::AppTheme};

use super::{TITLE_BAR_HEIGHT, WINDOW_BTN_WIDTH};

/// Platform-specific window chrome icons.
pub mod icon {
    #[cfg(target_os = "windows")]
    pub const FONT: iced::Font = iced::Font {
        family: iced::font::Family::Name("Segoe MDL2 Assets"),
        weight: iced::font::Weight::Normal,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    };
    #[cfg(target_os = "windows")]
    pub const MINIMIZE: char = '\u{E921}';
    #[cfg(target_os = "windows")]
    pub const MAXIMIZE: char = '\u{E922}';
    #[cfg(target_os = "windows")]
    pub const CLOSE: char = '\u{E8BB}';
    #[cfg(target_os = "windows")]
    pub const RESTORE: char = '\u{E923}';

    #[cfg(not(target_os = "windows"))]
    pub const FONT: iced::Font = crate::components::icons::FONT;
    #[cfg(not(target_os = "windows"))]
    pub const MINIMIZE: char = crate::components::icons::DASH_LG.char();
    #[cfg(not(target_os = "windows"))]
    pub const MAXIMIZE: char = crate::components::icons::SQUARE.char();
    #[cfg(not(target_os = "windows"))]
    pub const CLOSE: char = crate::components::icons::X_LG.char();
    #[cfg(not(target_os = "windows"))]
    pub const RESTORE: char = crate::components::icons::WINDOW_STACK.char();
}

/// A flat window control button using a platform-specific icon font glyph.
pub fn chrome_button<'a, M: Clone + 'a>(
    glyph: char,
    font: iced::Font,
    hover_color: iced::Color,
    message: M,
) -> Element<'a, M, AppTheme> {
    let icon = container(text(glyph).font(font).size(crate::text_size(12)))
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    button(icon)
        .on_press(message)
        .width(WINDOW_BTN_WIDTH)
        .height(TITLE_BAR_HEIGHT)
        .padding(0)
        .style(move |_theme: &AppTheme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => hover_color,
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: Color::WHITE,
                border: Border::default(),
                shadow: Shadow::default(),
                snap: false,
            }
        })
        .into()
}

/// Wrap content with invisible resize handles overlaid on all edges and corners.
pub fn resize_wrapper<'a, M: Clone + 'a>(
    content: impl Into<Element<'a, M, AppTheme>>,
    map_direction: fn(iced::window::Direction) -> M,
) -> Element<'a, M, AppTheme> {
    use iced::Length::Fixed;
    use iced::mouse;
    use iced::window::Direction;

    const EDGE: f32 = 5.0;
    const CORNER: f32 = 8.0;

    let content = content.into();

    let handle =
        |w: f32, h: f32, dir: Direction, cursor: mouse::Interaction| -> Element<'a, M, AppTheme> {
            mouse_area(iced::widget::Space::new().width(w).height(h))
                .on_press(map_direction(dir))
                .interaction(cursor)
                .into()
        };

    let top_row: Element<'a, M, AppTheme> = row![
        handle(
            CORNER,
            EDGE,
            Direction::NorthWest,
            mouse::Interaction::ResizingDiagonallyDown
        ),
        mouse_area(iced::widget::Space::new().width(Fill).height(Fixed(EDGE)))
            .on_press(map_direction(Direction::North))
            .interaction(mouse::Interaction::ResizingVertically),
        handle(
            CORNER,
            EDGE,
            Direction::NorthEast,
            mouse::Interaction::ResizingDiagonallyUp
        ),
    ]
    .spacing(0)
    .into();

    let bottom_row: Element<'a, M, AppTheme> = container(
        row![
            handle(
                CORNER,
                EDGE,
                Direction::SouthWest,
                mouse::Interaction::ResizingDiagonallyUp
            ),
            mouse_area(iced::widget::Space::new().width(Fill).height(Fixed(EDGE)))
                .on_press(map_direction(Direction::South))
                .interaction(mouse::Interaction::ResizingVertically),
            handle(
                CORNER,
                EDGE,
                Direction::SouthEast,
                mouse::Interaction::ResizingDiagonallyDown
            ),
        ]
        .spacing(0),
    )
    .align_y(Alignment::End)
    .width(Fill)
    .height(Fill)
    .into();

    let left_edge: Element<'a, M, AppTheme> = container(
        mouse_area(iced::widget::Space::new().width(Fixed(EDGE)).height(Fill))
            .on_press(map_direction(Direction::West))
            .interaction(mouse::Interaction::ResizingHorizontally),
    )
    .width(Fill)
    .height(Fill)
    .padding(Padding {
        top: CORNER,
        right: 0.0,
        bottom: CORNER,
        left: 0.0,
    })
    .into();

    let right_edge: Element<'a, M, AppTheme> = container(
        mouse_area(iced::widget::Space::new().width(Fixed(EDGE)).height(Fill))
            .on_press(map_direction(Direction::East))
            .interaction(mouse::Interaction::ResizingHorizontally),
    )
    .width(Fill)
    .height(Fill)
    .align_x(Alignment::End)
    .padding(Padding {
        top: CORNER,
        right: 0.0,
        bottom: CORNER,
        left: 0.0,
    })
    .into();

    ShellScope::new(iced::widget::stack![
        content, top_row, bottom_row, left_edge, right_edge
    ])
    .into()
}
