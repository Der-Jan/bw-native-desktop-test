//! Real accounts with an encrypted read-sync cache. OAuth tokens never enter SDK SQLite settings.
use super::{session::SessionState, unlock::UnlockData, user_entry::UserEntry};
use crate::{
    domain::UnlockMethods,
    services::accounts::{
        login::{AccountRecord, Attempt, LoginSuccess},
        profile::ProfileStore,
        storage::{SecretStore, atomic_write, read_bounded, sync_dir},
        transport::Transport,
    },
};
use bitwarden_core::{
    ClientBuilder, ClientSettings, UserId, key_management::LocalUserDataKeyState,
};
use bitwarden_pm::PasswordManagerClient;
use bitwarden_state::{DatabaseConfiguration, registry::StateRegistry};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;
use zeroize::Zeroizing;

pub struct Candidate {
    pub record: AccountRecord,
    entry: Option<Box<UserEntry>>,
    root: PathBuf,
    secrets: SecretStore,
    published: bool,
    source_transport: Transport,
}
impl std::fmt::Debug for Candidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuthenticatedCandidate(<hidden>)")
    }
}
impl Candidate {
    /// Called synchronously only after the UI has accepted the matching attempt.
    /// No keyring or network work occurs in this short atomic publication step.
    pub(super) fn publish(mut self, attempt: &Attempt) -> Result<(UserId, Box<UserEntry>), String> {
        attempt.check()?;
        self.source_transport
            .check(&self.source_transport.profile.default_api()?)?;
        let bytes =
            serde_json::to_vec(&self.record).map_err(|_| "Could not encode account state")?;
        atomic_write(
            &self.root.join("accounts"),
            &format!("{}.json", self.record.id),
            &bytes,
        )
        .map_err(|_| "Could not publish local account state")?;
        self.published = true;
        let _ = std::fs::remove_file(
            self.root
                .join(format!(".pending-account-{}.json", self.record.id)),
        );
        Ok((
            self.record.id,
            self.entry
                .take()
                .ok_or("Account candidate was already consumed")?,
        ))
    }
}
impl Drop for Candidate {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        if let Some(entry) = self.entry.take() {
            entry.session.revoke(true);
            if let Ok(client) = entry.session.client() {
                super::session::clear_keys(&client);
            }
            entry.session.release_client();
        }
        let root = self.root.clone();
        let record = self.record.clone();
        let secrets = self.secrets.clone();
        // The retained staging record is also cleaned before account loading after a crash.
        let _ = std::thread::Builder::new()
            .name("cancelled-login-cleanup".into())
            .spawn(move || {
                if secrets.delete(record.credentials).is_ok() {
                    if remove_database(&root, record.id).is_err() {
                        return;
                    }
                    let _ = std::fs::remove_file(
                        root.join(format!(".pending-account-{}.json", record.id)),
                    );
                }
            });
    }
}

pub async fn prepare(
    root: PathBuf,
    store: ProfileStore,
    attempt: Attempt,
    success: LoginSuccess,
) -> Result<Candidate, String> {
    attempt.check()?;
    let id = UserId::new(Uuid::new_v4());
    // Detach account certificate settings from the prelogin profile, including on the same server.
    let source = success.transport.profile.clone();
    success.transport.check(&success.api)?;
    let clone_store = store.clone();
    let profile = tokio::task::spawn_blocking(move || clone_store.clone_for_account(&source))
        .await
        .map_err(|_| "Could not prepare account server settings")??;
    let transport = success.transport.rebind(&store, profile.clone())?;
    let secret_store = store.secrets.clone();
    let serialized = Zeroizing::new(
        serde_json::to_vec(&success.credentials).map_err(|_| "Could not encode credentials")?,
    );
    let secrets_for_write = secret_store.clone();
    let credential_ref = tokio::task::spawn_blocking(move || secrets_for_write.put(&serialized))
        .await
        .map_err(|_| "Credential storage failed")??;
    let record = AccountRecord {
        id,
        server_user_id: success.server_user_id,
        email: success.email.clone(),
        display_name: success.display_name,
        profile: profile.id,
        api: success.api.to_string(),
        identity: success.identity.to_string(),
        master_unlock: success.master_unlock,
        crypto_state: success.crypto_state,
        credentials: credential_ref,
        os_unlock: None,
    };
    let mut candidate = Candidate {
        record,
        entry: None,
        root: root.clone(),
        secrets: secret_store,
        published: false,
        source_transport: success.transport.clone(),
    };
    let bytes =
        serde_json::to_vec(&candidate.record).map_err(|_| "Could not encode pending account")?;
    let stage_root = root.clone();
    let filename = format!(".pending-account-{id}.json");
    tokio::task::spawn_blocking(move || atomic_write(&stage_root, &filename, &bytes))
        .await
        .map_err(|_| "Could not stage account state")?
        .map_err(|_| "Could not stage account state")?;
    attempt.check()?;
    let entry = build_entry(&root, candidate.record.clone(), Some(transport)).await?;
    let session = entry
        .session
        .begin_unlock()
        .ok_or("Cannot reserve account unlock")?;
    UnlockData {
        _attempt: super::session::UnlockAttempt(session.clone()),
        session,
        sdk_user_id: entry.server_user_id,
        master_unlock: entry.master_unlock.clone(),
        crypto_state: entry.crypto_state.clone(),
        email: entry.email.clone(),
        organizations: Vec::new(),
    }
    .unlock(success.password.to_string())
    .await?;
    attempt.check()?;
    candidate.entry = Some(Box::new(entry));
    Ok(candidate)
}

