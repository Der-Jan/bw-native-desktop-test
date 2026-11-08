use iced::{
    Element, Fill,
    widget::{self, column, container, svg, text},
};

use crate::{
    components::{buttons, inputs::reveal_text_field_with_submit},
    fl,
    theme::{AppColors, AppTheme},
};

use super::{LoginMessage, layout};

pub const LOGIN_PASSWORD_FIELD_ID: widget::Id = widget::Id::new("login-password-field");

pub fn view<'a>(
    email: &'a str,
    password: &'a str,
    colors: &'a AppColors,
) -> Element<'a, LoginMessage, AppTheme> {
    let wave_icon = svg(svg::Handle::from_memory(crate::assets::WAVE_ICON))
        .width(64)
        .height(60);

    let card = layout::auth_card(card_content(password, colors));

    layout::icon_title_card(
        wave_icon.into(),
        fl!("login-password-title"),
        email,
        card,
        colors,
    )
}

fn card_content<'a>(
    password: &'a str,
    colors: &'a AppColors,
) -> Element<'a, LoginMessage, AppTheme> {
    let password_field = reveal_text_field_with_submit(
        Some(LOGIN_PASSWORD_FIELD_ID),
        fl!("login-password-placeholder"),
        password,
        LoginMessage::LoginPasswordChanged,
        LoginMessage::LoginWithPassword,
        false,
        colors,
    );

    let hint_link = buttons::transparent(
        text(fl!("login-password-get-hint"))
            .size(crate::text_size(14))
            .color(colors.accent),
    )
    .on_press(LoginMessage::GetPasswordHint)
    .padding(0);

    let login_button = buttons::primary(
        container(text(fl!("login-password-submit")).size(crate::text_size(16)))
            .center_x(Fill)
            .padding([4, 8]),
    )
    .on_press(LoginMessage::LoginWithPassword)
    .width(Fill);

    let back_button = buttons::secondary(
        container(text(fl!("login-password-back")).size(crate::text_size(16)))
            .center_x(Fill)
            .padding([4, 8]),
    )
    .on_press(LoginMessage::BackToEmail)
    .width(Fill);

    column![password_field, hint_link, login_button, back_button]
        .spacing(12)
        .into()
}
