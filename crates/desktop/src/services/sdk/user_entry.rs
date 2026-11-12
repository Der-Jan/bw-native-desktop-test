//! Per-user SDK state held by [`super::ClientManager`]. Fully private — the
//! app-facing handles are revocable `SessionClient` wrappers or exclusive
//! unlock reservations assembled by `ClientManager`; raw SDK clients stay
//! inside this service.

use bitwarden_core::{
    UserId,
    key_management::{
        MasterPasswordUnlockData, account_cryptographic_state::WrappedAccountCryptographicState,
    },
};

use crate::domain::UnlockMethods;

use super::types::{Collection, Organization};

pub(super) struct UserEntry {
    pub(super) session: std::sync::Arc<super::session::SessionState>,
    // App-side stand-in for SDK profile methods that don't exist yet.
    pub(super) os_unlock: Option<uuid::Uuid>,
    pub(super) email: String,
    pub(super) display_name: String,
    pub(super) server_url: String,
    pub(super) unlock_methods: UnlockMethods,
    pub(super) server_user_id: UserId,
    pub(super) master_unlock: MasterPasswordUnlockData,
    pub(super) crypto_state: WrappedAccountCryptographicState,
    pub(super) profile: Option<uuid::Uuid>,
    /// Seeded from `mock.json` (no sync path in this stub).
    pub(super) organizations: Vec<Organization>,
    pub(super) collections: Vec<Collection>,
}