async fn build_entry(
    root: &Path,
    record: AccountRecord,
    transport: Option<Transport>,
) -> Result<UserEntry, String> {
    let path = root.join(format!("{}.sqlite", record.id));
    if !path.exists() {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        opts.open(&path)
            .map_err(|_| "Could not create private account database")?;
    } else if std::fs::symlink_metadata(&path)
        .map_err(|_| "Account database unavailable")?
        .file_type()
        .is_symlink()
    {
        return Err("Account database must not be a symbolic link".into());
    }
    let registry = StateRegistry::new_with_db(
        DatabaseConfiguration::Sqlite {
            db_name: record.id.to_string(),
            folder_path: root.to_path_buf(),
        },
        bitwarden_pm::migrations::get_sdk_managed_migrations(),
    )
    .await
    .map_err(|_| "Could not open account database")?;
    super::loader::register_empty_repo::<LocalUserDataKeyState>(&registry);
    // The committed snapshot is the only durable source of org memberships/keys.
    super::loader::register_empty_repo::<
        bitwarden_core::client::persisted_state::OrganizationSharedKey,
    >(&registry);
    let snapshot = super::snapshot::register(&registry, record.server_user_id, record.id).await?;
    let settings = ClientSettings {
        api_url: record.api.clone(),
        identity_url: record.identity.clone(),
        user_agent: "Bitwarden Native Desktop".into(),
        device_type: if cfg!(target_os = "windows") {
            bitwarden_core::DeviceType::WindowsDesktop
        } else if cfg!(target_os = "macos") {
            bitwarden_core::DeviceType::MacOsDesktop
        } else {
            bitwarden_core::DeviceType::LinuxDesktop
        },
        ..Default::default()
    };
    let mut builder = ClientBuilder::new()
        .with_settings(settings)
        .with_state(registry);
    if let Some(transport) = transport {
        builder = transport.configure_sdk(builder);
    } else {
        // Offline crypto remains usable when certificate/keyring recovery is required.
        // Every SDK API/Identity request is denied, never sent without the bound certificate.
        let client = reqwest_account::Client::builder()
            .redirect(reqwest_account::redirect::Policy::none())
            .build()
            .map_err(|_| "Could not prepare offline account")?;
        builder = builder.with_account_transport(client, Arc::new(OfflineOnly));
    }
    let client = PasswordManagerClient(builder.build());
    // A failed field decryption must never become an empty value in a saved edit.
    // This client supports Vaultwarden; no server rollout flag may weaken writes.
    client
        .flags()
        .load(std::collections::HashMap::from([(
            "strict-cipher-decryption".into(),
            true,
        )]))
        .await;
    let mut session = SessionState::new(record.id, client);
    Arc::get_mut(&mut session)
        .expect("new session is unique")
        .snapshot = snapshot;
    session
        .writable
        .store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(UserEntry {
        session,
        os_unlock: record.os_unlock,
        server_user_id: record.server_user_id,
        email: record.email,
        display_name: record.display_name,
        server_url: record.api,
        unlock_methods: UnlockMethods {
            master_password: true,
            pin: false,
            biometrics: false,
        },
        master_unlock: record.master_unlock,
        crypto_state: record.crypto_state,
        profile: Some(record.profile),
        organizations: Vec::new(),
        collections: Vec::new(),
    })
}
struct OfflineOnly;
#[async_trait::async_trait]
impl reqwest_middleware::Middleware for OfflineOnly {
    async fn handle(
        &self,
        _req: reqwest_account::Request,
        _ext: &mut http::Extensions,
        _next: reqwest_middleware::Next<'_>,
    ) -> Result<reqwest_account::Response, reqwest_middleware::Error> {
        Err(reqwest_middleware::Error::Middleware(
            std::io::Error::other(
                "Account transport unavailable; recover its certificate or credential store first",
            )
            .into(),
        ))
    }
}

