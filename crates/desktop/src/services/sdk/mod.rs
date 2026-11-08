//! Per-user `PasswordManagerClient` management.
//!
//! Explicit demo mode discovers `*.sqlite` files in the selected directory
//! and pairs them with `mock.json`; normal startup loads no mock accounts. Each user gets a
//! `PasswordManagerClient` whose state registry is backed by that user's
//! SQLite db. [`UnlockData::unlock`](crate::services::sdk::UnlockData::unlock)
//! invokes `initialize_user_crypto` and unlocks the in-memory keystore.
//!
//! ## Dev passwords
//!
//! - `alice@example.com` → `password` (Personal, ~20 ciphers)
//! - `alice@acmecorp.com` → `123456` (Work, ~10 ciphers)
//! - `loadtest@example.com` → `loadtest` (Load Test, ~20k ciphers)
//!
//! Generate a fresh demo via `cargo run -p fake-data -- --output <directory>`.
//!
//! ## Layout
//!
//! - [`types`] — public data types ([`Organization`], [`Collection`],
//!   [`AccountEntry`], [`PasswordHistoryEntry`]).
//! - [`client_ext`] — SDK operation implementations.
//! - [`session_client`] — revocable app-facing `ClientExt` implementation.
//! - [`session`] — authority generations, leases and crypto cleanup.
//! - [`unlock`] — local SDK key unlock.
//! - [`snapshot`] / [`vault_sync`] — atomic encrypted SQLite cache and read-sync coordinator.
//! - [`loader`] — `mock.json` parsing + per-user client construction.
//! - [`user_entry`] — private `UserEntry` struct held by `ClientManager`.

mod authorization;
mod browser;
pub use browser::BrowserSession;
mod favicon;
pub use favicon::FaviconSource;
mod client_ext;
mod loader;
mod local_edits;
pub mod production;
pub mod refresh;
mod removal;
mod session;
mod session_client;
mod snapshot;
pub mod ssh_agent;
mod types;
mod unlock;
mod user_entry;
pub mod vault_sync;

#[cfg(test)]
mod sdk4_tests;
#[cfg(test)]
pub(crate) mod test_support;

use std::collections::HashMap;

use bitwarden_core::UserId;
use bitwarden_send::{SendId, SendView};

use self::user_entry::UserEntry;

pub use authorization::{Action, Authorization};
pub use client_ext::ClientExt;
pub use removal::{LogoutOperation, StorageWarning};
pub use session::{LockOperation, SessionToken};
pub use session_client::SessionClient;
pub use types::{AccountEntry, Collection, Organization, PasswordHistoryEntry};
pub use unlock::UnlockData;

use crate::domain::UnlockMethods;

/// Owns per-user state. Single-threaded: every method runs from the iced
/// update thread. Async SDK work uses revocable [`SessionClient`] handles or
/// exclusive [`UnlockData`] reservations; per-account leases serialize crypto
/// transitions. The manager itself is never borrowed across `.await`.
/// Mutating methods take `&mut self` and run sync from `App::update`.
pub struct ClientManager {
    /// Boxed so the (rather large) `UserEntry` doesn't bloat the HashMap
    /// node; cloning a `PasswordManagerClient` for an async task only needs
    /// the inner `Arc`-backed handle, not the entry itself.
    users: HashMap<UserId, Box<UserEntry>>,
    storage: Option<std::sync::Arc<removal::RemovalStore>>,
    startup_warning: Option<StorageWarning>,
    profiles: Option<crate::services::accounts::profile::ProfileStore>,
    /// In-memory send store keyed by user; not encrypted/persisted yet.
    sends: HashMap<UserId, Vec<SendView>>,
    /// In-memory generator history, keyed by user. Cleared on `log_out`.
    password_history: HashMap<UserId, Vec<PasswordHistoryEntry>>,
}

// Replacing or dropping the owner must not leave authority in cloned task handles.
// Existing leases retain SDK resources only until their own work finishes.
impl Drop for ClientManager {
    fn drop(&mut self) {
        for entry in self.users.values() {
            entry.session.revoke(true);
            entry.session.release_client();
        }
    }
}

// `PasswordManagerClient` doesn't implement `Debug`, so we print just the count
// — needed so `Message` can derive `Debug` with the `ClientManagerLoaded` variant.
impl std::fmt::Debug for ClientManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientManager")
            .field("users", &self.users.len())
            .finish()
    }
}

