use iced::{
    Alignment, Element, Fill,
    widget::{self, column, container, svg, text},
};

use crate::{
    components::{buttons, inputs::reveal_text_field_with_submit, spinner},
    domain::UnlockMethod,
    fl,
    theme::{AppColors, AppTheme},
};

use super::{LoginMessage, layout};

/// Master-password and PIN inputs share the id — they're never rendered at
/// the same time, and `LoginView::auto_focus_task` only needs to find one.
pub const UNLOCK_FIELD_ID: widget::Id = widget::Id::new("unlock-field");

/// When `in_progress` is true (the unlock task is in flight), the input is
/// read-only, the primary button is replaced by a spinner, and the alternate
/// unlock methods + Log out button stay visible but inert — same visual
/// weight as the button so the card height doesn't jump.
pub fn view<'a>(
    method: UnlockMethod,
    alternatives: &[UnlockMethod],
    email: Option<&'a str>,
    password: &'a str,
    pin: &'a str,
    in_progress: bool,
    colors: &'a AppColors,
) -> Element<'a, LoginMessage, AppTheme> {
    let lock_icon = svg(svg::Handle::from_memory(crate::assets::LOCK_ICON))
        .width(64)
        .height(60);

    let card = layout::auth_card(card_content(
        method,
        alternatives,
        password,
        pin,
        in_progress,
        colors,
    ));

    layout::icon_title_card(
        lock_icon.into(),
        fl!("login-unlock-title"),
        email.unwrap_or(""),
        card,
        colors,
    )
}

fn card_content<'a>(
    method: UnlockMethod,
    alternatives: &[UnlockMethod],
    password: &'a str,
    pin: &'a str,
    in_progress: bool,
    colors: &'a AppColors,
) -> Element<'a, LoginMessage, AppTheme> {
    let mut items: Vec<Element<'_, LoginMessage, AppTheme>> = Vec::new();

    match method {
        UnlockMethod::Biometrics => {
            items.push(primary_action(
                fl!("login-unlock-biometrics-button"),
                LoginMessage::UnlockWithBiometrics,
                in_progress,
                colors,
            ));
        }
        UnlockMethod::Pin => {
            items.push(reveal_text_field_with_submit(
                Some(UNLOCK_FIELD_ID),
                fl!("login-unlock-pin-placeholder"),
                pin,
                LoginMessage::PinChanged,
                LoginMessage::UnlockWithPin,
                in_progress,
                colors,
            ));
            items.push(primary_action(
                fl!("login-unlock-pin-button"),
                LoginMessage::UnlockWithPin,
                in_progress,
                colors,
            ));
        }
        UnlockMethod::MasterPassword => {
            items.push(reveal_text_field_with_submit(
                Some(UNLOCK_FIELD_ID),
                fl!("login-unlock-password-placeholder"),
                password,
                LoginMessage::PasswordChanged,
                LoginMessage::Unlock,
                in_progress,
                colors,
            ));
            items.push(primary_action(
                fl!("login-unlock-button"),
                LoginMessage::Unlock,
                in_progress,
                colors,
            ));
        }
    }

    // While unlocking, the alternate-method + Log out buttons stay visible
    // but inert (no `on_press`), keeping layout stable and avoiding the jar
    // of UI disappearing and returning around a ~200 ms task.
    items.push(
        text(fl!("login-unlock-or"))
            .size(crate::text_size(14))
            .color(colors.text_primary)
            .center()
            .into(),
    );

    for alt in alternatives {
        let (label, msg) = match alt {
            UnlockMethod::Biometrics => (
                fl!("login-unlock-biometrics-button"),
                LoginMessage::SwitchUnlockMethod(UnlockMethod::Biometrics),
            ),
            UnlockMethod::Pin => (
                fl!("login-unlock-pin-button"),
                LoginMessage::SwitchUnlockMethod(UnlockMethod::Pin),
            ),
            UnlockMethod::MasterPassword => (
                fl!("login-unlock-master-password-button"),
                LoginMessage::SwitchUnlockMethod(UnlockMethod::MasterPassword),
            ),
        };
        items.push(secondary_action(label, msg, in_progress).into());
    }

    items.push(
        secondary_action(
            fl!("login-log-out"),
            LoginMessage::AccountSwitcher(
                crate::components::account_switcher::AccountSwitcherMessage::LogOut,
            ),
            in_progress,
        )
        .into(),
    );

    column(items).spacing(12).align_x(Alignment::Center).into()
}

/// Secondary (outline) button used for alternate unlock methods and Log out.
/// Omits `on_press` while an unlock task is in flight so the widget renders
/// as inert (iced treats a button without `on_press` as disabled).
fn secondary_action<'a>(
    label: String,
    msg: LoginMessage,
    in_progress: bool,
) -> iced::widget::Button<'a, LoginMessage, AppTheme> {
    let mut btn = buttons::secondary(
        container(text(label).size(crate::text_size(16)))
            .center_x(Fill)
            .padding([4, 8]),
    )
    .width(Fill);
    if !in_progress {
        btn = btn.on_press(msg);
    }
    btn
}

/// The primary unlock button. While `in_progress`, it renders as a spinner
/// at the same padded height as the active button so the card layout
/// doesn't shift when the unlock flips into the in-flight state.
fn primary_action<'a>(
    label: String,
    msg: LoginMessage,
    in_progress: bool,
    colors: &'a AppColors,
) -> Element<'a, LoginMessage, AppTheme> {
    if in_progress {
        buttons::primary(
            container(spinner::spinner(21.0, colors.nav_text))
                .center_x(Fill)
                .padding([4, 8]),
        )
        .width(Fill)
        .into()
    } else {
        buttons::primary(
            container(text(label).size(crate::text_size(16)))
                .center_x(Fill)
                .padding([4, 8]),
        )
        .on_press(msg)
        .width(Fill)
        .into()
    }
}
