//! Top-level send form view: header + scrollable body (two cards) + footer.
//! The cards themselves live in [`super::sections`].

use bitwarden_send::SendType;
use iced::{
    Alignment, Element, Fill,
    widget::{Space, column, row, scrollable, text},
};

use crate::{
    components::{self, buttons},
    fl,
    theme::{AppColors, AppTheme},
};

use super::{
    message::SendEditMessage,
    sections::{additional::additional_options_card, details::details_card},
    state::SendForm,
};

pub fn view<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
    top_radius: f32,
) -> Element<'a, SendEditMessage, AppTheme> {
    let is_new = form.id.is_none();

    let title = match (is_new, form.send_type) {
        (true, SendType::Text) => fl!("send-form-title-new-text"),
        (true, SendType::File) => fl!("send-form-title-new-file"),
        (false, SendType::Text) => fl!("send-form-title-edit-text"),
        (false, SendType::File) => fl!("send-form-title-edit-file"),
        (_, SendType::Item) => fl!("send-item-unsupported"),
    };

    let header = components::pane_header(title, SendEditMessage::CancelPressed, colors);

    let body: Element<'a, SendEditMessage, AppTheme> = scrollable(
        column![
            details_card(form, colors),
            additional_options_card(form, colors)
        ]
        .spacing(16)
        .padding([16, 20]),
    )
    .height(Fill)
    .style(components::rail_scroll_style)
    .into();

    let footer = components::pane_footer(footer_content(form, colors));

    components::rounded_top_pane(column![header, body, footer].height(Fill), top_radius)
}

// ── Footer (Save / Cancel / Delete) ───────────────────────────────────────

fn footer_content<'a>(
    form: &'a SendForm,
    colors: &'a AppColors,
) -> Element<'a, SendEditMessage, AppTheme> {
    let mut save =
        buttons::primary(text(fl!("send-form-save")).size(crate::text_size(14))).padding([8, 20]);
    if !form.saving {
        save = save.on_press(SendEditMessage::SavePressed);
    }

    let cancel = buttons::secondary(text(fl!("send-form-cancel")).size(crate::text_size(14)))
        .on_press(SendEditMessage::CancelPressed)
        .padding([8, 20]);

    let mut left_row = row![save, cancel].spacing(8).align_y(Alignment::Center);

    // Delete icon only for existing sends — there's no saved resource to
    // remove on a brand-new draft. Uses `titlebar_close_hover` (red) for
    // consistency with the vault detail pane's trash icon.
    if form.id.is_some() {
        left_row = left_row.push(Space::new().width(Fill));
        left_row = left_row.push(buttons::delete_icon_button(
            SendEditMessage::DeletePressed,
            colors,
        ));
    }

    left_row.into()
}
