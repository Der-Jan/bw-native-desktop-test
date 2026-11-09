//! Online item lifecycle changes with durable intent and read-only recovery.
use super::{
    Action, SessionClient, SessionToken, local_edits, refresh, snapshot::Snapshot, vault_sync,
};
use crate::services::accounts::profile::ProfileStore;
use bitwarden_pm::PasswordManagerClient;
use bitwarden_vault::{Cipher, CipherId};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::atomic::Ordering};
#[derive(Clone, Copy)]
pub enum Change {
    Archive(bool),
    Trash(bool),
}
impl Change {
    fn action(self) -> Action {
        match self {
            Self::Archive(_) => Action::Archive,
            Self::Trash(true) => Action::Delete,
            Self::Trash(false) => Action::Restore,
        }
    }
    fn matches(self, cipher: &Cipher) -> bool {
        match self {
            Self::Archive(value) => cipher.archived_date.is_some() == value,
            Self::Trash(value) => cipher.deleted_date.is_some() == value,
        }
    }
    fn endpoint(self) -> &'static str {
        match self {
            Self::Archive(true) => "archive",
            Self::Archive(false) => "unarchive",
            Self::Trash(true) => "delete",
            Self::Trash(false) => "restore",
        }
    }
    pub fn success(self) -> &'static str {
        match self {
            Self::Archive(true) => "Entry archived",
            Self::Archive(false) => "Entry unarchived",
            Self::Trash(true) => "Entry moved to trash",
            Self::Trash(false) => "Entry restored",
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Intent {
    pub cipher: CipherId,
    pub archived: bool,
    // Additive compatibility with existing archive/unarchive intent records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trashed: Option<bool>,
}
impl Intent {
    pub fn confirmed(&self, snapshot: &Snapshot) -> bool {
        snapshot
            .ciphers
            .get(&self.cipher.to_string())
            .is_some_and(|cipher| {
                self.trashed
                    .map_or(Change::Archive(self.archived), Change::Trash)
                    .matches(cipher)
            })
    }
}
pub struct Outcome {
    pub report: vault_sync::Report,
    pub applied: bool,
}
async fn install(
    session: &SessionToken,
    client: &PasswordManagerClient,
    intent: Option<Intent>,
) -> Result<(), String> {
    let snapshot = session
        .state
        .snapshot
        .current()
        .ok_or("Synchronize first")?;
    let mut persisted = snapshot.persisted.clone();
    persisted.archive_intent = intent;
    let next = snapshot.rebuild(persisted)?;
    session.ensure_unlocked()?;
    local_edits::commit(client, &next).await?;
    session.state.snapshot.install(next);
    session.ensure_unlocked()
}
#[cfg(test)]
pub async fn change(
    handle: SessionClient,
    id: CipherId,
    archived: bool,
    root: PathBuf,
    store: ProfileStore,
) -> Result<Outcome, String> {
    change_state(handle, id, Change::Archive(archived), root, store).await
}
pub async fn change_state(
    handle: SessionClient,
    id: CipherId,
    change: Change,
    root: PathBuf,
    store: ProfileStore,
) -> Result<Outcome, String> {
    let session = handle.session();
    let _coordinator = session.state.sync_gate.lock().await;
    let _lease = session.state.gate.write().await;
    session.ensure_unlocked()?;
    let client = session.state.client()?;
    super::session_client::check_cipher(&client, &session, id, change.action(), None).await?;
    let before = session
        .state
        .snapshot
        .current()
        .ok_or("Synchronize first")?;
    if before.persisted.archive_intent.is_some() {
        return Err(
            "An item change is unconfirmed. Check the server before another change.".into(),
        );
    }
    if before.persisted.local_edits.contains_key(&id.to_string()) {
        return Err("Synchronize this item's local changes first.".into());
    }
    let original = before
        .ciphers
        .get(&id.to_string())
        .cloned()
        .ok_or("Item no longer exists")?;
    let (fresh, record, transport) =
        vault_sync::fetch(&session, root.clone(), store.clone()).await?;
    vault_sync::apply(&session, &client, fresh, &record, &transport).await?;
    let result = dispatch(
        &session,
        &client,
        id,
        change,
        &original,
        root.clone(),
        store.clone(),
    )
    .await;
    let mut applied = result.as_ref().is_ok_and(|confirmed| *confirmed);
    let mut problems = Vec::new();
    if let Err(error) = result {
        problems.push(error);
    }
    if session.ensure_unlocked().is_ok() {
        let pending = session
            .state
            .snapshot
            .current()
            .is_some_and(|s| s.persisted.archive_intent.is_some());
        match vault_sync::fetch(&session, root, store).await {
            Ok((fresh, record, transport)) => {
                match vault_sync::apply(&session, &client, fresh, &record, &transport).await {
                    Ok(_) => {
                        if pending
                            && session
                                .state
                                .snapshot
                                .current()
                                .is_some_and(|s| s.persisted.archive_intent.is_none())
                        {
                            applied = true;
                            problems.clear();
                        }
                    }
                    Err(error) => problems.push(error),
                }
            }
            Err(error) => problems.push(error),
        }
    }
    session.ensure_unlocked()?;
    let snapshot = session
        .state
        .snapshot
        .current()
        .ok_or("No server snapshot")?;
    let mut report =
        vault_sync::report(session.advance_snapshot(snapshot.collection_names(&client)?)?)?;
    report.problems = problems;
    Ok(Outcome { report, applied })
}
async fn dispatch(
    session: &SessionToken,
    client: &PasswordManagerClient,
    id: CipherId,
    change: Change,
    original: &Cipher,
    root: PathBuf,
    store: ProfileStore,
) -> Result<bool, String> {
    let snapshot = session
        .state
        .snapshot
        .current()
        .ok_or("No server snapshot")?;
    let current = snapshot
        .ciphers
        .get(&id.to_string())
        .ok_or("The item was removed on the server")?;
    if !current.view_password
        || current.key != original.key
        || current.organization_id != original.organization_id
        || current.reprompt != original.reprompt
        || !snapshot
            .policies
            .allows_read(current.r#type, current.organization_id)
        || (matches!(change, Change::Archive(_)) && current.deleted_date.is_some())
        || (matches!(change, Change::Trash(_)) && (!current.edit || current.partial_data.is_some()))
        || (matches!(change, Change::Trash(true))
            && current.permissions.as_ref().is_some_and(|p| !p.delete))
        || (matches!(change, Change::Trash(false))
            && current.permissions.as_ref().is_some_and(|p| !p.restore))
        || (matches!(change, Change::Trash(false))
            && !snapshot
                .policies
                .allows_write(current.r#type, current.organization_id))
    {
        return Err(
            "Item content or permissions changed. Review the current entry before trying again."
                .into(),
        );
    }
    if change.matches(current) {
        return Ok(true);
    }
    if current.revision_date != original.revision_date
        || current.deleted_date != original.deleted_date
        || current.archived_date != original.archived_date
    {
        return Err(
            "The item changed on the server. Review the current entry before trying again.".into(),
        );
    }
    let authority = refresh::authorized(session, root, store, false).await?;
    let url = url::Url::parse(&authority.record.api)
        .and_then(|u| u.join(&format!("ciphers/{id}/{}", change.endpoint())))
        .map_err(|_| "Invalid item endpoint")?;
    // PUT /delete is soft deletion. POST /delete and DELETE /ciphers/<id>
    // are permanent deletion in Vaultwarden and must never be substituted.
    let request = authority
        .transport
        .request(reqwest_account::Method::PUT, url)?
        .bearer_auth(&authority.credentials.access_token)
        .build()
        .map_err(|_| "Could not prepare item change")?;
    let (archived, trashed) = match change {
        Change::Archive(value) => (value, None),
        Change::Trash(value) => (false, Some(value)),
    };
    install(
        session,
        client,
        Some(Intent {
            cipher: id,
            archived,
            trashed,
        }),
    )
    .await?;
    session.ensure_unlocked()?;
    let response=authority.transport.execute(request).await.map_err(|_|"Item change outcome is unknown. Check the server; it will not be repeated automatically.")?;
    session.ensure_unlocked()?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        if status == 401 {
            session
                .state
                .network_authorized
                .store(false, Ordering::SeqCst);
        }
        if matches!(status, 400 | 401 | 403 | 404 | 405 | 409 | 422) {
            install(session, client, None).await?;
        }
        return Err("The server refused the item change or the item is no longer available. Check the connection and permissions.".into());
    }
    // Only a full authenticated read confirms the requested state. Never invent
    // lifecycle timestamps or automatically replay a request with a lost reply.
    Ok(false)
}
impl SessionClient {
    pub async fn dismiss_archive_intent(self) -> Result<(), String> {
        let session = self.session;
        let _coordinator = session.state.sync_gate.lock().await;
        let _lease = session.state.gate.write().await;
        session.ensure_unlocked()?;
        install(&session, &session.state.client()?, None).await
    }
    pub async fn restore_local(self, id: CipherId) -> Result<(), String> {
        let session = self.session();
        self.run(move |client| async move {
            if !session.state.writable.load(Ordering::SeqCst) {
                return Err("Use account-scoped restore synchronization".into());
            }
            super::session_client::check_cipher(&client, &session, id, Action::Restore, None)
                .await?;
            let repo = client
                .platform()
                .state()
                .get::<Cipher>()
                .map_err(|_| "Item repository unavailable")?;
            let mut cipher = repo
                .get(id)
                .await
                .map_err(|_| "Could not read item")?
                .ok_or("Item no longer exists")?;
            cipher.deleted_date = None;
            cipher.revision_date = chrono::Utc::now();
            session.ensure_unlocked()?;
            repo.set(id, cipher)
                .await
                .map_err(|_| "Could not restore item")?;
            session.state.grants.clear();
            Ok(())
        })
        .await
    }
}