impl ClientManager {
    /// Initial value while `load()` runs on a background task, so `App::new`
    /// can return and the window can appear before SQLite opens finish.
    pub fn empty() -> Self {
        Self {
            users: HashMap::new(),
            storage: None,
            startup_warning: None,
            profiles: None,
            sends: HashMap::new(),
            password_history: HashMap::new(),
        }
    }

    pub async fn load() -> Self {
        let storage = std::sync::Arc::new(removal::RemovalStore::new(
            crate::paths::data_dir(),
            crate::paths::is_demo(),
        ));
        let root = crate::paths::data_dir();
        let profiles = crate::services::accounts::profile::ProfileStore::system(&root).ok();
        if !crate::paths::is_demo()
            && let Some(store) = &profiles
        {
            let root = root.clone();
            let secrets = store.secrets.clone();
            let prune_store = store.clone();
            if !matches!(
                tokio::task::spawn_blocking(move || {
                    production::recover_pending(&root, &secrets)?;
                    production::prune_orphans(&root, &prune_store)
                })
                .await,
                Ok(Ok(()))
            ) {
                let mut manager = Self::empty();
                manager.startup_warning = Some(StorageWarning::StorageUnavailable);
                manager.profiles = profiles;
                return manager;
            }
        }
        let recovery_store = storage.clone();
        let recovery = tokio::task::spawn_blocking(move || recovery_store.recover()).await;
        let (users, startup_warning) = match recovery {
            Ok(Ok((removed, pending))) => match if crate::paths::is_demo() {
                loader::load_users(&removed).await
            } else if let Some(store) = &profiles {
                production::load(&root, store, &removed).await
            } else {
                Err("Server profile storage unavailable".into())
            } {
                Ok(users) => (users, pending.then_some(StorageWarning::RemovalIncomplete)),
                Err(error) => {
                    tracing::error!(%error, "Account loading failed; refusing to load accounts");
                    (HashMap::new(), Some(StorageWarning::StorageUnavailable))
                }
            },
            _ => {
                tracing::error!("Removal journal unavailable; refusing to load accounts");
                (HashMap::new(), Some(StorageWarning::StorageUnavailable))
            }
        };
        Self {
            users,
            storage: Some(storage),
            startup_warning,
            profiles,
            sends: HashMap::new(),
            password_history: HashMap::new(),
        }
    }

    pub fn profiles(&self) -> Option<crate::services::accounts::profile::ProfileStore> {
        self.profiles.clone()
    }
    pub fn accept_login(
        &mut self,
        candidate: production::Candidate,
        attempt: &crate::services::accounts::login::Attempt,
    ) -> Result<UserId, String> {
        let (id, entry) = candidate.publish(attempt)?;
        self.users.insert(id, entry);
        Ok(id)
    }

    pub fn take_startup_warning(&mut self) -> Option<StorageWarning> {
        self.startup_warning.take()
    }

    /// Revocable SDK handle for an authorized account. Async work checks its
    /// generation under a lease; raw clients never escape the SDK service.
    pub fn client_for(&self, uid: &UserId) -> Option<SessionClient> {
        let e = self.users.get(uid)?;
        if !self.is_unlocked(uid) {
            return None;
        }
        Some(SessionClient {
            session: e.session.token(),
        })
    }

    pub fn session_for(&self, uid: &UserId) -> Option<SessionToken> {
        self.users.get(uid).map(|e| e.session.token())
    }

    pub fn agent_session_for(&self, uid: &UserId) -> Option<ssh_agent::AgentSession> {
        self.is_unlocked(uid)
            .then(|| self.session_for(uid))
            .flatten()
            .map(ssh_agent::AgentSession::new)
    }

    pub fn accepts_session(&self, token: &SessionToken) -> bool {
        self.users
            .get(&token.user_id())
            .is_some_and(|e| token.belongs_to(&e.session) && token.is_current())
    }

    /// Snapshot of the data `UnlockData::unlock` needs. Pulled sync at the
    /// call site so the async task never borrows the manager.
    pub fn unlock_data_for(&self, uid: &UserId) -> Option<UnlockData> {
        let e = self.users.get(uid)?;
        let session = e.session.begin_unlock()?;
        Some(UnlockData {
            _attempt: session::UnlockAttempt(session.clone()),
            session,
            sdk_user_id: e.server_user_id,
            master_unlock: e.master_unlock.clone(),
            email: e.email.clone(),
            crypto_state: e.crypto_state.clone(),
            organizations: e
                .session
                .snapshot
                .current()
                .map(|s| s.organizations.clone())
                .unwrap_or_else(|| e.organizations.clone()),
        })
    }

