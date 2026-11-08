use iced::{
    Alignment, Element, Fill,
    widget::{self, Space, checkbox, column, container, row, svg, text},
};

use crate::{
    components::{
        buttons, icons,
        inputs::{bare_text_input, field_frame},
    },
    fl,
    theme::{AppColors, AppTheme},
};

use super::{LoginMessage, layout};

pub const LOGIN_EMAIL_FIELD_ID: widget::Id = widget::Id::new("login-email-field");

pub fn view<'a>(
    email: &'a str,
    remember_email: bool,
    colors: &'a AppColors,
) -> Element<'a, LoginMessage, AppTheme> {
    let vault_icon = svg(svg::Handle::from_memory(crate::assets::VAULT_ICON))
        .width(64)
        .height(60);

    let title = text(fl!("login-email-title"))
        .size(crate::text_size(28))
        .color(colors.text_primary);

    let card = layout::auth_card(card_content(email, remember_email, colors));

    let create_account_link = row![
        text(format!("{} ", fl!("login-email-new-prompt")))
            .size(crate::text_size(14))
            .color(colors.text_secondary),
        text(fl!("login-email-create-account"))
            .size(crate::text_size(14))
            .color(colors.accent),
    ]
    .align_y(Alignment::Center);

    column![
        vault_icon,
        title,
        Space::new().height(12),
        card,
        Space::new().height(8),
        create_account_link
    ]
    .spacing(8)
    .align_x(Alignment::Center)
    .into()
}

fn card_content<'a>(
    email: &'a str,
    remember_email: bool,
    colors: &'a AppColors,
) -> Element<'a, LoginMessage, AppTheme> {
    let email_field = field_frame(
        fl!("login-email-placeholder"),
        bare_text_input(email)
            .id(LOGIN_EMAIL_FIELD_ID)
            .on_input(LoginMessage::EmailChanged)
            .on_submit(LoginMessage::ContinueWithEmail),
        colors,
    );

    let remember_checkbox = checkbox(remember_email)
        .label(fl!("login-email-remember"))
        .on_toggle(LoginMessage::ToggleRememberEmail)
        .size(crate::text_size(18))
        .spacing(8);

    let continue_button = buttons::primary(
        container(text(fl!("login-email-continue")).size(crate::text_size(16)))
            .center_x(Fill)
            .padding([4, 8]),
    )
    .on_press(LoginMessage::ContinueWithEmail)
    .width(Fill);

    let or_text = text(fl!("login-email-or"))
        .size(crate::text_size(14))
        .color(colors.text_primary)
        .center();

    let sso_button = buttons::secondary(
        container(
            row![
                icons::BWI_HANDSHAKE.render(16.0, colors.accent),
                text(fl!("login-email-sso")).size(crate::text_size(16)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .center_x(Fill)
        .padding([4, 8]),
    )
    .on_press(LoginMessage::UseSingleSignOn)
    .width(Fill);

    column![
        email_field,
        remember_checkbox,
        continue_button,
        container(or_text).center_x(Fill),
        sso_button,
    ]
    .spacing(12)
    .into()
}
