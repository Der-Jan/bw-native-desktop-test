//! Account-bound authority used only by the explicitly approved browser bridge.
use super::{ClientManager, SessionToken};
use bitwarden_core::{UserId, key_management::SymmetricKeySlotId};
use bitwarden_crypto::SymmetricCryptoKey;
use std::sync::Arc;
#[derive(Clone)]
pub struct BrowserSession {
    pub uid: UserId,
    pub server_user_id: UserId,
    pub email: String,
    pub server: String,
    token: SessionToken,
}
impl BrowserSession {
    pub fn current(&self) -> bool {
        self.token.is_current()
    }
    pub fn unlocked(&self) -> bool {
        self.token.ensure_unlocked().is_ok()
    }
    pub fn same_authority(&self, other: &Self) -> bool {
        self.uid == other.uid
            && self.token.generation == other.token.generation
            && Arc::ptr_eq(&self.token.state, &other.token.state)
            && self.server == other.server
            && self.server_user_id == other.server_user_id
    }
    /// Private bridge caller must obtain explicit approval before requesting this.
    /// Returns only the user key, never the master key/password or OAuth tokens.
    pub(crate) async fn approved_browser_key(&self) -> Result<SymmetricCryptoKey, String> {
        let _lease = self.token.state.gate.read().await;
        self.token.ensure_unlocked()?;
        let client = self.token.state.client()?;
        let store = client.0.internal.get_key_store();
        #[allow(deprecated)]
        let key = store
            .context()
            .dangerous_get_symmetric_key(SymmetricKeySlotId::User)
            .map_err(|_| "Vault key unavailable")?
            .clone();
        self.token.ensure_unlocked()?;
        Ok(key)
    }
}
impl ClientManager {
    pub fn browser_session_for(&self, uid: &UserId) -> Option<BrowserSession> {
        let entry = self.users.get(uid)?;
        Some(BrowserSession {
            uid: *uid,
            server_user_id: entry.server_user_id,
            email: entry.email.clone(),
            server: entry.server_url.clone(),
            token: entry.session.token(),
        })
    }
}
