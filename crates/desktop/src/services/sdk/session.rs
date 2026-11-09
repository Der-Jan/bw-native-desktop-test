//! Per-account authority and serialization for SDK work.
//!
//! Revocation is synchronous; key cleanup waits for existing SDK operations.
//! A token carries its account incarnation through its private Arc identity.

use std::sync::{Arc, Mutex};

use bitwarden_core::UserId;
use bitwarden_pm::PasswordManagerClient;
use tokio::sync::RwLock;

pub(super) const STALE_SESSION: &str = "Account session is no longer authorized";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Locked,
    Unlocking,
    Unlocked,
    Locking,
    Removed,
}

struct Status {
    generation: u64,
    phase: Phase,
}

pub(super) struct SessionState {
    uid: UserId,
    status: Mutex<Status>,
    pub(super) gate: RwLock<()>,
    client: Mutex<Option<PasswordManagerClient>>,
    pub(super) grants: super::authorization::Grants,
    pub(super) writable: std::sync::atomic::AtomicBool,
    pub(super) network_authorized: std::sync::atomic::AtomicBool,
    pub(super) snapshot: super::snapshot::State,
    pub(super) collection_names: Mutex<Vec<super::Collection>>,
    pub(super) sync_gate: tokio::sync::Mutex<()>,
    pub(super) icon_gate: tokio::sync::Mutex<()>,
    pub(super) sync_sequence: std::sync::atomic::AtomicU64,
}

/// Authority captured when an operation starts. Contains no vault data or SDK handle.
#[derive(Clone)]
pub struct SessionToken {
    pub(super) state: Arc<SessionState>,
    pub(super) generation: u64,
}

impl std::fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionToken")
            .field("uid", &self.state.uid)
            .field("generation", &self.generation)
            .finish()
    }
}

impl SessionState {
    pub(super) fn new(uid: UserId, client: PasswordManagerClient) -> Arc<Self> {
        Arc::new(Self {
            uid,
            status: Mutex::new(Status {
                generation: 0,
                phase: Phase::Locked,
            }),
            gate: RwLock::new(()),
            client: Mutex::new(Some(client)),
            grants: Default::default(),
            writable: std::sync::atomic::AtomicBool::new(true),
            network_authorized: std::sync::atomic::AtomicBool::new(true),
            snapshot: Default::default(),
            collection_names: Mutex::new(Vec::new()),
            sync_gate: tokio::sync::Mutex::new(()),
            icon_gate: tokio::sync::Mutex::new(()),
            sync_sequence: std::sync::atomic::AtomicU64::new(0),
        })
    }

    pub(super) fn client(&self) -> Result<PasswordManagerClient, String> {
        self.client
            .lock()
            .expect("SDK owner poisoned")
            .as_ref()
            .map(|client| PasswordManagerClient(client.0.clone()))
            .ok_or_else(|| STALE_SESSION.into())
    }

    pub(super) fn release_client(&self) {
        self.client.lock().expect("SDK owner poisoned").take();
    }

    pub(super) fn token(self: &Arc<Self>) -> SessionToken {
        let status = self.status.lock().expect("session status poisoned");
        SessionToken {
            state: Arc::clone(self),
            generation: status.generation,
        }
    }

    pub(super) fn is_unlocking(&self) -> bool {
        self.status.lock().expect("session status poisoned").phase == Phase::Unlocking
    }

    pub(super) fn is_unlocked(&self) -> bool {
        self.status.lock().expect("session status poisoned").phase == Phase::Unlocked
    }

    pub(super) fn begin_unlock(self: &Arc<Self>) -> Option<SessionToken> {
        let mut status = self.status.lock().expect("session status poisoned");
        if status.phase != Phase::Locked {
            return None;
        }
        status.generation = status
            .generation
            .checked_add(1)
            .expect("session generation exhausted");
        status.phase = Phase::Unlocking;
        Some(SessionToken {
            state: Arc::clone(self),
            generation: status.generation,
        })
    }

