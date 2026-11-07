//! `CipherEditMessage` + `FormEvent` (the return shape of `update()`).

use bitwarden_collections::collection::CollectionId;
use bitwarden_core::OrganizationId;
use bitwarden_vault::{FieldType, FolderId};
use iced::widget::text_editor;

#[derive(Clone)]
pub enum CipherEditMessage {
    // Item details (universal)
    NameChanged(String),
    NotesAction(text_editor::Action),
    FavoriteToggled,
    RepromptToggled,

    // Ownership
    FolderSelected(Option<FolderId>),
    FolderComboClosed,
    OrgSelected(Option<OrganizationId>),
    OrgComboClosed,
    CollectionsDropdownToggled,
    CollectionToggled(CollectionId),

    // Login
    UsernameChanged(String),
    PasswordChanged(String),
    TotpChanged(String),
    UriChanged(usize, String),
    UriAdded,
    UriRemoved(usize),
    /// Remove the passkey at the given index in `login.fido2_credentials`.
    PasskeyRemoved(usize),

    // Card
    CardCardholderChanged(String),
    CardBrandSelected(Option<String>),
    CardNumberChanged(String),
    CardExpMonthSelected(Option<String>),
    CardExpYearChanged(String),
    CardCodeChanged(String),

    // Identity
    IdentityTitleSelected(Option<String>),
    IdentityFirstNameChanged(String),
    IdentityMiddleNameChanged(String),
    IdentityLastNameChanged(String),
    IdentityUsernameChanged(String),
    IdentityCompanyChanged(String),
    IdentitySsnChanged(String),
    IdentityPassportChanged(String),
    IdentityLicenseChanged(String),
    IdentityEmailChanged(String),
    IdentityPhoneChanged(String),
    IdentityAddress1Changed(String),
    IdentityAddress2Changed(String),
    IdentityAddress3Changed(String),
    IdentityCityChanged(String),
    IdentityStateChanged(String),
    IdentityPostalCodeChanged(String),
    IdentityCountryChanged(String),

    // Bank account
    BankNameChanged(String),
    BankNameOnAccountChanged(String),
    BankAccountTypeSelected(Option<String>),
    BankAccountNumberChanged(String),
    BankRoutingNumberChanged(String),
    BankBranchNumberChanged(String),
    BankPinChanged(String),
    BankSwiftCodeChanged(String),
    BankIbanChanged(String),
    BankContactPhoneChanged(String),

    // Drivers license
    DlFirstNameChanged(String),
    DlMiddleNameChanged(String),
    DlLastNameChanged(String),
    DlDateOfBirthChanged(String),
    DlLicenseNumberChanged(String),
    DlIssuingCountryChanged(String),
    DlIssuingStateChanged(String),
    DlIssuingAuthorityChanged(String),
    DlIssueDateChanged(String),
    DlExpirationDateChanged(String),
    DlLicenseClassChanged(String),

    // Passport
    PpGivenNameChanged(String),
    PpSurnameChanged(String),
    PpDateOfBirthChanged(String),
    PpSexChanged(String),
    PpBirthPlaceChanged(String),
    PpNationalityChanged(String),
    PpPassportNumberChanged(String),
    PpPassportTypeChanged(String),
    PpNationalIdNumberChanged(String),
    PpIssuingCountryChanged(String),
    PpIssuingAuthorityChanged(String),
    PpIssueDateChanged(String),
    PpExpirationDateChanged(String),

    // Custom fields
    CustomFieldAdded,
    CustomFieldRemoved(usize),
    CustomFieldTypeSelected(usize, FieldType),
    CustomFieldNameChanged(usize, String),
    CustomFieldValueChanged(usize, String),
    CustomFieldBoolToggled(usize),

    // Flow
    Save,
    Cancel,
}

/// Result of applying a form message. Most messages only mutate the form;
/// Save/Cancel bubble up so the vault router can kick off the save task or
/// dispose the form.
pub enum FormEvent {
    None,
    Save,
    Cancel,
}

impl std::fmt::Debug for CipherEditMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CipherEditMessage(<redacted>)")
    }
}
