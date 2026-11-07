//! Signing prompt: no secret key or raw authentication payload enters the UI.
use crate::{
    components::{buttons, focus_button::FocusButton, modal},
    fl,
    services::ssh_agent::{Approval, Decision},
    theme::{AppColors, AppTheme},
};
use iced::{
    Element, Fill,
    widget::{column, row, scrollable, text},
};
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Decide(Decision),
    Focus(Decision),
}
pub const ALLOW_ONCE_ID: iced::widget::Id = iced::widget::Id::new("ssh-approval-allow-once");
pub const UNTIL_LOCK_ID: iced::widget::Id = iced::widget::Id::new("ssh-approval-until-lock");
pub const DENY_ID: iced::widget::Id = iced::widget::Id::new("ssh-approval-deny");
pub fn focus_id(decision: Decision) -> iced::widget::Id {
    match decision {
        Decision::Once => ALLOW_ONCE_ID,
        Decision::UntilLock => UNTIL_LOCK_ID,
        Decision::Deny => DENY_ID,
    }
}

pub fn view<'a>(
    approval: &'a Arc<Approval>,
    colors: &'a AppColors,
) -> Element<'a, (Uuid, Action), AppTheme> {
    let deny = (approval.id, Action::Decide(Decision::Deny));
    let mut details = column![
        text(fl!("ssh-approval-key", name = approval.key_name.clone())).size(16),
        text(approval.fingerprint.clone()).size(14),
        text(fl!("ssh-approval-user", name = approval.username.clone())).size(14),
        text(fl!("ssh-approval-process", name = approval.process.clone())).size(14),
        text(fl!("ssh-approval-process-note"))
            .size(12)
            .color(colors.text_secondary),
    ]
    .spacing(10)
    .width(Fill);
    if approval.hosts.is_empty() {
        details = details.push(text(fl!("ssh-approval-unbound")).size(14));
    } else {
        details = details.push(text(fl!("ssh-approval-hosts")).size(14));
        for host in &approval.hosts {
            details = details.push(text(host.clone()).size(12));
        }
    }
    if !approval.hosts.is_empty() && !approval.can_remember {
        details = details.push(text(fl!("ssh-approval-bound-once")).size(14));
    }
    let action = |decision: Decision, label: String, next: Decision, previous: Decision| {
        let message = (approval.id, Action::Decide(decision));
        let button = if approval.default_decision == decision {
            buttons::primary(text(format!("{label}  ↵")))
        } else {
            buttons::secondary(text(label))
        }
        .on_press(message);
        FocusButton::new(
            button,
            focus_id(decision),
            approval.default_decision == decision,
            message,
            (approval.id, Action::Focus(next)),
            (approval.id, Action::Focus(previous)),
        )
    };
    let last = if approval.can_remember {
        Decision::UntilLock
    } else {
        Decision::Once
    };
    let mut actions = row![
        action(
            Decision::Deny,
            fl!("ssh-approval-deny"),
            Decision::Once,
            last
        ),
        action(
            Decision::Once,
            fl!("ssh-approval-once"),
            if approval.can_remember {
                Decision::UntilLock
            } else {
                Decision::Deny
            },
            Decision::Deny
        )
    ]
    .spacing(10);
    if approval.can_remember {
        actions = actions.push(action(
            Decision::UntilLock,
            fl!("ssh-approval-remember"),
            Decision::Deny,
            Decision::Once,
        ));
    }
    let body = column![
        modal::dialog_header(fl!("ssh-approval-title"), deny, colors),
        // Keep actions visible even with long key names, usernames or host paths.
        scrollable(details).height(260.0),
        text(fl!("ssh-approval-timeout"))
            .size(12)
            .color(colors.text_secondary),
        actions
    ]
    .spacing(16)
    .padding(modal::BODY_PADDING);
    modal::dialog(
        660.0,
        None,
        |c| c.card_bg,
        1.0,
        iced::widget::keyed_column([(approval.id, body.into())]),
        deny,
    )
}
