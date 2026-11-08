//! Bank-account-type read-only section.

use bitwarden_vault::BankAccountView;
use iced::{
    Element, Fill,
    widget::{column, text},
};

use crate::{
    components::inputs::reveal_field,
    fl,
    theme::{AppColors, AppTheme},
    views::vault::widgets::{
        cipher_detail::CipherDetailMessage,
        field_helpers::{card_with_margin, styled_card},
    },
};

use super::shared::push_optional_field;

pub(in super::super) fn bank_account_card<'a>(
    bank: &'a BankAccountView,
    colors: &'a AppColors,
) -> Element<'a, CipherDetailMessage, AppTheme> {
    let mut fields: Vec<Element<'a, CipherDetailMessage, AppTheme>> = Vec::new();
    push_optional_field(
        &mut fields,
        fl!("detail-field-bank-name"),
        bank.bank_name.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-bank-name-on-account"),
        bank.name_on_account.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-bank-account-type"),
        bank.account_type.as_deref(),
        colors,
    );
    if let Some(num) = bank.account_number.as_deref() {
        fields.push(reveal_field(
            fl!("detail-field-bank-account-number"),
            num,
            None,
            colors,
        ));
    }
    push_optional_field(
        &mut fields,
        fl!("detail-field-bank-routing-number"),
        bank.routing_number.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-bank-branch-number"),
        bank.branch_number.as_deref(),
        colors,
    );
    if let Some(pin) = bank.pin.as_deref() {
        fields.push(reveal_field(
            fl!("detail-field-bank-pin"),
            pin,
            None,
            colors,
        ));
    }
    push_optional_field(
        &mut fields,
        fl!("detail-field-bank-swift-code"),
        bank.swift_code.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-bank-iban"),
        bank.iban.as_deref(),
        colors,
    );
    push_optional_field(
        &mut fields,
        fl!("detail-field-bank-contact-phone"),
        bank.bank_contact_phone.as_deref(),
        colors,
    );

    if fields.is_empty() {
        fields.push(
            text(fl!("detail-empty-bank-account"))
                .size(crate::text_size(14))
                .color(colors.text_muted)
                .into(),
        );
    }
    card_with_margin(styled_card(column(fields).spacing(12).width(Fill)))
}
