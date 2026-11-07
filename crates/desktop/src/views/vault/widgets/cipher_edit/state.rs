//! `CipherForm` struct + choice enums + constructors/mutators.

use bitwarden_core::OrganizationId;
use bitwarden_ssh::generator::{KeyAlgorithm, generate_sshkey};
use bitwarden_vault::{
    CardView, CipherRepromptType, CipherType, CipherView, FolderId, FolderView, IdentityView,
    LoginView, SecureNoteType, SecureNoteView,
};
use chrono::{NaiveDate, Utc};
use iced::widget::{combo_box, text_editor};

use crate::{
    fl,
    services::sdk::{Collection, Organization},
};

// ── Choice enums for searchable dropdowns ─────────────────────────────────

#[derive(Debug, Clone)]
pub struct FolderOption {
    pub id: FolderId,
    pub name: String,
}

impl TryFrom<FolderView> for FolderOption {
    type Error = ();

    fn try_from(v: FolderView) -> Result<Self, Self::Error> {
        // Folders from `FoldersClient::list()` are documented to always
        // carry an id — but a partial-sync edge case or upstream regression
        // shouldn't crash the app. Drop entries without an id at load.
        Ok(Self {
            id: v.id.ok_or(())?,
            name: v.name,
        })
    }
}

pub type OrganizationOption = Organization;
pub type CollectionOption = Collection;

/// Options shown in the searchable folder `combo_box`. `None` is a first-class
/// "No folder" entry so users can clear the selection by picking it — iced's
/// `combo_box` doesn't surface a separate clear signal.
#[derive(Debug, Clone)]
pub(super) enum FolderChoice {
    None,
    Folder(FolderOption),
}

impl FolderChoice {
    pub(super) fn id(&self) -> Option<FolderId> {
        match self {
            Self::None => None,
            Self::Folder(f) => Some(f.id),
        }
    }
}

impl std::fmt::Display for FolderChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str(&fl!("form-folder-none")),
            Self::Folder(fo) => f.write_str(&fo.name),
        }
    }
}

/// Options shown in the searchable organization `combo_box`. `None` is
/// the "Personal (me)" entry — personal ciphers have no organization.
#[derive(Debug, Clone)]
pub(super) enum OrgChoice {
    None,
    Org(OrganizationOption),
}

impl OrgChoice {
    pub(super) fn id(&self) -> Option<OrganizationId> {
        match self {
            Self::None => None,
            Self::Org(o) => Some(o.id),
        }
    }
}

impl std::fmt::Display for OrgChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str(&fl!("form-organization-personal")),
            Self::Org(o) => f.write_str(&o.name),
        }
    }
}

/// Preserve incomplete text input while SDK 4 stores validated calendar dates.
#[derive(Default)]
pub(super) struct DateFields {
    pub(super) birth: String,
    pub(super) issued: String,
    pub(super) expires: String,
}

impl DateFields {
    fn from_cipher(cipher: &CipherView) -> Self {
        let dates = match cipher.r#type {
            CipherType::DriversLicense => cipher
                .drivers_license
                .as_ref()
                .map(|d| (d.date_of_birth, d.issue_date, d.expiration_date)),
            CipherType::Passport => cipher
                .passport
                .as_ref()
                .map(|p| (p.date_of_birth, p.issue_date, p.expiration_date)),
            _ => None,
        }
        .unwrap_or((None, None, None));
        Self {
            birth: dates.0.map(|d| d.to_string()).unwrap_or_default(),
            issued: dates.1.map(|d| d.to_string()).unwrap_or_default(),
            expires: dates.2.map(|d| d.to_string()).unwrap_or_default(),
        }
    }

    fn is_valid(&self) -> bool {
        [&self.birth, &self.issued, &self.expires]
            .into_iter()
            .all(|s| s.is_empty() || s.parse::<NaiveDate>().is_ok())
    }
}

// ── CipherForm struct ──────────────────────────────────────────────────────

/// Form backing the cipher edit view. `original = None` distinguishes the
/// "New item" flow from "Edit existing".
pub struct CipherForm {
    pub original: Option<CipherView>,
    pub modified: CipherView,
    pub(super) dates: DateFields,

    pub folders: Vec<FolderOption>,
    pub organizations: Vec<OrganizationOption>,
    pub collections: Vec<CollectionOption>,

    // `combo_box` holds its filter text / open state in a `RefCell<Inner<T>>`
    // that must persist across frames, so we keep the state on the struct
    // rather than recreating it in `view()`. Rebuilt by `set_folders` /
    // `set_organizations` when the SDK lists arrive.
    pub(super) folder_combo_state: combo_box::State<FolderChoice>,
    pub(super) org_combo_state: combo_box::State<OrgChoice>,