    pub(super) fn revoke(self: &Arc<Self>, removed: bool) -> SessionToken {
        self.grants.clear();
        let mut status = self.status.lock().expect("session status poisoned");
        status.generation = status
            .generation
            .checked_add(1)
            .expect("session generation exhausted");
        status.phase = if removed {
            Phase::Removed
        } else {
            Phase::Locking
        };
        self.collection_names.lock().unwrap().clear();
        SessionToken {
            state: Arc::clone(self),
            generation: status.generation,
        }
    }
}

impl SessionToken {
    pub fn user_id(&self) -> UserId {
        self.state.uid
    }

    pub(super) fn belongs_to(&self, state: &Arc<SessionState>) -> bool {
        Arc::ptr_eq(&self.state, state)
    }

    pub(super) fn is_current(&self) -> bool {
        let status = self.state.status.lock().expect("session status poisoned");
        status.generation == self.generation
            && !matches!(status.phase, Phase::Locking | Phase::Removed)
    }

    pub(super) fn ensure_unlocked(&self) -> Result<(), String> {
        let status = self.state.status.lock().expect("session status poisoned");
        if status.generation == self.generation && status.phase == Phase::Unlocked {
            Ok(())
        } else {
            Err(STALE_SESSION.into())
        }
    }

    pub(super) fn publish_collections(&self, names: Vec<super::Collection>) -> Result<(), String> {
        let status = self.state.status.lock().unwrap();
        if status.generation != self.generation || status.phase != Phase::Unlocked {
            return Err(STALE_SESSION.into());
        }
        *self.state.collection_names.lock().unwrap() = names;
        Ok(())
    }
    pub(super) fn advance_snapshot(&self, names: Vec<super::Collection>) -> Result<Self, String> {
        let mut status = self.state.status.lock().unwrap();
        if status.generation != self.generation || status.phase != Phase::Unlocked {
            return Err(STALE_SESSION.into());
        }
        status.generation = status
            .generation
            .checked_add(1)
            .expect("session generation exhausted");
        *self.state.collection_names.lock().unwrap() = names;
        self.state.grants.clear();
        self.state
            .sync_sequence
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Self {
            state: self.state.clone(),
            generation: status.generation,
        })
    }
    pub(super) fn ensure_unlocking(&self) -> Result<(), String> {
        let status = self.state.status.lock().expect("session status poisoned");
        if status.generation == self.generation && status.phase == Phase::Unlocking {
            Ok(())
        } else {
            Err(STALE_SESSION.into())
        }
    }

    pub(super) fn finish_unlock(&self, success: bool) -> Result<(), String> {
        let mut status = self.state.status.lock().expect("session status poisoned");
        if status.generation != self.generation || status.phase != Phase::Unlocking {
            return Err(STALE_SESSION.into());
        }
        status.phase = if success {
            Phase::Unlocked
        } else {
            Phase::Locked
        };
        Ok(())
    }

    fn generation_matches(&self) -> bool {
        self.state
            .status
            .lock()
            .expect("session status poisoned")
            .generation
            == self.generation
    }

    fn finish_lock(&self) {
        let mut status = self.state.status.lock().expect("session status poisoned");
        if status.generation == self.generation && status.phase == Phase::Locking {
            status.phase = Phase::Locked;
        }
    }
}

pub(super) fn clear_keys(client: &PasswordManagerClient) {
    client.0.internal.get_key_store().clear();
}

/// Drains leases before final key cleanup. New operations are already denied.
pub struct LockOperation {
    pub(super) session: SessionToken,
}

impl LockOperation {
    pub async fn finish(self) -> SessionToken {
        let _exclusive = self.session.state.gate.write().await;
        // A second lock may have completed before this task was polled.
        // Never clear keys belonging to a subsequent successful unlock.
        if self.session.generation_matches() {
            if let Ok(client) = self.session.state.client() {
                clear_keys(&client);
            }
            let removed = self
                .session
                .state
                .status
                .lock()
                .expect("session status poisoned")
                .phase
                == Phase::Removed;
            if removed {
                self.session.state.release_client();
            }
            self.session.finish_lock();
        }
        self.session.clone()
    }
}

