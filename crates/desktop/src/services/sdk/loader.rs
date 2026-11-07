//! Mock data loader: parses `mock.json`, opens per-user SQLite handles, and
//! assembles the `UserEntry` records the [`super::ClientManager`] holds.
//!
//! Plus the `MemoryRepo` shim used to register `LocalUserDataKeyState` (which
//! `initialize_user_crypto` writes to but isn't part of the SDK-managed
//! migration list).

use std::{
    collections::HashMap,
    io::BufReader,
    path::Path,
    str::FromStr,
    sync::{Arc, Mutex},
};

use bitwarden_core::{UserId, key_management::LocalUserDataKeyState};
use bitwarden_crypto::{EncString, Kdf};
use bitwarden_pm::PasswordManagerClient;
use bitwarden_state::{
    DatabaseConfiguration,
    registry::StateRegistry,
    repository::{Repository, RepositoryError, RepositoryItem},
};
use serde::Deserialize;

use crate::{domain::UnlockMethods, paths::data_dir};

use super::{
    types::{Collection, Organization},
    user_entry::UserEntry,
};

// ── mock.json schema ───────────────────────────────────────────────────────
// Mirrored in `tools/fake-data/src/main.rs`. Keep field names + types in sync.

#[derive(Deserialize)]
struct MockVaultMeta {
    users: Vec<MockUserMeta>,
}

#[derive(Deserialize, Clone)]
struct MockUserMeta {
    user_id: UserId,
    email: String,
    display_name: String,
    server_url: String,
    #[expect(dead_code)] // Read by debug helpers, not by the runtime unlock path.
    master_password_dev_only: String,
    unlock_methods: UnlockMethodsCfg,
    kdf: Kdf,
    encrypted_user_key: EncString,
    private_key: EncString,
    #[serde(default)]
    organizations: Vec<Organization>,
    #[serde(default)]
    collections: Vec<Collection>,
}

#[derive(Deserialize, Clone)]
struct UnlockMethodsCfg {
    master_password: bool,
    pin: bool,
    biometrics: bool,
}

pub async fn load_users(
    removed: &std::collections::HashSet<UserId>,
) -> Result<HashMap<UserId, Box<UserEntry>>, String> {
    if !crate::paths::is_demo() {
        // Real account persistence will be supplied by the login implementation.
        return Ok(HashMap::new());
    }
    let data_dir = data_dir();
    let meta_path = data_dir.join("mock.json");
    let file = std::fs::File::open(&meta_path).map_err(|e| e.to_string())?;
    let meta: MockVaultMeta =
        serde_json::from_reader(BufReader::new(file)).map_err(|e| e.to_string())?;

    let meta_by_id: HashMap<UserId, MockUserMeta> =
        meta.users.into_iter().map(|u| (u.user_id, u)).collect();

    let mut users = HashMap::with_capacity(meta_by_id.len());
    let entries = match std::fs::read_dir(&data_dir) {
        Ok(entries) => entries,
        Err(err) => {
            tracing::error!(path = %data_dir.display(), %err, "failed to read data dir");
            return Err(err.to_string());
        }
    };
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(err) => {
                tracing::warn!(%err, "skipping unreadable directory entry");
                continue;
            }
        };
        if path.extension().and_then(|s| s.to_str()) != Some("sqlite") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(uid) = UserId::from_str(stem) else {
            tracing::warn!(file = %path.display(), "skipping sqlite file with non-UUID name");
            continue;
        };
        if removed.contains(&uid) {
            continue;
        }
        let Some(mu) = meta_by_id.get(&uid).cloned() else {
            tracing::warn!(user_id = %uid, "sqlite file has no entry in mock.json, skipping");
            continue;
        };
        tracing::debug!(user_id = %uid, email = %mu.email, "loading mock user");
        users.insert(uid, Box::new(build_user_entry(mu, &data_dir).await?));
    }

    tracing::info!(users = users.len(), "ClientManager loaded");
    Ok(users)
}

