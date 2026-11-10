//! Online ownership/collection changes. Never replay an uncertain sharing request.
use super::{
    Action, ClientExt, SessionClient, SessionToken, local_edits, refresh, snapshot::Snapshot,
    vault_sync, vault_writes,
};
use crate::services::accounts::profile::ProfileStore;
use bitwarden_collections::collection::CollectionId;
use bitwarden_core::OrganizationId;
use bitwarden_pm::PasswordManagerClient;
use bitwarden_vault::{Cipher, CipherId};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf, sync::atomic::Ordering};

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Intent {
    pub cipher: CipherId,
    organization: OrganizationId,
    collections: Vec<CollectionId>,
    // Only ciphertext, including SDK-rewrapped keys; no decrypted item data.
    encrypted: Option<Cipher>,
}
fn same_set(a: &[CollectionId], b: &[CollectionId]) -> bool {
    a.iter().collect::<HashSet<_>>() == b.iter().collect::<HashSet<_>>()
}
impl Intent {
    pub fn confirmed(&self, snapshot: &Snapshot) -> bool {
        snapshot
            .ciphers
            .get(&self.cipher.to_string())
            .is_some_and(|current| {
                current.organization_id == Some(self.organization)
                    && same_set(&current.collection_ids, &self.collections)
                    && self.encrypted.as_ref().is_none_or(|desired| {
                        vault_writes::same_content(desired, current).unwrap_or(false)
                    })
            })
    }
}
pub(super) fn pending(snapshot: &Snapshot, id: CipherId) -> bool {
    snapshot
        .persisted
        .placement_intent
        .as_ref()
        .is_some_and(|i| i.cipher == id)
}
pub(super) fn organization_allowed(
    snapshot: &Snapshot,
    cipher: &Cipher,
    org: OrganizationId,
) -> bool {
    let mut probe = cipher.clone();
    probe.organization_id = Some(org);
    probe.collection_ids = snapshot
        .collections
        .iter()
        .filter(|c| c.organization_id == org && !c.read_only && !c.hide_passwords)
        .filter_map(|c| c.id)
        .take(1)
        .collect();
    vault_writes::creation_allowed(snapshot, &probe)
}
fn validate(
    snapshot: &Snapshot,
    original: &Cipher,
    org: OrganizationId,
    ids: &[CollectionId],
) -> Result<(), String> {
    if !original.edit
        || !original.view_password
        || original.deleted_date.is_some()
        || original.partial_data.is_some()
        || !snapshot
            .policies
            .allows_read(original.r#type, original.organization_id)
        || original
            .organization_id
            .is_some_and(|current| current != org)
        || !organization_allowed(snapshot, original, org)
    {
        return Err("This item or organization does not permit this change".into());
    }
    if ids.is_empty() || ids.len() > 1024 || ids.iter().collect::<HashSet<_>>().len() != ids.len() {
        return Err(
            "Select at least one collection; duplicate or excessive selections are not allowed"
                .into(),
        );
    }
    for id in ids.iter().chain(original.collection_ids.iter()) {
        let collection = snapshot
            .collections
            .iter()
            .find(|c| c.id == Some(*id) && c.organization_id == org)
            .ok_or(
                "A selected or existing collection is unavailable; synchronize and review the item",
            )?;
        let changed = ids.contains(id) != original.collection_ids.contains(id);
        if changed && (collection.read_only || collection.hide_passwords) {
            return Err("You do not have permission to change this collection assignment".into());
        }
    }
    if !ids.iter().any(|id| {
        snapshot
            .collections
            .iter()
            .any(|c| c.id == Some(*id) && !c.read_only && !c.hide_passwords)
    }) {
        return Err("Keep at least one writable collection selected".into());
    }
    Ok(())
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
    persisted.placement_intent = intent;
    let next = snapshot.rebuild(persisted)?;
    session.ensure_unlocked()?;
    local_edits::commit(client, &next).await?;
    session.state.snapshot.install(next);
    session.ensure_unlocked()
}
pub async fn change(
    handle: SessionClient,
    id: CipherId,
    org: OrganizationId,
    collections: Vec<CollectionId>,
    root: PathBuf,
    store: ProfileStore,
) -> Result<super::archive::Outcome, String> {
    let session = handle.session();
    let _coordinator = session.state.sync_gate.lock().await;
    let _lease = session.state.gate.write().await;
    session.ensure_unlocked()?;
    let client = session.state.client()?;
    super::session_client::check_cipher(&client, &session, id, Action::Write, None).await?;
    let before = session
        .state
        .snapshot
        .current()
        .ok_or("Synchronize first")?;
    if before.persisted.placement_intent.is_some()
        || before.persisted.attachment_intent.is_some()
        || before.persisted.archive_intent.is_some()
        || before.persisted.folder_intent.is_some()
        || before.persisted.local_edits.contains_key(&id.to_string())
    {
        return Err("Synchronize pending changes and resolve unconfirmed requests before changing ownership or collections".into());
    }
    let original = before
        .ciphers
        .get(&id.to_string())
        .cloned()
        .ok_or("Item unavailable")?;
    validate(&before, &original, org, &collections)?;
    let (fresh, record, transport) =
        vault_sync::fetch(&session, root.clone(), store.clone()).await?;
    vault_sync::apply(&session, &client, fresh, &record, &transport).await?;
    let result = dispatch(
        &session,
        &client,
        &original,
        org,
        collections,
        root.clone(),
        store.clone(),
    )
    .await;
    let no_error = result.is_ok();
    let mut problems = result.err().into_iter().collect::<Vec<_>>();
    let sent = session
        .state
        .snapshot
        .current()
        .is_some_and(|s| s.persisted.placement_intent.is_some());
    let mut applied = no_error && !sent;
    if session.ensure_unlocked().is_ok() {
        match vault_sync::fetch(&session, root, store).await {
            Ok((fresh, record, transport)) => {
                match vault_sync::apply(&session, &client, fresh, &record, &transport).await {
                    Ok(_) => {
                        if sent
                            && session
                                .state
                                .snapshot
                                .current()
                                .is_some_and(|s| s.persisted.placement_intent.is_none())
                        {
                            applied = true;
                            problems.clear();
                        }
                    }
                    Err(e) => problems.push(e),
                }
            }
            Err(e) => problems.push(e),
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
    Ok(super::archive::Outcome { report, applied })
}
async fn dispatch(
    session: &SessionToken,
    client: &PasswordManagerClient,
    original: &Cipher,
    org: OrganizationId,
    collections: Vec<CollectionId>,
    root: PathBuf,
    store: ProfileStore,
) -> Result<(), String> {
    let snapshot = session
        .state
        .snapshot
        .current()
        .ok_or("Synchronize first")?;
    let id = original.id.ok_or("Missing item ID")?;
    let current = snapshot
        .ciphers
        .get(&id.to_string())
        .ok_or("The item was removed on the server")?;
    validate(&snapshot, current, org, &collections)?;
    if current.revision_date != original.revision_date
        || current.organization_id != original.organization_id
        || current.key != original.key
        || current.reprompt != original.reprompt
        || !same_set(&current.collection_ids, &original.collection_ids)
        || !vault_writes::same_content(current, original)?
    {
        return Err(
            "The item changed on the server. Review it before changing ownership or collections"
                .into(),
        );
    }
    if current.organization_id == Some(org) && same_set(&current.collection_ids, &collections) {
        return Ok(());
    }
    let authority = refresh::authorized(session, root, store, false).await?;
    let (encrypted, body, path) = if current.organization_id.is_none() {
        let view = client.clone().full_cipher(id).await?;
        if view.partial
            || view
                .attachment_decryption_failures
                .as_ref()
                .is_some_and(|v| !v.is_empty())
        {
            return Err("Incomplete decryption; moving this item would risk losing data".into());
        }
        let mut view=client.vault().ciphers().move_to_organization(view,org)
            .map_err(|_|"Items with legacy attachment keys must be upgraded in the web vault before moving")?;
        view.collection_ids = collections.clone();
        let encrypted = client
            .vault()
            .ciphers()
            .encrypt(view)
            .await
            .map_err(|_| "Could not encrypt the item for the organization")?
            .cipher;
        let mut model: bitwarden_api_api::models::CipherRequestModel = encrypted
            .clone()
            .try_into()
            .map_err(|_| "Could not prepare encrypted sharing request")?;
        model.last_known_revision_date = Some(current.revision_date.to_rfc3339());
        model.encrypted_for = Some(authority.record.server_user_id.into());
        (
            Some(encrypted),
            serde_json::json!({"cipher":model,"collectionIds":collections}),
            format!("ciphers/{id}/share"),
        )
    } else {
        (
            None,
            serde_json::json!({"collectionIds":collections}),
            format!("ciphers/{id}/collections"),
        )
    };
    let url = url::Url::parse(&authority.record.api)
        .and_then(|u| u.join(&path))
        .map_err(|_| "Invalid collection endpoint")?;
    let request = authority
        .transport
        .request(reqwest_account::Method::PUT, url)?
        .bearer_auth(&authority.credentials.access_token)
        .json(&body)
        .build()
        .map_err(|_| "Could not prepare collection change")?;
    install(
        session,
        client,
        Some(Intent {
            cipher: id,
            organization: org,
            collections,
            encrypted,
        }),
    )
    .await?;
    session.ensure_unlocked()?;
    let response = authority
        .transport
        .execute(request)
        .await
        .map_err(|_| "The outcome is unknown. Check the server before repeating this change")?;
    session.ensure_unlocked()?;
    if response.status().as_u16() == 401 {
        session
            .state
            .network_authorized
            .store(false, Ordering::SeqCst);
    }
    // Even a 4xx may follow partial collection changes on Vaultwarden. Keep the
    // durable record until full read confirmation or explicit dismissal.
    if !response.status().is_success() {
        return Err("The server rejected or did not confirm the change. Check the server and permissions before retrying".into());
    }
    Ok(())
}
impl SessionClient {
    pub async fn dismiss_placement_intent(self) -> Result<(), String> {
        let session = self.session;
        let _coordinator = session.state.sync_gate.lock().await;
        let _lease = session.state.gate.write().await;
        session.ensure_unlocked()?;
        install(&session, &session.state.client()?, None).await
    }
}