impl Drop for LockOperation {
    fn drop(&mut self) {
        if self
            .session
            .state
            .status
            .lock()
            .expect("session status poisoned")
            .phase
            == Phase::Removed
        {
            self.session.state.release_client();
        }
    }
}

/// Returns an unpolled or cancelled unlock reservation to the locked state.
/// Key cleanup, if crypto has started, runs first under the exclusive lease.
pub(super) struct UnlockAttempt(pub(super) SessionToken);

impl Drop for UnlockAttempt {
    fn drop(&mut self) {
        let _ = self.0.finish_unlock(false);
    }
}

/// Clears partially initialized crypto on errors, cancellation, or revocation.
pub(super) struct UnlockCleanup {
    client: PasswordManagerClient,
    session: SessionToken,
    committed: bool,
}

impl UnlockCleanup {
    pub(super) fn new(client: &PasswordManagerClient, session: SessionToken) -> Self {
        Self {
            client: PasswordManagerClient(client.0.clone()),
            session,
            committed: false,
        }
    }

    pub(super) fn commit(&mut self) -> Result<(), String> {
        self.session.finish_unlock(true)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for UnlockCleanup {
    fn drop(&mut self) {
        if !self.committed {
            clear_keys(&self.client);
            let _ = self.session.finish_unlock(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::sdk::test_support::{install_user_key, raw_client};

    fn state() -> Arc<SessionState> {
        SessionState::new(
            "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            raw_client(),
        )
    }

    #[test]
    fn revocation_cannot_be_undone_by_a_late_unlock() {
        let state = state();
        let old = state.begin_unlock().unwrap();
        let locked = state.revoke(false);
        assert!(old.finish_unlock(true).is_err());
        assert!(!old.is_current());
        assert!(state.begin_unlock().is_none(), "cleanup must finish first");
        locked.finish_lock();
        let next = state.begin_unlock().unwrap();
        next.finish_unlock(true).unwrap();
        assert!(next.ensure_unlocked().is_ok());
        assert!(
            old.ensure_unlocked().is_err(),
            "re-unlock cannot revive old work"
        );
    }

    #[test]
    fn an_account_replacement_does_not_reuse_authority() {
        let old = state();
        let replacement = state();
        assert!(!old.token().belongs_to(&replacement));
        old.revoke(true).finish_lock();
        assert!(
            old.begin_unlock().is_none(),
            "removed accounts stay revoked"
        );
    }

    #[tokio::test]
    async fn delayed_lock_cannot_clear_a_subsequent_unlocks_keys() {
        let state = state();
        let client = state.client().unwrap();
        let stale = LockOperation {
            session: state.revoke(false),
        };
        LockOperation {
            session: state.revoke(false),
        }
        .finish()
        .await;
        let current = state.begin_unlock().unwrap();
        install_user_key(&client);
        current.finish_unlock(true).unwrap();
        stale.finish().await;
        assert!(
            client.is_unlocked(),
            "old cleanup must not clear the new keys"
        );
        assert!(state.is_unlocked());
    }

    #[tokio::test]
    async fn cancelled_unlock_clears_real_sdk_keys_before_releasing_its_lease() {
        let state = state();
        let client = state.client().unwrap();
        let session = state.begin_unlock().unwrap();
        let exclusive = state.gate.write().await;
        let cleanup = UnlockCleanup::new(&client, session);
        install_user_key(&client); // Simulate partial SDK initialization.
        assert!(client.is_unlocked());
        state.revoke(false);
        drop(cleanup); // The same drop path used by cancellation and SDK errors.
        assert!(!client.is_unlocked());
        drop(exclusive);
    }
}