pub(super) async fn load(
    root: &Path,
    store: &ProfileStore,
    removed: &HashSet<UserId>,
) -> Result<HashMap<UserId, Box<UserEntry>>, String> {
    let account_dir = root.join("accounts");
    let mut users = HashMap::new();
    let entries = match std::fs::read_dir(&account_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(users),
        Err(_) => return Err("Account records are unavailable".into()),
    };
    for entry in entries {
        let path = entry.map_err(|_| "Account records are unavailable")?.path();
        if path.extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }
        let record: AccountRecord = serde_json::from_slice(&read_bounded(&path, 2 * 1024 * 1024)?)
            .map_err(|_| "Invalid local account record")?;
        if path.file_stem().and_then(|v| v.to_str()) != Some(record.id.to_string().as_str()) {
            return Err("Account record ID does not match its filename".into());
        }
        if removed.contains(&record.id) {
            continue;
        }
        let profile = store
            .get(record.profile)
            .ok_or("The account server profile is missing")?;
        let source_store = store.clone();
        let transport = tokio::task::spawn_blocking(move || Transport::new(&source_store, profile))
            .await
            .map_err(|_| "Account transport unavailable")?
            .ok();
        let id = record.id;
        users.insert(id, Box::new(build_entry(root, record, transport).await?));
    }
    Ok(users)
}
pub fn remove_record(root: &Path, id: UserId) -> Result<(), String> {
    remove_record_with_secrets(root, id, &SecretStore::system(root))
}
pub(crate) fn remove_record_with_secrets(
    root: &Path,
    id: UserId,
    secrets: &SecretStore,
) -> Result<(), String> {
    let path = root.join("accounts").join(format!("{id}.json"));
    if !path.exists() {
        return Ok(());
    }
    let record: AccountRecord = serde_json::from_slice(&read_bounded(&path, 2 * 1024 * 1024)?)
        .map_err(|_| "Invalid account record during removal")?;
    if record.id != id {
        return Err("Account removal ID mismatch".into());
    }
    if let Some(unlock) = record.os_unlock {
        secrets.delete(unlock)?;
    }
    secrets.delete(record.credentials)?;
    std::fs::remove_file(&path).map_err(|_| "Could not remove account metadata")?;
    sync_dir(&root.join("accounts")).map_err(|_| "Could not commit account removal".into())
}
pub fn recover_pending(root: &Path, secrets: &SecretStore) -> Result<(), String> {
    for entry in std::fs::read_dir(root).map_err(|_| "Account storage unavailable")? {
        let path = entry.map_err(|_| "Account storage unavailable")?.path();
        if !path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with(".pending-account-") && s.ends_with(".json"))
        {
            continue;
        }
        let record: AccountRecord = serde_json::from_slice(&read_bounded(&path, 2 * 1024 * 1024)?)
            .map_err(|_| "Invalid staged account record")?;
        if path.file_name().and_then(|s| s.to_str())
            != Some(format!(".pending-account-{}.json", record.id).as_str())
        {
            return Err("Staged account filename mismatch".into());
        }
        // A completed publication may have crashed before removing its stage receipt.
        if !root
            .join("accounts")
            .join(format!("{}.json", record.id))
            .exists()
        {
            secrets.delete(record.credentials)?;
            remove_database(root, record.id)?;
        }
        std::fs::remove_file(path).map_err(|_| "Could not clear staged account")?;
    }
    Ok(())
}
fn remove_database(root: &Path, id: UserId) -> Result<(), String> {
    for suffix in [".sqlite", ".sqlite-wal", ".sqlite-shm", ".sqlite-journal"] {
        match std::fs::remove_file(root.join(format!("{id}{suffix}"))) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("Could not remove staged account database".into()),
        }
    }
    sync_dir(root).map_err(|_| "Could not commit staged account removal".into())
}

/// Runs once before loading accounts, never concurrently with imports/login publication.
pub fn prune_orphans(root: &Path, store: &ProfileStore) -> Result<(), String> {
    let mut profiles = std::collections::HashSet::new();
    let mut secrets = std::collections::HashSet::new();
    let entries = match std::fs::read_dir(root.join("accounts")) {
        Ok(e) => Some(e),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err("Account storage unavailable".into()),
    };
    if let Some(entries) = entries {
        for entry in entries {
            let path = entry.map_err(|_| "Account storage unavailable")?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let record: AccountRecord =
                serde_json::from_slice(&read_bounded(&path, 2 * 1024 * 1024)?)
                    .map_err(|_| "Invalid account record")?;
            profiles.insert(record.profile);
            secrets.insert(record.credentials);
            if let Some(unlock) = record.os_unlock {
                secrets.insert(unlock);
            }
        }
    }
    for profile in store.list() {
        if profile.account_bound && !profiles.contains(&profile.id) {
            store.remove(profile.id)?;
        } else if let Some(cert) = profile.certificate {
            secrets.insert(cert.secret);
        }
    }
    store.secrets.prune(&secrets)
}

#[cfg(test)]
pub(crate) async fn test_load(root: &Path, store: &ProfileStore) -> super::ClientManager {
    let mut manager = super::ClientManager::empty();
    manager.users = load(root, store, &HashSet::new()).await.unwrap();
    manager.profiles = Some(store.clone());
    manager
}
