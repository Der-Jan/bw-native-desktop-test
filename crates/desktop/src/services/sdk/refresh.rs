//! Serialized token refresh. Persist rotated credentials before using the new access token.
use crate::services::accounts::{
    login::{AccountRecord, Credentials},
    profile::ProfileStore,
    storage::{atomic_write, read_bounded},
    transport::{Transport, json_body},
};
use crate::{domain::UserId, services::sdk::SessionClient};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};
use zeroize::Zeroizing;
fn gate(id: UserId) -> Arc<tokio::sync::Mutex<()>> {
    static GATES: OnceLock<Mutex<HashMap<UserId, Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    GATES
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .entry(id)
        .or_default()
        .clone()
}

pub(super) struct Authorized {
    pub record: AccountRecord,
    pub credentials: Credentials,
    pub transport: Transport,
}
/// Caller must hold a session lease; this serializes refresh and durable rotation.
pub(super) async fn authorized(
    session: &super::SessionToken,
    root: PathBuf,
    store: ProfileStore,
    force: bool,
) -> Result<Authorized, String> {
    authorized_inner(session, root, store, force, false).await
}
pub(super) async fn authorized_notifications(
    session: &super::SessionToken,
    root: PathBuf,
    store: ProfileStore,
    force: bool,
) -> Result<Authorized, String> {
    authorized_inner(session, root, store, force, true).await
}
async fn authorized_inner(
    session: &super::SessionToken,
    root: PathBuf,
    store: ProfileStore,
    force: bool,
    notifications: bool,
) -> Result<Authorized, String> {
    let id = session.user_id();
    let gate = gate(id);
    let _guard = gate.lock().await;
    session.ensure_unlocked()?;
    if !session
        .state
        .network_authorized
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err("Die Serversitzung ist abgelaufen. Bitte das Konto erneut anmelden; der gespeicherte Tresor bleibt offline zugänglich.".into());
    }
    let load_root = root.clone();
    let load_store = store.clone();
    let (mut record, mut credentials, transport) = tokio::task::spawn_blocking(move || {
        let record: AccountRecord = serde_json::from_slice(&read_bounded(
            &load_root.join("accounts").join(format!("{id}.json")),
            2 * 1024 * 1024,
        )?)
        .map_err(|_| "Invalid account record")?;
        if record.id != id {
            return Err("Account ID mismatch".into());
        }
        let credentials: Credentials =
            serde_json::from_slice(&load_store.secrets.get(record.credentials)?)
                .map_err(|_| "Invalid protected credentials")?;
        let profile = load_store
            .get(record.profile)
            .ok_or("Server profile unavailable")?;
        let transport = if notifications {
            Transport::notifications(&load_store, profile)?
        } else {
            Transport::new(&load_store, profile)?
        };
        Ok::<_, String>((record, credentials, transport))
    })
    .await
    .map_err(|_| "Could not load protected credentials")??;
    session.ensure_unlocked()?;
    if force || credentials.expires_at <= chrono::Utc::now().timestamp() + 60 {
        let refresh = credentials
            .refresh_token
            .as_ref()
            .ok_or("Session expired; sign in again")?;
        let url = url::Url::parse(&record.identity)
            .and_then(|u| u.join("connect/token"))
            .map_err(|_| "Invalid identity endpoint")?;
        let request = transport
            .request(reqwest_account::Method::POST, url)?
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh.as_str()),
                ("client_id", "desktop"),
            ])
            .build()
            .map_err(|_| "Could not prepare session refresh")?;
        let response = transport.execute(request).await?;
        if !response.status().is_success() {
            if matches!(response.status().as_u16(), 400 | 401) {
                session
                    .state
                    .network_authorized
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                return Err("Die Serversitzung ist abgelaufen. Bitte erneut anmelden.".into());
            }
            return Err(
                "Der Server konnte die Sitzung nicht erneuern. Bitte später erneut versuchen."
                    .into(),
            );
        }
        let value = json_body(response).await?;
        let access = value
            .get("access_token")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or("Invalid refreshed session")?
            .to_owned();
        let refresh = value
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .or_else(|| credentials.refresh_token.clone());
        let expiry = value
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .filter(|n| *n > 0 && *n <= 365 * 86400)
            .ok_or("Invalid refreshed session lifetime")?;
        credentials = Credentials {
            access_token: access,
            refresh_token: refresh,
            expires_at: chrono::Utc::now().timestamp() + expiry as i64,
            device: credentials.device,
        };
        let bytes = Zeroizing::new(
            serde_json::to_vec(&credentials).map_err(|_| "Could not protect refreshed session")?,
        );
        let old = record.credentials;
        let save_store = store.clone();
        let save_root = root.clone();
        let mut next = record.clone();
        session.ensure_unlocked()?;
        record = tokio::task::spawn_blocking(move || {
            let new = save_store.secrets.put(&bytes)?;
            next.credentials = new;
            let result = atomic_write(
                &save_root.join("accounts"),
                &format!("{id}.json"),
                &serde_json::to_vec(&next).map_err(|_| "Could not encode refreshed account")?,
            );
            if result.is_err() {
                let _ = save_store.secrets.delete(new);
                return Err("Could not persist refreshed session".into());
            }
            // Orphan collection retries a failed old-key deletion on restart.
            save_store.secrets.delete(old)?;
            Ok::<_, String>(next)
        })
        .await
        .map_err(|_| "Could not persist refreshed session")??;
    }
    session.ensure_unlocked()?;
    Ok(Authorized {
        record,
        credentials,
        transport,
    })
}
pub async fn probe(
    client: SessionClient,
    root: PathBuf,
    store: ProfileStore,
) -> Result<(), String> {
    let session = client.session();
    client
        .run(move |_| async move {
            let Authorized {
                record,
                credentials,
                transport,
            } = authorized(&session, root, store, false).await?;
            session.ensure_unlocked()?;
            let url = url::Url::parse(&record.api)
                .and_then(|u| u.join("accounts/profile"))
                .map_err(|_| "Invalid profile endpoint")?;
            let request = transport
                .request(reqwest_account::Method::GET, url)?
                .bearer_auth(&credentials.access_token)
                .build()
                .map_err(|_| "Could not build account probe")?;
            let response = transport.execute(request).await?;
            if !response.status().is_success() {
                return Err("Account verification failed; sign in again".into());
            }
            let data = json_body(response).await?;
            if data
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<UserId>().ok())
                != Some(record.server_user_id)
            {
                return Err("Account verification returned a different account".into());
            }
            Ok(())
        })
        .await
}