    /// Snapshot of the data `validate_master_password` needs.
    pub fn validation_data_for(&self, uid: &UserId) -> Option<(SessionClient, String)> {
        let e = self.users.get(uid)?;
        Some((
            self.client_for(uid)?,
            e.master_unlock.master_key_wrapped_user_key.to_string(),
        ))
    }

    pub fn is_remote(&self, uid: &UserId) -> bool {
        self.users.get(uid).is_some_and(|e| e.profile.is_some())
    }
    pub fn local_edit_count(&self, uid: &UserId) -> usize {
        self.users
            .get(uid)
            .and_then(|e| e.session.snapshot.current())
            .map_or(0, |s| s.persisted.local_edits.len())
    }
    pub fn has_local_edit(&self, uid: &UserId, id: bitwarden_vault::CipherId) -> bool {
        self.users
            .get(uid)
            .and_then(|e| e.session.snapshot.current())
            .is_some_and(|s| s.persisted.local_edits.contains_key(&id.to_string()))
    }
    pub fn last_sync(&self, uid: &UserId) -> Option<chrono::DateTime<chrono::Utc>> {
        self.users
            .get(uid)?
            .session
            .snapshot
            .current()
            .map(|s| s.persisted.downloaded_at)
    }
    pub fn profile_for(&self, uid: &UserId) -> Option<uuid::Uuid> {
        self.users.get(uid).and_then(|e| e.profile)
    }
    pub fn clear_grants(&self) {
        for e in self.users.values() {
            e.session.grants.clear();
        }
    }
    pub fn permitted(&self, uid: &UserId, target: &Authorization, consume: bool) -> bool {
        self.users.get(uid).is_some_and(|e| {
            let t = e.session.token();
            t.ensure_unlocked().is_ok() && e.session.grants.permits(target, t.generation, consume)
        })
    }

    /// User IDs sorted by email. `HashMap` order is non-deterministic, so
    /// "first user" callers would otherwise land on a different account each
    /// launch.
    pub fn user_ids(&self) -> Vec<UserId> {
        let mut ids: Vec<UserId> = self.users.keys().copied().collect();
        ids.sort_by(|a, b| self.users[a].email.cmp(&self.users[b].email));
        ids
    }

    /// Callers cache the result on App rather than calling per view rebuild.
    pub fn accounts(&self) -> Vec<AccountEntry> {
        let mut entries: Vec<AccountEntry> = self
            .users
            .iter()
            .map(|(uid, e)| AccountEntry {
                user_id: *uid,
                email: e.email.clone(),
                display_name: e.display_name.clone(),
                server_url: e.server_url.clone(),
                locked: !self.is_unlocked(uid),
            })
            .collect();
        entries.sort_by(|a, b| a.email.cmp(&b.email));
        entries
    }

    pub fn unlock_methods(&self, uid: &UserId) -> Option<UnlockMethods> {
        self.users.get(uid).map(|e| e.unlock_methods.clone())
    }

    pub fn is_unlocking(&self, uid: &UserId) -> bool {
        self.users
            .get(uid)
            .is_some_and(|e| e.session.is_unlocking())
    }

    pub fn is_unlocked(&self, uid: &UserId) -> bool {
        self.users.get(uid).is_some_and(|e| {
            e.session.is_unlocked() && e.session.client().is_ok_and(|c| c.is_unlocked())
        })
    }

    pub fn has_users(&self) -> bool {
        !self.users.is_empty()
    }

    pub fn has_unlocked_users(&self) -> bool {
        self.users.keys().any(|uid| self.is_unlocked(uid))
    }

    pub fn lock(&mut self, uid: &UserId) -> Option<LockOperation> {
        let entry = self.users.get(uid)?;
        let session = entry.session.revoke(false);
        self.sends.remove(uid);
        self.password_history.remove(uid);
        Some(LockOperation { session })
    }

