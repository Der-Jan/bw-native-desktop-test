//! Folder mutations use the account transport and a durable encrypted uncertain
//! intent. Vaultwarden assigns create IDs and has no folder CAS/idempotency API.
use super::{SessionClient, SessionToken, local_edits, refresh, snapshot::Snapshot, vault_sync};
use crate::services::accounts::{profile::ProfileStore, transport::json_body_limited};
use bitwarden_api_api::models::{FolderRequestModel, FolderResponseModel};
use bitwarden_pm::PasswordManagerClient;
use bitwarden_vault::{Folder, FolderAddEditRequest, FolderId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::atomic::Ordering};

#[derive(Clone)]
pub enum Change {
    Create(String),
    Rename {
        id: FolderId,
        revision: DateTime<Utc>,
        name: String,
    },
    Delete {
        id: FolderId,
        revision: DateTime<Utc>,
    },
}
impl Change {
    pub fn target(&self) -> Option<FolderId> {
        match self {
            Self::Create(_) => None,
            Self::Rename { id, .. } | Self::Delete { id, .. } => Some(*id),
        }
    }
    fn name(&self) -> Option<&str> {
        match self {
            Self::Create(name) | Self::Rename { name, .. } => Some(name.trim()),
            Self::Delete { .. } => None,
        }
    }
    fn revision(&self) -> Option<DateTime<Utc>> {
        match self {
            Self::Create(_) => None,
            Self::Rename { revision, .. } | Self::Delete { revision, .. } => Some(*revision),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Intent {
    pub original: Option<Folder>,
    pub desired: Option<Folder>,
}
impl Intent {
    pub fn confirmed(&self, snapshot: &Snapshot) -> bool {
        match (&self.original, &self.desired) {
            (None, Some(desired)) => snapshot.folders.values().any(|f| f.name == desired.name),
            (Some(original), Some(desired)) => original
                .id
                .and_then(|id| snapshot.folders.get(&id.to_string()))
                .is_some_and(|f| f.name == desired.name),
            (Some(original), None) => original
                .id
                .is_some_and(|id| !snapshot.folders.contains_key(&id.to_string())),
            _ => false,
        }
    }
}
pub struct Outcome {
    pub report: vault_sync::Report,
    /// Confirmed by a validated reply or exact read-after-write reconciliation.
    pub applied: bool,
}
fn validate_name(change: &Change) -> Result<(), String> {
    if change
        .name()
        .is_some_and(|name| name.is_empty() || name.chars().count() > 1000)
    {
        return Err("Enter a folder name between 1 and 1000 characters".into());
    }
    Ok(())
}
fn encrypt(client: &PasswordManagerClient, change: &Change) -> Result<Option<Folder>, String> {
    change
        .name()
        .map(|name| {
            let request: FolderRequestModel = client
                .0
                .internal
                .get_key_store()
                .encrypt(FolderAddEditRequest { name: name.into() })
                .map_err(|_| "Could not encrypt the folder name")?;
            Ok(Folder {
                id: change.target(),
                name: request
                    .name
                    .parse()
                    .map_err(|_| "Invalid encrypted folder name")?,
                revision_date: Utc::now(),
            })
        })
        .transpose()
}
pub async fn change(
    client: SessionClient,
    change: Change,
    root: PathBuf,
    store: ProfileStore,
) -> Result<Outcome, String> {
    validate_name(&change)?;
    let session = client.session();
    let _coordinator = session.state.sync_gate.lock().await;
    let _lease = session.state.gate.write().await;
    session.ensure_unlocked()?;
    let client = session.state.client()?;
    let before = session
        .state
        .snapshot
        .current()
        .ok_or("Synchronize before managing folders")?;
    if before.persisted.folder_intent.is_some() {
        return Err("A folder request is unconfirmed. Synchronize to check its outcome before another change.".into());
    }
    let original = change
        .target()
        .map(|id| {
            before
                .folders
                .get(&id.to_string())
                .cloned()
                .ok_or("Folder no longer exists")
        })
        .transpose()?;
    if original
        .as_ref()
        .is_some_and(|f| Some(f.revision_date) != change.revision())
    {
        return Err("The folder changed. Close and reopen the folder dialog.".into());
    }
    let (fresh, record, transport) =
        vault_sync::fetch(&session, root.clone(), store.clone()).await?;
    vault_sync::apply(&session, &client, fresh, &record, &transport).await?;
    let mut applied = false;
    let result = dispatch(
        &session,
        &client,
        &change,
        original,
        root.clone(),
        store.clone(),
    )
    .await;
    let mut problems = Vec::new();
    if let Err(error) = result.as_ref() {
        problems.push(error.clone());
    }
    if let Ok(confirmed) = result {
        applied = confirmed;
    }
    // Never dispatch again here: reconcile a rejected or lost reply with reads.
    if session.ensure_unlocked().is_ok() {
        match vault_sync::fetch(&session, root, store).await {
            Ok((fresh, record, transport)) => {
                let pending = session
                    .state
                    .snapshot
                    .current()
                    .is_some_and(|s| s.persisted.folder_intent.is_some());
                match vault_sync::apply(&session, &client, fresh, &record, &transport).await {
                    Ok(_) => {
                        if pending
                            && !session
                                .state
                                .snapshot
                                .current()
                                .is_some_and(|s| s.persisted.folder_intent.is_some())
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
    let current = session
        .state
        .snapshot
        .current()
        .ok_or("No server snapshot")?;
    let names = current.collection_names(&client)?;
    let mut report = vault_sync::report(session.advance_snapshot(names)?)?;
    report.problems = problems;
    Ok(Outcome { report, applied })
}
async fn dispatch(
    session: &SessionToken,
    client: &PasswordManagerClient,
    change: &Change,
    original: Option<Folder>,
    root: PathBuf,
    store: ProfileStore,
) -> Result<bool, String> {
    let snapshot = session
        .state
        .snapshot
        .current()
        .ok_or("No server snapshot")?;
    if let Some(original) = &original {
        let current = original
            .id
            .and_then(|id| snapshot.folders.get(&id.to_string()));
        if current.is_none() && matches!(change, Change::Delete { .. }) {
            return Ok(true);
        }
        if current
            .is_none_or(|f| f.name != original.name || f.revision_date != original.revision_date)
        {
            return Err("The folder changed on the server. Close and reopen the dialog before trying again.".into());
        }
    }
    let desired = encrypt(client, change)?;
    let authority = refresh::authorized(session, root, store, false).await?;
    let path = change
        .target()
        .map_or_else(|| "folders".to_owned(), |id| format!("folders/{id}"));
    let url = url::Url::parse(&authority.record.api)
        .and_then(|u| u.join(&path))
        .map_err(|_| "Invalid folder endpoint")?;
    let method = match change {
        Change::Create(_) => reqwest_account::Method::POST,
        Change::Rename { .. } => reqwest_account::Method::PUT,
        Change::Delete { .. } => reqwest_account::Method::DELETE,
    };
    let mut request = authority
        .transport
        .request(method, url)?
        .bearer_auth(&authority.credentials.access_token);
    if let Some(desired) = &desired {
        request = request.json(&FolderRequestModel {
            name: desired.name.to_string(),
        });
    }
    let request = request
        .build()
        .map_err(|_| "Could not prepare the folder request")?;
    let mut persisted = snapshot.persisted.clone();
    persisted.folder_intent = Some(Intent {
        original,
        desired: desired.clone(),
    });
    let pending = snapshot.rebuild(persisted)?;
    session.ensure_unlocked()?;
    local_edits::commit(client, &pending).await?;
    session.state.snapshot.install(pending.clone());
    session.ensure_unlocked()?;
    let response=authority.transport.execute(request).await.map_err(|_|"Folder request outcome is unknown. Synchronize to check; the request will not be sent again automatically.")?;
    session.ensure_unlocked()?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        if status == 401 {
            session
                .state
                .network_authorized
                .store(false, Ordering::SeqCst);
        }
        // Keep intent for all uncertain/server errors. Explicit request validation
        // and authentication rejection are known not to have applied the mutation.
        if matches!(status, 400 | 401 | 403 | 404 | 409 | 422) {
            let mut persisted = pending.persisted.clone();
            persisted.folder_intent = None;
            let next = pending.rebuild(persisted)?;
            local_edits::commit(client, &next).await?;
            session.state.snapshot.install(next);
        }
        return Err("The server refused the folder change. Your existing folders and entries are retained; check the server connection and permissions.".into());
    }
    if matches!(change, Change::Delete { .. }) {
        // The subsequent full sync clears folder assignments using the server's
        // authoritative ciphers. Do not fabricate entry revisions locally.
        return Ok(true);
    }
    let value = json_body_limited(response, 32 * 1024).await?;
    let model: FolderResponseModel = serde_json::from_value(value)
        .map_err(|_| "Unverified folder response. Synchronize to check the outcome.")?;
    let folder = Folder::try_from(model.clone())
        .map_err(|_| "Incomplete folder response. Synchronize to check the outcome.")?;
    let id = folder
        .id
        .ok_or("The server returned no folder ID; synchronize before retrying")?;
    if change.target().is_some_and(|expected| expected != id)
        || desired.as_ref().is_none_or(|d| d.name != folder.name)
    {
        return Err(
            "The returned folder does not match the request. Synchronize to check the outcome."
                .into(),
        );
    }
    if matches!(change, Change::Create(_)) && snapshot.folders.contains_key(&id.to_string()) {
        return Err(
            "The server returned an existing folder ID. Synchronize to check the outcome.".into(),
        );
    }
    // Acknowledgment and server-assigned ID are durable even if the final read fails.
    let mut persisted = pending.persisted.clone();
    let folders = persisted
        .response
        .folders
        .as_mut()
        .ok_or("No folder snapshot")?;
    folders.retain(|f| f.id != model.id);
    folders.push(model);
    persisted.folder_intent = None;
    let next = pending.rebuild(persisted)?;
    session.ensure_unlocked()?;
    local_edits::commit(client, &next).await?;
    session.state.snapshot.install(next);
    Ok(true)
}
impl SessionClient {
    pub async fn dismiss_folder_intent(self) -> Result<(), String> {
        let session = self.session;
        let _coordinator = session.state.sync_gate.lock().await;
        let _lease = session.state.gate.write().await;
        session.ensure_unlocked()?;
        let snapshot = session
            .state
            .snapshot
            .current()
            .ok_or("No server snapshot")?;
        let mut persisted = snapshot.persisted.clone();
        persisted.folder_intent = None;
        let next = snapshot.rebuild(persisted)?;
        let client = session.state.client()?;
        local_edits::commit(&client, &next).await?;
        session.state.snapshot.install(next);
        session.ensure_unlocked()
    }
    pub async fn change_local_folder(self, change: Change) -> Result<(), String> {
        validate_name(&change)?;
        let session = self.session.clone();
        self.run(move |client| async move {
            if !session.state.writable.load(Ordering::SeqCst) {
                return Err("Use account-scoped folder synchronization".into());
            }
            let repo = client
                .platform()
                .state()
                .get::<Folder>()
                .map_err(|_| "Folder repository unavailable")?;
            if let Some(id) = change.target() {
                let original = repo
                    .get(id)
                    .await
                    .map_err(|_| "Could not read folder")?
                    .ok_or("Folder no longer exists")?;
                if Some(original.revision_date) != change.revision() {
                    return Err("The folder changed; reopen its dialog".into());
                }
            }
            match encrypt(&client, &change)? {
                Some(mut folder) => {
                    let id = change.target().unwrap_or_else(FolderId::new_v4);
                    folder.id = Some(id);
                    repo.set(id, folder)
                        .await
                        .map_err(|_| "Could not save folder")?;
                }
                None => {
                    let id = change.target().ok_or("No folder selected")?;
                    let ciphers = client
                        .platform()
                        .state()
                        .get::<bitwarden_vault::Cipher>()
                        .map_err(|_| "Item repository unavailable")?;
                    for mut cipher in ciphers.list().await.map_err(|_| "Could not read items")? {
                        if cipher.folder_id == Some(id) {
                            cipher.folder_id = None;
                            let cid = cipher.id.ok_or("Item without ID")?;
                            ciphers
                                .set(cid, cipher)
                                .await
                                .map_err(|_| "Could not clear folder assignment")?;
                        }
                    }
                    repo.remove(id)
                        .await
                        .map_err(|_| "Could not delete folder")?;
                }
            }
            Ok(())
        })
        .await
    }
}
