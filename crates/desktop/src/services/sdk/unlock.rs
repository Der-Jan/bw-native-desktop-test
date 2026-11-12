//! Local SDK key unlock and reconstruction of cached organizational metadata.
//!
//! `unlock` lives on [`UnlockData`] (not on `ClientExt`) because it needs the
//! kdf params and key envelopes alongside the client. Networking is handled separately.

use std::collections::HashMap;

use bitwarden_core::{
    OrganizationId, UserId,
    key_management::{
        MasterPasswordUnlockData,
        account_cryptographic_state::WrappedAccountCryptographicState,
        crypto::{InitOrgCryptoRequest, InitUserCryptoMethod, InitUserCryptoRequest},
    },
};
use bitwarden_crypto::UnsignedSharedKey;

use super::{
    SessionToken,
    session::{UnlockAttempt, UnlockCleanup},
    types::Organization,
};

/// Bundle of fields `UnlockData::unlock` needs. Pulled sync from the manager
/// via [`super::ClientManager::unlock_data_for`] so the async task is fully
/// owned.
pub struct UnlockData {
    pub(super) session: SessionToken,
    pub(super) _attempt: UnlockAttempt,
    pub sdk_user_id: UserId,
    pub master_unlock: MasterPasswordUnlockData,
    pub email: String,
    pub crypto_state: WrappedAccountCryptographicState,
    pub organizations: Vec<Organization>,
}

impl UnlockData {
    /// Initialize the SDK crypto state with the user's master password. On
    /// success the keystore is unlocked in memory.
    pub async fn unlock(self, password: String) -> Result<(), String> {
        let method = InitUserCryptoMethod::MasterPasswordUnlock {
            password,
            master_password_unlock: self.master_unlock.clone(),
        };
        self.unlock_method(method).await
    }
    pub(super) async fn unlock_method(self, method: InitUserCryptoMethod) -> Result<(), String> {
        let _exclusive = self.session.state.gate.write().await;
        self.session.ensure_unlocking()?;
        let client = self.session.state.client()?;
        // Declared after the lease: cancellation clears keys before releasing it.
        let mut cleanup = UnlockCleanup::new(&client, self.session.clone());
        let req = InitUserCryptoRequest {
            // Reuse the Client's bound UserId on every unlock — the SDK binds
            // it on first `init_user_id` (called by `load_from_state`), so
            // subsequent unlocks MUST pass the same id. `init_user_id` is
            // idempotent for the same value.
            user_id: Some(self.sdk_user_id),
            kdf_params: self.master_unlock.kdf.clone(),
            email: self.email.clone(),
            account_cryptographic_state: self.crypto_state,
            method,
            upgrade_token: None,
        };

        client
            .crypto()
            .initialize_user_crypto(req)
            .await
            .map_err(|e| e.to_string())?;

        // Replay org keys so the vault client can encrypt/decrypt org-owned
        // ciphers. Orgs without a wrapped key contribute nothing.
        let org_keys: HashMap<OrganizationId, UnsignedSharedKey> = self
            .organizations
            .into_iter()
            .filter_map(|o| o.wrapped_key.map(|k| (o.id, k)))
            .collect();
        if !org_keys.is_empty() {
            client
                .crypto()
                .initialize_org_crypto(InitOrgCryptoRequest {
                    organization_keys: org_keys,
                })
                .await
                .map_err(|e| e.to_string())?;
        }

        let names = self
            .session
            .state
            .snapshot
            .current()
            .map(|s| s.collection_names(&client))
            .transpose()?
            .unwrap_or_default();
        cleanup.commit()?;
        self.session.publish_collections(names)
    }
}

#[cfg(test)]
mod tests {
    use crate::services::sdk::test_support::unlocked_manager;

    #[tokio::test]
    async fn dropping_an_unpolled_unlock_releases_its_reservation() {
        let uid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
        let mut manager = unlocked_manager(uid);
        manager.lock(&uid).unwrap().finish().await;
        let future = manager.unlock_data_for(&uid).unwrap().unlock("test".into());
        assert!(manager.is_unlocking(&uid));
        drop(future);
        assert!(!manager.is_unlocking(&uid));
        assert!(manager.unlock_data_for(&uid).is_some());
    }

    #[tokio::test]
    async fn cancelling_while_waiting_for_crypto_does_not_leave_unlock_stuck() {
        let uid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
        let mut manager = unlocked_manager(uid);
        manager.lock(&uid).unwrap().finish().await;
        let data = manager.unlock_data_for(&uid).unwrap();
        let state = data.session.state.clone();
        let lease = state.gate.write().await;
        let task = tokio::spawn(data.unlock("test".into()));
        tokio::task::yield_now().await;
        assert!(manager.is_unlocking(&uid));
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(!manager.is_unlocking(&uid));
        drop(lease);
        assert!(manager.unlock_data_for(&uid).is_some());
    }
}
