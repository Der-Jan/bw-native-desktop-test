//! Account-local archive state, changed online with durable read-only recovery.
use super::{
    Action, SessionClient, SessionToken, local_edits, refresh, snapshot::Snapshot, vault_sync,
};
use crate::services::accounts::profile::ProfileStore;
use bitwarden_pm::PasswordManagerClient;
use bitwarden_vault::CipherId;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::atomic::Ordering};

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Intent {
    pub cipher: CipherId,
    pub archived: bool,
}
impl Intent {
    pub fn confirmed(&self, snapshot: &Snapshot) -> bool {
        snapshot
            .ciphers
            .get(&self.cipher.to_string())
            .is_some_and(|c| c.archived_date.is_some() == self.archived)
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
pub async fn change(
    handle: SessionClient,
    id: CipherId,
    archived: bool,
    root: PathBuf,
    store: ProfileStore,
) -> Result<Outcome, String> {
    let session = handle.session();
    let _coordinator = session.state.sync_gate.lock().await;
    let _lease = session.state.gate.write().await;
    session.ensure_unlocked()?;
    let client = session.state.client()?;
    super::session_client::check_cipher(&client, &session, id, Action::Archive, None).await?;
    let before = session
        .state
        .snapshot
        .current()
        .ok_or("Synchronize first")?;
    if before.persisted.archive_intent.is_some() {
        return Err(
            "An archive request is unconfirmed. Check the server before another change.".into(),
        );
    }
    if before.persisted.local_edits.contains_key(&id.to_string()) {
        return Err("Synchronize this item's local changes before archiving it.".into());
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
        archived,
        &original,
        root.clone(),
        store.clone(),
    )
    .await;
    let mut problems = Vec::new();
    let mut applied = result.as_ref().is_ok_and(|confirmed| *confirmed);
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
    archived: bool,
    original: &bitwarden_vault::Cipher,
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
        || current.deleted_date.is_some()
        || current.revision_date != original.revision_date
        || current.key != original.key
        || current.organization_id != original.organization_id
        || current.reprompt != original.reprompt
        || !snapshot
            .policies
            .allows_read(current.r#type, current.organization_id)
    {
        return Err(
            "Item content or permissions changed. Review the current entry before trying again."
                .into(),
        );
    }
    if current.archived_date.is_some() == archived {
        return Ok(true);
    }
    if current.archived_date != original.archived_date {
        return Err("The archive state changed on the server. Review the current entry before trying again.".into());
    }
    let authority = refresh::authorized(session, root, store, false).await?;
    let operation = if archived { "archive" } else { "unarchive" };
    let url = url::Url::parse(&authority.record.api)
        .and_then(|u| u.join(&format!("ciphers/{id}/{operation}")))
        .map_err(|_| "Invalid archive endpoint")?;
    let request = authority
        .transport
        .request(reqwest_account::Method::PUT, url)?
        .bearer_auth(&authority.credentials.access_token)
        .build()
        .map_err(|_| "Could not prepare archive request")?;
    install(
        session,
        client,
        Some(Intent {
            cipher: id,
            archived,
        }),
    )
    .await?;
    session.ensure_unlocked()?;
    let response=authority.transport.execute(request).await.map_err(|_|"Archive request outcome is unknown. Check the server; it will not be repeated automatically.")?;
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
        return Err(if matches!(status, 404 | 405) {
            "The server does not support this archive action or the item is no longer available."
        } else {
            "The server refused the archive change. Check the connection and item permissions."
        }
        .into());
    }
    // A full authenticated read confirms the account-local flag; do not invent
    // timestamps or trust success alone, and never repeat a lost PUT here.
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
}
