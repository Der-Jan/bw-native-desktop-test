use iced::{
    Alignment, Background, Border, Color, Element, Fill, Length, Shadow,
    theme::Base,
    widget::{Space, button, column, container, row, stack, svg, text},
};

use crate::{
    components::{
        account_switcher::{self, AccountSwitcherMessage},
        icons,
        shell_scope::ShellScope,
    },
    services::sdk::AccountEntry,
    theme::{AppColors, AppTheme},
};

use super::LoginMessage;

/// Shared outer shell for all auth screens.
pub fn auth_page_shell<'a>(
    center_content: Element<'a, LoginMessage, AppTheme>,
    status_bar: Element<'a, LoginMessage, AppTheme>,
    active_email: Option<&'a str>,
    accounts: &'a [AccountEntry],
    dropdown_open: bool,
    colors: &'a AppColors,
) -> Element<'a, LoginMessage, AppTheme> {
    let logo = svg(svg::Handle::from_memory(crate::assets::LOGO_WHITE))
        .width(209)
        .height(35)
        .style(|theme: &AppTheme, _status| svg::Style {
            color: if theme.mode() == iced::theme::Mode::Light {
                Some(theme.colors.button_primary)
            } else {
                None
            },
        });

    let logo_row = container(logo).width(Fill).padding([16, 24]);

    let bg_left = svg(svg::Handle::from_memory(crate::assets::BG_LEFT))
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(180.0))
        .opacity(0.11);

    let bg_right = svg(svg::Handle::from_memory(crate::assets::BG_RIGHT))
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(240.0))
        .opacity(0.11);

    let bg_illustrations = column![
        Space::new().height(Fill),
        row![bg_left, Space::new().width(Fill), bg_right].align_y(Alignment::End),
    ]
    .width(Fill)
    .height(Fill);

    let menu_dot_trigger = button(
        container(icons::THREE_DOTS.render(16.0, Color::WHITE))
            .width(36)
            .height(36)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    )
    .on_press(LoginMessage::AccountSwitcher(
        AccountSwitcherMessage::ToggleDropdown,
    ))
    .padding(0)
    .style(|theme: &AppTheme, status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => theme.colors.button_primary_hover,
            _ => theme.colors.button_primary,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: Color::WHITE,
            border: Border::default().rounded(18),
            shadow: Shadow::default(),
            snap: false,
        }
    });

    let dd_panel = account_switcher::dropdown(active_email, accounts, colors)
        .map(LoginMessage::AccountSwitcher);
    let menu_dot_dropdown: Element<'a, LoginMessage, AppTheme> =
        crate::components::drop_down::DropDown::new(menu_dot_trigger, dd_panel, dropdown_open)
            .on_dismiss(LoginMessage::AccountSwitcher(
                AccountSwitcherMessage::ToggleDropdown,
            ))
            .alignment(crate::components::drop_down::Alignment::BelowRight)
            .width(360.0)
            .offset(4.0)
            .into();

    let top_row = row![
        logo_row,
        Space::new().width(Fill),
        container(menu_dot_dropdown).padding([16, 24]),
    ]
    .align_y(Alignment::Center);

    let foreground = column![
        top_row,
        Space::new().height(Length::Fixed(40.0)),
        container(center_content).center_x(Fill),
        Space::new().height(Fill),
        status_bar,
    ]
    .width(Fill)
    .height(Fill);

    // Wrap in ShellScope so a future interactive element in
    // `bg_illustrations` can't leak `is_event_captured` into the foreground
    // and break the account-switcher dropdown / server selector inside it.
    let layered = ShellScope::new(stack![bg_illustrations, foreground]);

    container(layered)
        .width(Fill)
        .height(Fill)
        .style(|theme: &AppTheme| container::Style::default().background(theme.colors.card_bg))
        .into()
}

/// Centered "icon → 28pt title → secondary-text email → card" column shared by
/// the Unlock and Login-with-Password screens. Login-with-Email diverges (no
/// email row, plus a trailing "create account" link) so it inlines its own
/// column rather than calling this helper.
pub fn icon_title_card<'a>(
    icon: Element<'a, LoginMessage, AppTheme>,
    title: impl Into<String>,
    email: &'a str,
    card: Element<'a, LoginMessage, AppTheme>,
    colors: &'a AppColors,
) -> Element<'a, LoginMessage, AppTheme> {
    let title_el = text(title.into())
        .size(crate::text_size(28))
        .color(colors.text_primary);
    let email_el = text(email)
        .size(crate::text_size(16))
        .color(colors.text_secondary);
    column![icon, title_el, email_el, Space::new().height(12), card]
        .spacing(8)
        .align_x(Alignment::Center)
        .into()
}

/// The standard card container used on all auth screens.
pub fn auth_card<'a>(
    content: impl Into<Element<'a, LoginMessage, AppTheme>>,
) -> Element<'a, LoginMessage, AppTheme> {
    container(content)
        .max_width(450)
        .padding(32)
        .style(|theme: &AppTheme| {
            container::Style::default()
                .background(theme.colors.background)
                .border(
                    Border::default()
                        .color(theme.colors.border)
                        .width(1.0)
                        .rounded(16),
                )
        })
        .into()
}