    // Custom DropDown widget; pick_list is single-select.
    pub(super) collections_dropdown_open: bool,

    /// Multi-line editor buffer for the Notes field. `text_editor` requires
    /// its content/cursor state to live on the parent; mutations flow through
    /// `CipherEditMessage::NotesAction`.
    pub(super) notes_content: text_editor::Content,

    /// Disables the Save button + form inputs while the save task is in flight.
    pub saving: bool,
}

impl CipherForm {
    /// Require a name and valid optional calendar dates. Partial date input
    /// stays in the form and cannot silently overwrite a valid stored date.
    pub fn is_valid(&self) -> bool {
        !self.modified.name.trim().is_empty() && self.dates.is_valid()
    }

    /// Construct an empty form for creating a new cipher of the given type.
    /// `id` stays `None` until the server (mocked by `ClientManager::save_cipher`)
    /// allocates one. `original = None` so the form renders the "new item"
    /// title and skips diff logic.
    pub fn new(cipher_type: CipherType, organization_id: Option<OrganizationId>) -> Self {
        let now = Utc::now();
        // SSH keys are generated up-front (Ed25519 — fast, sync) so the form
        // opens already populated with private/public/fingerprint. Other types
        // are filled in by `ensure_sub_structs()` below.
        let ssh_key = (cipher_type == CipherType::SshKey)
            .then(|| generate_sshkey(KeyAlgorithm::Ed25519))
            .and_then(|res| match res {
                Ok(view) => Some(view.into()),
                Err(err) => {
                    tracing::error!(%err, "ssh key generation failed; opening form with empty fields");
                    None
                }
            });
        let modified = CipherView {
            id: None,
            organization_id,
            folder_id: None,
            collection_ids: Vec::new(),
            key: None,
            partial: false,
            name: String::new(),
            notes: None,
            r#type: cipher_type,
            login: None,
            identity: None,
            card: None,
            secure_note: None,
            ssh_key,
            bank_account: None,
            drivers_license: None,
            passport: None,
            favorite: false,
            reprompt: CipherRepromptType::None,
            organization_use_totp: false,
            edit: true,
            permissions: None,
            view_password: true,
            local_data: None,
            attachments: None,
            attachment_decryption_failures: None,
            fields: None,
            password_history: None,
            creation_date: now,
            deleted_date: None,
            revision_date: now,
            archived_date: None,
        };
        let mut form = Self {
            dates: DateFields::default(),
            original: None,
            modified,
            folders: Vec::new(),
            organizations: Vec::new(),
            collections: Vec::new(),
            folder_combo_state: combo_box::State::new(vec![FolderChoice::None]),
            org_combo_state: combo_box::State::new(vec![OrgChoice::None]),
            collections_dropdown_open: false,
            notes_content: text_editor::Content::new(),
            saving: false,
        };
        form.ensure_sub_structs();
        form
    }

    /// Construct for editing an existing cipher. Clones the view so `original`
    /// stays pristine regardless of what form events do to `modified`.
    pub fn edit(cv: CipherView) -> Self {
        let notes_content = text_editor::Content::with_text(cv.notes.as_deref().unwrap_or(""));
        let mut form = Self {
            dates: DateFields::from_cipher(&cv),
            original: Some(cv.clone()),
            modified: cv,
            folders: Vec::new(),
            organizations: Vec::new(),
            collections: Vec::new(),
            folder_combo_state: combo_box::State::new(vec![FolderChoice::None]),
            org_combo_state: combo_box::State::new(vec![OrgChoice::None]),
            collections_dropdown_open: false,
            notes_content,
            saving: false,
        };
        form.ensure_sub_structs();
        form
    }

    /// Replace the folder list and rebuild the combo_box state. Uses
    /// `State::new` (rather than `State::with_selection`) so the internal
    /// `value` field starts empty — combo_box uses `value` as both the
    /// displayed text *and* the filter key, so a pre-filled value would
    /// reduce the next open to a single filtered row. The currently-selected
    /// folder is rendered via the `selected` argument passed into
    /// `search_select_field`; nothing in combo_box state needs to know it.
    pub fn set_folders(&mut self, folders: Vec<FolderOption>) {
        let mut choices = Vec::with_capacity(folders.len() + 1);
        choices.push(FolderChoice::None);
        choices.extend(folders.iter().cloned().map(FolderChoice::Folder));

        self.folder_combo_state = combo_box::State::new(choices);
        self.folders = folders;
    }

