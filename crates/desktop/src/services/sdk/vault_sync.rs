//! Full sync with revision-checked existing-entry writes and atomic encrypted snapshots.
use super::{
    SessionClient, SessionToken, refresh,
    snapshot::{self, OrgRollback, Snapshot},
};
use crate::services::accounts::{
    login::AccountRecord, profile::ProfileStore, transport::json_body_limited,
};
use bitwarden_vault::{Cipher, CipherListView};
use chrono::{DateTime, Utc};
use std::{path::PathBuf, sync::atomic::Ordering};
#[derive(Clone, Debug)]
pub struct Report {
    pub session: SessionToken,
    pub downloaded_at: DateTime<Utc>,
    pub ciphers: usize,
    pub uploaded: usize,
    pub conflicts: usize,
    pub problems: Vec<String>,
}
pub async fn synchronize(
    client: SessionClient,
    root: PathBuf,
    store: ProfileStore,
) -> Result<Report, String> {
    synchronize_inner(client, root, store, true).await
}
/// Startup/unlock refresh keeps drafts, but never starts writes implicitly.
pub async fn download(
    client: SessionClient,
    root: PathBuf,
    store: ProfileStore,
) -> Result<Report, String> {
    synchronize_inner(client, root, store, false).await
}
async fn synchronize_inner(
    mut client: SessionClient,
    root: PathBuf,
    store: ProfileStore,
    upload: bool,
) -> Result<Report, String> {
    let mut session = client.session();
    let state = session.state.clone();
    let sequence = state.sync_sequence.load(Ordering::SeqCst);
    let _coordinator = state.sync_gate.lock().await;
    if session.state.sync_sequence.load(Ordering::SeqCst) != sequence {
        let current = session.state.token();
        if current.generation == session.generation + 1 && current.ensure_unlocked().is_ok() {
            if !upload
                || current
                    .state
                    .snapshot
                    .current()
                    .is_none_or(|s| s.persisted.local_edits.is_empty())
            {
                return report(current);
            }
            session = current;
            client = SessionClient {
                session: session.clone(),
            };
        }
    }
    session.ensure_unlocked()?;
    let upload_root = root.clone();
    let upload_store = store.clone();
    let fetch_session = session.clone();
    let (snapshot, record, transport) = client
        .run(move |_| async move { fetch(&fetch_session, root, store).await })
        .await?;
    let _exclusive = session.state.gate.write().await;
    session.ensure_unlocked()?;
    let client = session.state.client()?;
    let mut names = apply(&session, &client, snapshot, &record, &transport).await?;
    let drafts_before = session
        .state
        .snapshot
        .current()
        .map_or(0, |s| s.persisted.local_edits.len());
    let uploads = if upload {
        super::vault_writes::upload(&session, &client, upload_root.clone(), upload_store.clone())
            .await
    } else {
        Ok(super::vault_writes::UploadReport::default())
    };
    // Refresh after dispatch even when the reply was lost or rejected. This is
    // read-only reconciliation: it never loops back into uploads.
    let mut reconcile_error = None;
    if uploads
        .as_ref()
        .is_ok_and(|r| r.uploaded > 0 || !r.problems.is_empty())
        && session.ensure_unlocked().is_ok()
    {
        match fetch(&session, upload_root, upload_store).await {
            Ok((snapshot, record, transport)) => {
                match apply(&session, &client, snapshot, &record, &transport).await {
                    Ok(updated) => names = updated,
                    Err(error) => reconcile_error = Some(error),
                }
            }
            Err(error) => reconcile_error = Some(error),
        }
    }
    // Even a persistence/network error must invalidate old UI views and grants
    // after the fresh remote snapshot was committed.
    let new_session = session.advance_snapshot(names)?;
    let uploads = uploads?;
    let mut result = report(new_session)?;
    let remaining = result
        .session
        .state
        .snapshot
        .current()
        .map_or(0, |s| s.persisted.local_edits.len());
    result.uploaded = uploads
        .uploaded
        .max(drafts_before.saturating_sub(remaining));
    result.problems = uploads.problems;
    // An exact read-after-write match resolves an initially lost/unusable reply.
    if remaining == 0 && reconcile_error.is_none() {
        result.problems.clear();
    }
    if let Some(error) = reconcile_error {
        result.problems.push(error);
    }
    Ok(result)
}
pub(super) async fn fetch(
    session: &SessionToken,
    root: PathBuf,
    store: ProfileStore,
) -> Result<
    (
        std::sync::Arc<Snapshot>,
        AccountRecord,
        crate::services::accounts::transport::Transport,
    ),
    String,
