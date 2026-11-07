//! Top-level read-only detail pane: header + scrollable body dispatching on
//! cipher type + bottom bar. Parallel to `cipher_edit::view`.

use bitwarden_vault::{CipherType, CipherView};
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
    CipherDetailMessage,
    sections::{bank_account, card, drivers_license, identity, login, passport, shared, ssh_key},
};
use crate::views::vault::widgets::field_helpers::{card_with_margin, section_label, styled_card};

pub fn view<'a>(
    item: &'a CipherView,
    colors: &'a AppColors,
    top_radius: f32,
    read_only: bool,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let header = header_row(item, colors);

    let mut sections: Vec<Element<'a, CipherDetailMessage, AppTheme>> = vec![
        section_label(fl!("detail-section-item-details"), colors),
        shared::item_details_card(item, colors),
    ];

    match item.r#type {
        CipherType::Login => {
            if let Some(login_view) = item.login.as_ref() {
                sections.push(section_label(
                    fl!("detail-section-login-credentials"),
                    colors,
                ));
                sections.push(login::login_card(login_view, colors));

                let uris = login::collect_login_uris(login_view);
                if !uris.is_empty() {
                    sections.push(section_label(
                        fl!("detail-section-autofill-options"),
                        colors,
                    ));
                    sections.push(login::autofill_card(&uris, colors));
                }
            }
        }
        CipherType::Card => {
            if let Some(card_view) = item.card.as_ref() {
                sections.push(section_label(fl!("detail-section-card-details"), colors));
                sections.push(card::card_details_card(card_view, colors));
            }
        }
        CipherType::Identity => {
            if let Some(identity_view) = item.identity.as_ref() {
                sections.push(section_label(
                    fl!("detail-section-personal-details"),
                    colors,
                ));
                sections.push(identity::identity_card(identity_view, colors));
            }
        }
        CipherType::SecureNote => {
            if let Some(notes) = item.notes.as_deref() {
                sections.push(section_label(fl!("detail-section-note"), colors));
                sections.push(card_with_margin(styled_card(
                    text(notes).size(14).color(colors.text_primary),
                )));
            }
        }
        CipherType::SshKey => {
            if let Some(key) = item.ssh_key.as_ref() {
                sections.push(section_label(fl!("detail-section-ssh-key"), colors));
                sections.push(ssh_key::ssh_key_card(key, colors));
            }
        }
        CipherType::BankAccount => {
            if let Some(bank) = item.bank_account.as_ref() {
                sections.push(section_label(fl!("detail-section-bank-account"), colors));
                sections.push(bank_account::bank_account_card(bank, colors));
            }
        }
        CipherType::DriversLicense => {
            if let Some(dl) = item.drivers_license.as_ref() {
                sections.push(section_label(fl!("detail-section-drivers-license"), colors));
                sections.push(drivers_license::drivers_license_card(dl, colors));
            }
        }
        CipherType::Passport => {
            if let Some(pp) = item.passport.as_ref() {
                sections.push(section_label(fl!("detail-section-passport"), colors));
                sections.push(passport::passport_card(pp, colors));
            }
        }
    }

    if let Some(fields) = item.fields.as_deref().filter(|f| !f.is_empty()) {
        sections.push(section_label(fl!("detail-section-custom-fields"), colors));
        sections.push(shared::custom_fields_card(fields, colors));
    }

    let body = scrollable(column(sections).spacing(4).padding([12, 20])).height(Fill);
    let bottom = if read_only {
        components::pane_footer(text("Synchronisierter Tresor · schreibgeschützt").size(12))
    } else {
        bottom_bar(colors)
    };

    components::rounded_top_pane(
        column![header, body, bottom].spacing(0).height(Fill),
        top_radius,
    )
}

fn header_row<'a>(
    item: &'a CipherView,
    colors: &'a AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let category_label = match item.r#type {
        CipherType::Login => fl!("detail-header-login"),
        CipherType::Card => fl!("detail-header-card"),
        CipherType::Identity => fl!("detail-header-identity"),
        CipherType::SecureNote => fl!("detail-header-note"),
        CipherType::SshKey => fl!("detail-header-ssh-key"),
        CipherType::BankAccount => fl!("detail-header-bank-account"),
        CipherType::DriversLicense => fl!("detail-header-drivers-license"),
        CipherType::Passport => fl!("detail-header-passport"),
    };
    components::pane_header(category_label, CipherDetailMessage::Close, colors)
}

fn bottom_bar<'a>(colors: &'a AppColors) -> Element<'a, CipherDetailMessage, AppTheme> {
    let edit_btn = buttons::primary(text(fl!("detail-edit-button")).size(14))
        .on_press(CipherDetailMessage::Edit)
        .padding([8, 20]);
    let delete_btn = buttons::delete_icon_button(CipherDetailMessage::Delete, colors);
    components::pane_footer(
        row![edit_btn, Space::new().width(Fill), delete_btn].align_y(Alignment::Center),
    )
}
