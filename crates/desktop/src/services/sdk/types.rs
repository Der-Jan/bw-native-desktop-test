//! Public data types exposed by the sdk module.
//!
//! Plain `Clone`/`Debug` POD: every field is read by the UI directly. Kept
//! separate from the manager so callers needing just a type don't pull in
//! the full `ClientManager` surface.

use bitwarden_collections::collection::CollectionId;
use bitwarden_core::{OrganizationId, UserId};
use bitwarden_crypto::UnsignedSharedKey;
use serde::Deserialize;

/// App-level org metadata. The SDK has no `Repository<Organization>` for
/// this UI-only stub, so we read it from `mock.json` and hold it here.
#[derive(Deserialize, Clone, Debug)]
pub struct Organization {
    pub id: OrganizationId,
    pub name: String,
    /// Org's symmetric key, wrapped with the user's public key. Replayed on
    /// `unlock` via `initialize_org_crypto`. `None` for orgs added without a key.
    #[serde(default)]
    pub wrapped_key: Option<UnsignedSharedKey>,
}

/// Source / destination vault picker shared by the import and export modals:
/// the user's personal vault, or one of their organizations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultChoice {
    Personal,
    Org { id: OrganizationId, name: String },
}

impl VaultChoice {
    /// Caller supplies the localized "personal" label so each modal can use
    /// its own fluent key without this type owning either.
    pub fn label(&self, personal_label: &str) -> String {
        match self {
            Self::Personal => personal_label.to_string(),
            Self::Org { name, .. } => name.clone(),
        }
    }

    pub fn list_with_personal(orgs: &[Organization]) -> Vec<Self> {
        let mut choices = Vec::with_capacity(orgs.len() + 1);
        choices.push(Self::Personal);
        for org in orgs {
            choices.push(Self::Org {
                id: org.id,
                name: org.name.clone(),
            });
        }
        choices
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Collection {
    pub id: CollectionId,
    pub organization_id: OrganizationId,
    pub name: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub hide_passwords: bool,
}

/// UI-facing snapshot of a single user account. App caches one
/// `Vec<AccountEntry>` and hands it to views via `RenderCtx::accounts`.
pub struct AccountEntry {
    pub user_id: UserId,
    pub email: String,
    #[expect(dead_code)] // Not displayed yet; reserved for future avatar / profile views.
    pub display_name: String,
    pub server_url: String,
    pub locked: bool,
}

/// One entry in the in-memory generator history. Flat across password /
/// passphrase / username — matches the official Bitwarden client.
#[derive(Debug, Clone)]
pub struct PasswordHistoryEntry {
    pub value: String,
    pub created: chrono::DateTime<chrono::Utc>,
}