    /// Revoke immediately; the returned operation commits durable removal intent,
    /// drains SDK work, closes its runtime, then removes local account records.
    pub fn log_out(&mut self, uid: &UserId) -> Option<LogoutOperation> {
        let entry = self.users.remove(uid)?;
        let session = entry.session.revoke(true);
        self.sends.remove(uid);
        self.password_history.remove(uid);
        Some(LogoutOperation {
            cleanup: LockOperation { session },
            storage: self.storage.clone(),
        })
    }

    /// Seeded from `mock.json` (no sync path).
    pub fn list_organizations(&self, user_id: &UserId) -> Vec<Organization> {
        self.users
            .get(user_id)
            .map(|e| {
                e.session
                    .snapshot
                    .current()
                    .map(|s| s.organizations.clone())
                    .unwrap_or_else(|| e.organizations.clone())
            })
            .unwrap_or_default()
    }

    /// Collections across all of the user's orgs. Caller filters by
    /// `organization_id` as needed.
    pub fn list_collections(&self, user_id: &UserId) -> Vec<Collection> {
        self.users
            .get(user_id)
            .map(|e| {
                if e.profile.is_some() {
                    e.session.collection_names.lock().unwrap().clone()
                } else {
                    e.collections.clone()
                }
            })
            .unwrap_or_default()
    }

    /// Generate the user's fingerprint phrase (five hyphenated words), used
    /// by the Account → Fingerprint phrase menu. Returns `None` if the user
    /// is unknown or locked (the SDK can't access the private key).
    pub fn user_fingerprint(&self, user_id: &UserId) -> Option<String> {
        if !self.is_unlocked(user_id) {
            return None;
        }
        let entry = self.users.get(user_id)?;
        entry
            .session
            .client()
            .ok()?
            .platform()
            .user_fingerprint(entry.server_user_id.to_string())
            .ok()
    }

    // ── Send stubs ────────────────────────────────────────────────────────
    //
    // Stubbed storage in `self.sends`. No SDK encrypt/decrypt roundtrip yet.
    // When the SDK send flow lands, the create/edit/delete paths will move
    // into `ClientExt`; for now they're sync `&mut self` because no SDK call
    // is involved.

    pub fn list_sends(&self, user_id: &UserId) -> Vec<SendView> {
        let mut list = self.sends.get(user_id).cloned().unwrap_or_default();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    /// Used when the user picks a row — we always open directly into the
    /// edit form, so this returns the full `SendView`.
    pub fn full_send(&self, user_id: &UserId, send_id: SendId) -> Option<SendView> {
        self.sends
            .get(user_id)
            .and_then(|sends| sends.iter().find(|s| s.id == Some(send_id)).cloned())
    }

    /// Insert or replace a send. New sends (no id) get an id + access_id
    /// generated locally so the UI can render the link placeholder.
    pub fn save_send(&mut self, user_id: UserId, mut view: SendView) -> SendView {
        // Mock-only: a real backend would assign id + access_id on POST and
        // return them. With no server, generate them here.
        let id = *view.id.get_or_insert_with(SendId::new_v4);
        if view.access_id.is_none() {
            view.access_id = Some(format!("{:.16}", uuid::Uuid::new_v4().simple()));
        }

        let entry = self.sends.entry(user_id).or_default();
        if let Some(slot) = entry.iter_mut().find(|s| s.id == Some(id)) {
            *slot = view.clone();
        } else {
            entry.push(view.clone());
        }
        view
    }

    /// In-memory stub; the real SDK flow would call `SendsClient::delete`.
    pub fn delete_send(&mut self, user_id: &UserId, send_id: SendId) {
        if let Some(entry) = self.sends.get_mut(user_id) {
            entry.retain(|s| s.id != Some(send_id));
        }
    }

    // ── Generator history ─────────────────────────────────────────────────

    /// Ordered oldest-first by insertion; the view reverses for display.
    pub fn password_history(&self, user_id: &UserId) -> Vec<PasswordHistoryEntry> {
        self.password_history
            .get(user_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn clear_password_history(&mut self, user_id: &UserId) {
        self.password_history.remove(user_id);
    }

    /// Append a freshly-generated value to the user's history. Returns the
    /// updated snapshot so the caller can ship it straight to the view in
    /// the same `App::update` round-trip.
    pub fn push_history(&mut self, user_id: UserId, value: String) -> Vec<PasswordHistoryEntry> {
        let list = self.password_history.entry(user_id).or_default();
        list.push(PasswordHistoryEntry {
            value,
            created: chrono::Utc::now(),
        });
        list.clone()
    }
}