> {
    let authority = refresh::authorized(session, root.clone(), store.clone(), false).await?;
    let mut record = authority.record;
    let mut credentials = authority.credentials;
    let mut transport = authority.transport;
    let started = Utc::now();
    let url = url::Url::parse(&record.api)
        .and_then(|u| u.join("sync?excludeDomains=true"))
        .map_err(|_| "Ungültiger Sync-Endpunkt")?;
    let mut response = transport
        .execute(
            transport
                .request(reqwest_account::Method::GET, url.clone())?
                .bearer_auth(&credentials.access_token)
                .build()
                .map_err(|_| "Sync-Anfrage konnte nicht erstellt werden")?,
        )
        .await?;
    if response.status().as_u16() == 401 {
        let renewed = refresh::authorized(session, root, store, true).await?;
        record = renewed.record;
        credentials = renewed.credentials;
        transport = renewed.transport;
        response = transport
            .execute(
                transport
                    .request(reqwest_account::Method::GET, url)?
                    .bearer_auth(&credentials.access_token)
                    .build()
                    .map_err(|_| "Sync-Anfrage konnte nicht erstellt werden")?,
            )
            .await?;
    }
    if matches!(response.status().as_u16(), 401 | 403) {
        if response.status().as_u16() == 401 {
            session
                .state
                .network_authorized
                .store(false, Ordering::SeqCst);
        }
        return Err("Die Anmeldung ist abgelaufen oder der Server verweigert den Zugriff. Bitte erneut anmelden.".into());
    }
    if !response.status().is_success() {
        return Err("Der Server konnte den Tresor nicht synchronisieren. Der bisherige Snapshot bleibt erhalten.".into());
    }
    let data = json_body_limited(response, 64 * 1024 * 1024).await?;
    let expected = record.server_user_id;
    let local_account = record.id;
    let snapshot = tokio::task::spawn_blocking(move || {
        let response = serde_json::from_value(data).map_err(
            |_| "Die Sync-Antwort ist ungültig oder enthält nicht unterstützte Datentypen",
        )?;
        Snapshot::prepare(response, expected, local_account, started)
    })
    .await
    .map_err(|_| "Sync-Antwort konnte nicht verarbeitet werden")??;
    Ok((snapshot, record, transport))
}
/// Caller holds the exclusive lease. Invalid fresh data never replaces the cache.
pub(super) async fn apply(
    session: &SessionToken,
    client: &bitwarden_pm::PasswordManagerClient,
    snapshot: std::sync::Arc<Snapshot>,
    record: &AccountRecord,
    transport: &crate::services::accounts::transport::Transport,
) -> Result<Vec<super::Collection>, String> {
    transport.check(&transport.profile.default_api()?)?;
    if let Err(error) = validate_account_keys(&snapshot, record) {
        session
            .state
            .network_authorized
            .store(false, Ordering::SeqCst);
        return Err(error);
    }
    let old = session.state.snapshot.current();
    if let (Some(before), Some(after)) = (
        old.as_ref()
            .and_then(|s| s.persisted.response.profile.as_ref())
            .and_then(|p| p.security_stamp.as_ref()),
        snapshot
            .persisted
            .response
            .profile
            .as_ref()
            .and_then(|p| p.security_stamp.as_ref()),
    ) && before != after
    {
        session
            .state
            .network_authorized
            .store(false, Ordering::SeqCst);
        return Err(
            "Die Kontosicherheit wurde auf dem Server geändert. Bitte erneut anmelden.".into(),
        );
    }
    let mut rollback = OrgRollback::new(
        client,
        session.clone(),
        old.as_ref()
            .map(|s| s.organizations.clone())
            .unwrap_or_default(),
    );
    snapshot::org_keys(client, &snapshot.organizations)?;
    // Validate every list row; the SDK's partial-success list API must not make a
    // malformed/incompletely decryptable response look like an empty remote vault.
    let ciphers: Vec<Cipher> = snapshot.ciphers.values().cloned().collect();
    let _:Vec<CipherListView>=client.0.internal.get_key_store().decrypt_list(&ciphers).map_err(|_|"Einträge konnten nicht vollständig entschlüsselt werden. Der bisherige Snapshot bleibt erhalten.")?;
    {
        use bitwarden_crypto::Decryptable;
        let keys = client.0.internal.get_key_store();
        let mut context = keys.context_mut();
        for folder in snapshot.folders.values() {
            let _: String = folder
                .name
                .decrypt(
                    &mut context,
                    bitwarden_core::key_management::SymmetricKeySlotId::User,
                )
                .map_err(|_| "Ordner konnten nicht entschlüsselt werden")?;
        }
    }
    let snapshot = super::vault_writes::reconcile(client, snapshot, old.as_deref())?;
    let names = snapshot.collection_names(client)?;
    session.ensure_unlocked()?;
    transport.check(&transport.profile.default_api()?)?;
    // Commit authorization linearizes here. An already authorized encrypted SQLite
    // transaction may finish while lock/logout drains this lease; no plaintext or
    // new session authority is published after revocation. Logout then removes it.
    client.platform().state().setting(snapshot::CACHE).map_err(|_|"Snapshot-Speicher fehlt")?.update(snapshot.persisted.clone()).await.map_err(|_|"Der neue Snapshot konnte nicht gespeichert werden. Der bisherige Snapshot bleibt erhalten.")?;
    session.state.snapshot.install(snapshot);
    rollback.committed = true;
    Ok(names)
}
fn validate_account_keys(snapshot: &Snapshot, record: &AccountRecord) -> Result<(), String> {
    let profile = snapshot
        .persisted
        .response
        .profile
        .as_ref()
        .ok_or("Kontoprofil fehlt")?;
    if profile
        .email
        .as_ref()
        .is_none_or(|email| !email.eq_ignore_ascii_case(&record.email))
    {
        return Err("Die Konto-E-Mail wurde geändert. Bitte erneut anmelden.".into());
    }
    if profile
        .key
        .as_ref()
        .is_some_and(|key| key != &record.master_unlock.master_key_wrapped_user_key.to_string())
    {
        return Err("Die Passwort-/Kontoschlüssel wurden geändert. Bitte erneut anmelden.".into());
    }
    if let Some(model) = &profile.account_keys {
        let received=bitwarden_core::key_management::account_cryptographic_state::WrappedAccountCryptographicState::try_from(model.as_ref()).map_err(|_|"Ungültige Kontoschlüssel in der Sync-Antwort")?;
        if serde_json::to_value(&received)
            .map_err(|_| "Kontoschlüssel konnten nicht geprüft werden")?
            != serde_json::to_value(&record.crypto_state)
                .map_err(|_| "Lokale Kontoschlüssel konnten nicht geprüft werden")?
        {
            return Err(
                "Der kryptografische Kontozustand wurde geändert. Bitte erneut anmelden.".into(),
            );
        }
    }
    if let Some(model) = snapshot
        .persisted
        .response
        .user_decryption
        .as_ref()
        .and_then(|u| u.master_password_unlock.as_ref())
    {
        let received =
            bitwarden_core::key_management::MasterPasswordUnlockData::try_from(model.as_ref())
                .map_err(|_| "Ungültige Passwort-Entsperrdaten in der Sync-Antwort")?;
        if serde_json::to_value(&received)
            .map_err(|_| "Entsperrdaten konnten nicht geprüft werden")?
            != serde_json::to_value(&record.master_unlock)
                .map_err(|_| "Lokale Entsperrdaten konnten nicht geprüft werden")?
        {
            return Err(
                "Die Passwort-/KDF-Einstellungen wurden geändert. Bitte erneut anmelden.".into(),
            );
        }
    }
    if let bitwarden_core::key_management::account_cryptographic_state::WrappedAccountCryptographicState::V1{private_key}=&record.crypto_state && profile.private_key.as_ref().is_some_and(|key|key!=&private_key.to_string()){return Err("Der private Kontoschlüssel wurde geändert. Bitte erneut anmelden.".into())}
    Ok(())
}
pub(super) fn report(session: SessionToken) -> Result<Report, String> {
    let snapshot = session
        .state
        .snapshot
        .current()
        .ok_or("Noch kein synchronisierter Tresor vorhanden")?;
    Ok(Report {
        session,
        downloaded_at: snapshot.persisted.downloaded_at,
        ciphers: snapshot.ciphers.len(),
        uploaded: 0,
        conflicts: snapshot
            .persisted
            .local_edits
            .values()
            .filter(|e| e.state != snapshot::LocalEditState::Pending)
            .count(),
        problems: Vec::new(),
    })
}