    /// Replace the organization list and rebuild the combo_box state. Same
    /// pattern as `set_folders` — `State::new` (empty value) so the next
    /// expand shows the full list unfiltered.
    pub fn set_organizations(&mut self, organizations: Vec<OrganizationOption>) {
        let mut choices = Vec::with_capacity(organizations.len() + 1);
        choices.push(OrgChoice::None);
        choices.extend(organizations.iter().cloned().map(OrgChoice::Org));

        self.org_combo_state = combo_box::State::new(choices);
        self.organizations = organizations;
    }

    /// Ensure the type-specific sub-view (`login`, `card`, ...) is populated
    /// on `modified` so form handlers can mutate `.as_mut()?` without None
    /// short-circuiting on first edit.
    fn ensure_sub_structs(&mut self) {
        match self.modified.r#type {
            CipherType::Login => {
                self.modified.login.get_or_insert(LoginView {
                    username: None,
                    password: None,
                    password_revision_date: None,
                    uris: None,
                    totp: None,
                    autofill_on_page_load: None,
                    http_authentication_enabled: None,
                    fido2_credentials: None,
                });
            }
            CipherType::Card => {
                self.modified.card.get_or_insert(CardView {
                    cardholder_name: None,
                    exp_month: None,
                    exp_year: None,
                    code: None,
                    brand: None,
                    number: None,
                });
            }
            CipherType::Identity => {
                self.modified.identity.get_or_insert(IdentityView {
                    title: None,
                    first_name: None,
                    middle_name: None,
                    last_name: None,
                    address1: None,
                    address2: None,
                    address3: None,
                    city: None,
                    state: None,
                    postal_code: None,
                    country: None,
                    company: None,
                    email: None,
                    phone: None,
                    ssn: None,
                    username: None,
                    passport_number: None,
                    license_number: None,
                });
            }
            CipherType::SecureNote => {
                self.modified.secure_note.get_or_insert(SecureNoteView {
                    r#type: SecureNoteType::Generic,
                });
            }
            CipherType::SshKey => { /* SSH key fields read-only for now */ }
            CipherType::BankAccount => {
                self.modified.bank_account.get_or_insert_default();
            }
            CipherType::DriversLicense => {
                self.modified.drivers_license.get_or_insert_default();
            }
            CipherType::Passport => {
                self.modified.passport.get_or_insert_default();
            }
        }
    }
}

#[cfg(test)]
mod sdk4_date_tests {
    use super::*;
    use crate::views::vault::widgets::cipher_edit::CipherEditMessage;

    #[test]
    fn partial_or_invalid_dates_keep_text_and_cannot_be_saved() {
        let mut form = CipherForm::new(CipherType::DriversLicense, None);
        form.modified.name = "Synthetic license".into();
        for input in ["2", "2024-", "2024-02-30"] {
            form.update(CipherEditMessage::DlDateOfBirthChanged(input.into()));
            assert_eq!(form.dates.birth, input);
            assert!(!form.is_valid());
            assert!(
                form.modified
                    .drivers_license
                    .as_ref()
                    .unwrap()
                    .date_of_birth
                    .is_none()
            );
        }
        form.update(CipherEditMessage::DlDateOfBirthChanged("2024-02-29".into()));
        assert!(form.is_valid());
        assert_eq!(
            form.modified
                .drivers_license
                .as_ref()
                .unwrap()
                .date_of_birth,
            NaiveDate::from_ymd_opt(2024, 2, 29)
        );
        form.update(CipherEditMessage::DlDateOfBirthChanged(String::new()));
        assert!(form.is_valid());
        assert!(
            form.modified
                .drivers_license
                .as_ref()
                .unwrap()
                .date_of_birth
                .is_none()
        );
    }

    #[test]
    fn editing_a_passport_preserves_existing_dates_while_input_is_incomplete() {
        let mut original = CipherForm::new(CipherType::Passport, None);
        original.modified.name = "Synthetic passport".into();
        let issued = NaiveDate::from_ymd_opt(2020, 1, 2);
        original.modified.passport.as_mut().unwrap().issue_date = issued;
        let mut form = CipherForm::edit(original.modified);
        assert_eq!(form.dates.issued, "2020-01-02");
        form.update(CipherEditMessage::PpIssueDateChanged("2025-".into()));
        assert_eq!(form.dates.issued, "2025-");
        assert!(!form.is_valid());
        assert_eq!(form.modified.passport.as_ref().unwrap().issue_date, issued);
        form.update(CipherEditMessage::PpIssueDateChanged("2025-12-31".into()));
        assert!(form.is_valid());
        assert_eq!(
            form.modified.passport.as_ref().unwrap().issue_date,
            NaiveDate::from_ymd_opt(2025, 12, 31)
        );
    }
}
