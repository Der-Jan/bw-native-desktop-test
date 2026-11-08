use iced::{
    Alignment, Background, Border, Color, Element, Fill, Padding, Shadow,
    widget::{button, container, row, text},
};

use crate::{
    components::icons,
    fl,
    theme::{AppColors, AppTheme},
};

use super::{LoginMessage, ServerOption};

/// Non-interactive status bar: "Accessing {server}" (used on unlock screens).
pub fn simple_status<'a>(server: &str, colors: &AppColors) -> Element<'a, LoginMessage, AppTheme> {
    container(
        text(fl!("login-server-accessing", server = server))
            .size(crate::text_size(14))
            .color(colors.text_secondary),
    )
    .center_x(Fill)
    .padding(Padding {
        top: 12.0,
        right: 0.0,
        bottom: 20.0,
        left: 0.0,
    })
    .into()
}

/// Interactive server selector: "Accessing: **bitwarden.com** ▾" with dropdown.
pub fn view<'a>(
    selected: &'a ServerOption,
    is_open: bool,
    colors: &AppColors,
) -> Element<'a, LoginMessage, AppTheme> {
    let server_name = selected.display_name();
    let accent = colors.accent;

    let trigger = button(
        row![
            text(server_name).size(crate::text_size(14)).color(accent),
            icons::BWI_ANGLE_DOWN.render(12.0, accent),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .on_press(LoginMessage::ToggleServerSelector)
    .padding([2, 4])
    .style(|_theme: &AppTheme, _status| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: Color::TRANSPARENT,
        border: Border::default(),
        shadow: Shadow::default(),
        snap: false,
    });

    let panel = server_panel(selected, colors);
    let dropdown: Element<'a, LoginMessage, AppTheme> =
        crate::components::drop_down::DropDown::new(trigger, panel, is_open)
            .on_dismiss(LoginMessage::ToggleServerSelector)
            .alignment(crate::components::drop_down::Alignment::AboveRight)
            .width(iced::Length::Shrink)
            .offset(4.0)
            .into();

    container(
        row![
            text(format!("{} ", fl!("login-server-accessing-label")))
                .size(crate::text_size(14))
                .color(colors.text_secondary),
            dropdown,
        ]
        .align_y(Alignment::Center),
    )
    .center_x(Fill)
    .padding(Padding {
        top: 12.0,
        right: 0.0,
        bottom: 20.0,
        left: 0.0,
    })
    .into()
}

fn server_panel<'a>(
    current: &ServerOption,
    colors: &AppColors,
) -> Element<'a, LoginMessage, AppTheme> {
    let options: Vec<(String, ServerOption)> = vec![
        ("bitwarden.com".into(), ServerOption::Bitwarden),
        ("bitwarden.eu".into(), ServerOption::BitwardenEu),
        (
            fl!("login-server-self-hosted"),
            ServerOption::SelfHosted(String::new()),
        ),
    ];

    let current_name = current.display_name();
    let accent = colors.accent;
    let text_primary = colors.text_primary;

    let items: Vec<Element<'_, LoginMessage, AppTheme>> = options
        .into_iter()
        .map(|(name, opt)| {
            let is_selected = name == current_name;
            let label_color = if is_selected { accent } else { text_primary };

            button(text(name).size(crate::text_size(14)).color(label_color))
                .on_press(LoginMessage::SelectServer(opt))
                .padding([8, 16])
                .width(Fill)
                .style(move |theme: &AppTheme, status| {
                    let bg = match status {
                        button::Status::Hovered => theme.colors.item_hover,
                        _ => Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(Background::Color(bg)),
                        text_color: label_color,
                        border: Border::default(),
                        shadow: Shadow::default(),
                        snap: false,
                    }
                })
                .into()
        })
        .collect();

    container(iced::widget::column(items).spacing(2).width(200))
        .padding([6, 0])
        .style(|theme: &AppTheme| {
            container::Style::default()
                .background(theme.colors.card_bg)
                .border(
                    Border::default()
                        .color(theme.colors.border)
                        .width(1.0)
                        .rounded(8),
                )
        })
        .into()
}
