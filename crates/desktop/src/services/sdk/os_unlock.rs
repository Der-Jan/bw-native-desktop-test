//! Opt-in account-bound OS-store unlock; never persist the master password.
use super::{Action, Authorization, SessionClient, UnlockData};
use crate::services::accounts::{
    login::AccountRecord,
    profile::ProfileStore,
    storage::{SecretStore, atomic_write, read_bounded},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use bitwarden_core::{UserId, key_management::crypto::InitUserCryptoMethod};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredKey {
    version: u8,
    account: UserId,
    server_account: UserId,
    binding: Vec<u8>,
    key: String,
}
impl Drop for StoredKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}
fn record(store: &ProfileStore, uid: UserId) -> Result<AccountRecord, String> {
    let record: AccountRecord = serde_json::from_slice(&read_bounded(
        &store.root().join("accounts").join(format!("{uid}.json")),
        2 * 1024 * 1024,
    )?)
    .map_err(|_| "Invalid account record")?;
    if record.id != uid {
        return Err("OS unlock account mismatch".into());
    }
    Ok(record)
}
fn binding(record: &AccountRecord) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(&(
        record.id,
        record.server_user_id,
        &record.api,
        &record.identity,
        &record.crypto_state,
        &record.master_unlock,
    ))
    .map_err(|_| "Could not bind OS unlock to account")?;
    openssl::hash::hash(openssl::hash::MessageDigest::sha256(), &bytes)
        .map(|v| v.to_vec())
        .map_err(|_| "Could not bind OS unlock to account".into())
}
fn write(store: &ProfileStore, record: &AccountRecord) -> Result<(), String> {
    atomic_write(
        &store.root().join("accounts"),
        &format!("{}.json", record.id),
        &serde_json::to_vec(record).map_err(|_| "Could not encode OS unlock settings")?,
    )
    .map_err(|_| "Could not save OS unlock settings".into())
}
struct PendingKey {
    store: SecretStore,
    id: Uuid,
    committed: bool,
}
impl Drop for PendingKey {
    fn drop(&mut self) {
        if !self.committed {
            let store = self.store.clone();
            let id = self.id;
            // Also cleans a blocking worker's result when its async waiter is cancelled.
            let _ = std::thread::Builder::new()
                .name("os-unlock-cleanup".into())
                .spawn(move || {
                    let _ = store.delete(id);
                });
        }
    }
}
impl SessionClient {
    pub async fn configure_os_unlock(
        self,
        store: ProfileStore,
        enabled: bool,
    ) -> Result<Option<Uuid>, String> {
        let session = self.session();
        let _coordinator = session.state.sync_gate.lock().await;
        let _lease = session.state.gate.read().await;
        session.ensure_unlocked()?;
        let mut record = record(&store, session.user_id())?;
        if enabled {
            let deadline = session
                .state
                .grants
                .take(
                    &Authorization {
                        cipher: None,
                        revision: None,
                        action: Action::OsUnlock,
                    },
                    session.generation,
                )
                .ok_or(super::authorization::REPROMPT_REQUIRED)?;
            if record.os_unlock.is_some() {
                return Err("OS credential-store unlock is already enabled".into());
            }
            let client = session.state.client()?;
            let key = Zeroizing::new(
                client
                    .crypto()
                    .get_user_encryption_key()
                    .await
                    .map_err(|_| "Could not obtain vault unlock key")?
                    .into_bytes(),
            );
            let payload = StoredKey {
                version: 1,
                account: record.id,
                server_account: record.server_user_id,
                binding: binding(&record)?,
                key: STANDARD.encode(&*key),
            };
            let bytes = Zeroizing::new(
                serde_json::to_vec(&payload).map_err(|_| "Could not protect vault unlock key")?,
            );
            drop(payload);
            drop(key);
            let secrets = store.secrets.clone();
            let mut pending = tokio::task::spawn_blocking(move || -> Result<PendingKey, String> {
                let id = secrets.put(&bytes)?;
                Ok(PendingKey {
                    store: secrets,
                    id,
                    committed: false,
                })
            })
            .await
            .map_err(|_| "OS credential store task failed")??;
            session.ensure_unlocked()?;
            if std::time::Instant::now() >= deadline {
                return Err(super::authorization::REPROMPT_REQUIRED.into());
            }
            record.os_unlock = Some(pending.id);
            write(&store, &record)?;
            if let Err(error) = session.ensure_unlocked() {
                record.os_unlock = None;
                let _ = write(&store, &record);
                return Err(error);
            }
            pending.committed = true;
            Ok(Some(pending.id))
        } else {
            if let Some(id) = record.os_unlock {
                let secrets = store.secrets.clone();
                tokio::task::spawn_blocking(move || secrets.delete(id))
                    .await
                    .map_err(|_| "OS credential store task failed")??;
            }
            record.os_unlock = None;
            write(&store, &record)?;
            Ok(None)
        }
    }
}
impl UnlockData {
    pub async fn unlock_from_os_store(
        self,
        store: ProfileStore,
        expected: Uuid,
    ) -> Result<(), String> {
        self.session.ensure_unlocking()?;
        let uid = self.session.user_id();
        let mut payload=tokio::task::spawn_blocking(move || ->Result<StoredKey,String> {
            let record=record(&store,uid)?;
            if record.os_unlock!=Some(expected) {return Err("OS credential-store unlock is disabled or changed".into());}
            let bytes=store.secrets.get(expected)?;
            let payload:StoredKey=serde_json::from_slice(&bytes).map_err(|_|"Invalid OS-store unlock data")?;
            if payload.version!=1 || payload.account!=uid || payload.server_account!=record.server_user_id || payload.binding!=binding(&record)? {
                return Err("OS-store unlock data no longer matches this account. Unlock with the master password and enable it again.".into());
            }
            Ok(payload)
        }).await.map_err(|_|"OS credential store task failed")??;
        if payload.server_account != self.sdk_user_id {
            return Err("OS unlock server account mismatch".into());
        }
        self.session.ensure_unlocking()?;
        let key = std::mem::take(&mut payload.key);
        self.unlock_method(InitUserCryptoMethod::DecryptedKey {
            decrypted_user_key: key,
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn os_unlock_metadata_completion_handles_lock_without_touching_a_replacement_runtime() {
        use crate::services::sdk::test_support::unlocked_manager;
        let uid = UserId::new_v4();
        let mut manager = unlocked_manager(uid);
        let old = manager.client_for(&uid).unwrap().session();
        manager.lock(&uid).unwrap().finish().await;
        let reference = Uuid::new_v4();
        manager.set_os_unlock_ref(&old, Some(reference));
        assert_eq!(manager.os_unlock_ref(&uid), Some(reference));
        assert!(!manager.is_unlocked(&uid));
        manager.log_out(&uid).unwrap().finish().await.unwrap();
        let mut replacement = unlocked_manager(uid);
        let current = replacement.client_for(&uid).unwrap().session();
        let reference = Uuid::new_v4();
        replacement.set_os_unlock_ref(&current, Some(reference));
        replacement.set_os_unlock_ref(&old, None);
        assert_eq!(replacement.os_unlock_ref(&uid), Some(reference));
    }
    #[test]
    fn os_unlock_cancelled_registration_destroys_the_unpublished_key() {
        let root = std::env::temp_dir().join(format!("bw-os-unlock-cancel-{}", Uuid::new_v4()));
        let keys = std::sync::Arc::new(
            crate::services::accounts::storage::test_support::MemoryKeys::default(),
        );
        let store = SecretStore::with_keys(&root, keys.clone());
        let id = store.put(b"synthetic unlock payload").unwrap();
        drop(PendingKey {
            store: store.clone(),
            id,
            committed: false,
        });
        for _ in 0..2000 {
            if !root.join("secrets").join(format!("{id}.json")).exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(store.get(id).is_err());
        assert!(keys.0.lock().unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
