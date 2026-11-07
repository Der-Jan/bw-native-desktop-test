//! Domain-object builders for tests. Keeps the boilerplate of constructing
//! 20-field SDK structs out of every `#[cfg(test)] mod tests` block.

use std::sync::Arc;

use bitwarden_core::OrganizationId;
use bitwarden_vault::{
    CipherListView, CipherListViewType, CipherRepromptType, CipherType, CipherView, LoginListView,
    LoginUriView,
};
use chrono::{TimeZone, Utc};

/// Builder spec for [`make_cipher`]. Defaults: secure note named "Test", not
/// favorite, not deleted/archived, personal (no organization).
pub struct CipherSpec {
    pub name: &'static str,
    pub subtitle: &'static str,
    pub favorite: bool,
    pub deleted: bool,
    pub archived: bool,
    pub organization: Option<OrganizationId>,
    pub folder: Option<bitwarden_vault::FolderId>,
    pub collections: Vec<bitwarden_collections::collection::CollectionId>,
    pub kind: CipherListViewType,
}

impl Default for CipherSpec {
    fn default() -> Self {
        Self {
            name: "Test",
            subtitle: "",
            favorite: false,
            deleted: false,
            archived: false,
            organization: None,
            folder: None,
            collections: Vec::new(),
            kind: CipherListViewType::SecureNote,
        }
    }
}

/// Build an [`Arc<CipherListView>`] from a [`CipherSpec`]. All SDK-required
/// fields not exposed on the spec get inert defaults.
pub fn make_cipher(spec: CipherSpec) -> Arc<CipherListView> {
    let stamp = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    Arc::new(CipherListView {
        id: None,
        organization_id: spec.organization,
        folder_id: spec.folder,
        collection_ids: spec.collections,
        partial: false,
        name: spec.name.to_string(),
        subtitle: spec.subtitle.to_string(),
        r#type: spec.kind,
        favorite: spec.favorite,
        reprompt: CipherRepromptType::None,
        organization_use_totp: false,
        edit: true,
        permissions: None,
        view_password: true,
        attachments: 0,
        has_old_attachments: false,
        creation_date: stamp,
        deleted_date: spec.deleted.then_some(stamp),
        revision_date: stamp,
        archived_date: spec.archived.then_some(stamp),
        copyable_fields: Vec::new(),
        local_data: None,
    })
}

/// Build a `CipherListViewType::Login` with the given first URI, no other
/// login fields populated. Pass `None` for a login without URIs.
pub fn login_kind(uri: Option<&str>) -> CipherListViewType {
    CipherListViewType::Login(LoginListView {
        fido2_credentials: None,
        has_fido2: false,
        username: None,
        totp: None,
        uris: uri.map(|u| {
            vec![LoginUriView {
                uri: Some(u.to_string()),
                r#match: None,
                uri_checksum: None,
            }]
        }),
    })
}

pub fn make_cipher_view() -> CipherView {
    let now = Utc::now();
    CipherView {
        id: None,
        organization_id: None,
        folder_id: None,
        collection_ids: Vec::new(),
        key: None,
        partial: false,
        name: "test".to_string(),
        notes: None,
        r#type: CipherType::SecureNote,
        login: None,
        identity: None,
        card: None,
        secure_note: None,
        ssh_key: None,
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
    }
}