async fn build_user_entry(mu: MockUserMeta, data_dir: &Path) -> Result<UserEntry, String> {
    // Build the registry directly (rather than via `client.platform().state()`)
    // so we can fully prepare it before constructing the client. The persisted
    // state (user_id, urls, wrapped crypto state) was written into this SQLite
    // by `cargo run -p fake-data`, so `load_from_state` will find it and
    // rebuild a locked client bound to the right user + URLs.
    let registry = StateRegistry::new_with_db(
        DatabaseConfiguration::Sqlite {
            db_name: mu.user_id.to_string(),
            folder_path: data_dir.to_path_buf(),
        },
        bitwarden_pm::migrations::get_sdk_managed_migrations(),
    )
    .await
    .map_err(|e| e.to_string())?;

    // `LocalUserDataKeyState` isn't in `get_sdk_managed_migrations()` but
    // `initialize_user_crypto` writes to it during unlock — register an empty
    // in-memory repo so unlock doesn't fail. Everything else comes from the
    // SDK-managed SQLite DB.
    register_empty_repo::<LocalUserDataKeyState>(&registry);

    let token_handler =
        Arc::new(bitwarden_auth::token_management::PasswordManagerTokenHandler::default());
    let client = PasswordManagerClient::load_from_state(token_handler, registry)
        .await
        .map_err(|e| e.to_string())?;

    Ok(UserEntry {
        session: super::session::SessionState::new(mu.user_id, client),
        email: mu.email.clone(),
        display_name: mu.display_name,
        server_url: mu.server_url,
        unlock_methods: UnlockMethods {
            master_password: mu.unlock_methods.master_password,
            pin: mu.unlock_methods.pin,
            biometrics: mu.unlock_methods.biometrics,
        },
        server_user_id: mu.user_id,
        master_unlock: bitwarden_core::key_management::MasterPasswordUnlockData { kdf:mu.kdf, salt:mu.email.clone(),master_key_wrapped_user_key:mu.encrypted_user_key,contained_key_id:None },
        crypto_state: bitwarden_core::key_management::account_cryptographic_state::WrappedAccountCryptographicState::V1{private_key:mu.private_key},
        profile:None,
        organizations: mu.organizations,
        collections: mu.collections,
    })
}

pub(super) fn register_empty_repo<T: RepositoryItem + Clone>(registry: &StateRegistry) {
    let repo = Arc::new(MemoryRepo::<T> {
        data: Mutex::new(HashMap::new()),
    });
    registry.register_client_managed(repo);
}

// ── In-memory repository ───────────────────────────────────────────────────
// Kept only for `LocalUserDataKeyState`, which isn't part of the SDK-managed
// migration list but is written to during crypto init.

struct MemoryRepo<T: RepositoryItem + Clone> {
    data: Mutex<HashMap<String, T>>,
}

#[async_trait::async_trait]
impl<T: RepositoryItem + Clone> Repository<T> for MemoryRepo<T> {
    async fn get(&self, key: T::Key) -> Result<Option<T>, RepositoryError> {
        Ok(self
            .data
            .lock()
            .expect("MemoryRepo mutex poisoned")
            .get(&key.to_string())
            .cloned())
    }
    async fn list(&self) -> Result<Vec<T>, RepositoryError> {
        Ok(self
            .data
            .lock()
            .expect("MemoryRepo mutex poisoned")
            .values()
            .cloned()
            .collect())
    }
    async fn set(&self, key: T::Key, value: T) -> Result<(), RepositoryError> {
        self.data
            .lock()
            .expect("MemoryRepo mutex poisoned")
            .insert(key.to_string(), value);
        Ok(())
    }
    async fn set_bulk(&self, values: Vec<(T::Key, T)>) -> Result<(), RepositoryError> {
        let mut map = self.data.lock().expect("MemoryRepo mutex poisoned");
        for (k, v) in values {
            map.insert(k.to_string(), v);
        }
        Ok(())
    }
    async fn remove(&self, key: T::Key) -> Result<(), RepositoryError> {
        self.data
            .lock()
            .expect("MemoryRepo mutex poisoned")
            .remove(&key.to_string());
        Ok(())
    }
    async fn remove_bulk(&self, keys: Vec<T::Key>) -> Result<(), RepositoryError> {
        let mut map = self.data.lock().expect("MemoryRepo mutex poisoned");
        for k in keys {
            map.remove(&k.to_string());
        }
        Ok(())
    }
    async fn remove_all(&self) -> Result<(), RepositoryError> {
        self.data.lock().expect("MemoryRepo mutex poisoned").clear();
        Ok(())
    }
}
