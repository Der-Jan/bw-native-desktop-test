//! New-entry creation and revision-checked existing-entry uploads. Never replay an uncertain mutation automatically.
use super::{
    SessionToken, local_edits, refresh,
    snapshot::{LocalEditState, Snapshot},
};
use crate::services::accounts::profile::ProfileStore;
use bitwarden_api_api::models::{CipherDetailsResponseModel, CipherRequestModel};
use bitwarden_pm::PasswordManagerClient;
use bitwarden_vault::Cipher;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};

fn request(cipher: &Cipher) -> Result<CipherRequestModel, String> {
    cipher
        .clone()
        .try_into()
        .map_err(|_| "The encrypted item cannot be uploaded".into())
}
pub(super) fn same_content(a: &Cipher, b: &Cipher) -> Result<bool, String> {
    let mut a = request(a)?;
    let mut b = request(b)?;
    a.last_known_revision_date = None;
    b.last_known_revision_date = None;
    // Server DTOs normalize missing attachment/field/history arrays to empty.
    // Their absence and emptiness carry the same request semantics.
    for value in [&mut a, &mut b] {
        if value.attachments2.as_ref().is_some_and(|v| v.is_empty()) {
            value.attachments2 = None;
        }
        if value.fields.as_ref().is_some_and(|v| v.is_empty()) {
            value.fields = None;
        }
        if value
            .password_history
            .as_ref()
            .is_some_and(|v| v.is_empty())
        {
            value.password_history = None;
        }
    }
    Ok(a == b)
}
fn permitted(
    client: &PasswordManagerClient,
    snapshot: &Snapshot,
    remote: &Cipher,
    draft: &Cipher,
) -> bool {
    let metadata = remote.edit
        && remote.view_password
        && remote.deleted_date.is_none()
        && remote.partial_data.is_none()
        && draft.partial_data.is_none()
        && remote.organization_id == draft.organization_id
        && remote.collection_ids == draft.collection_ids
        && remote.r#type == draft.r#type
        && snapshot
            .policies
            .allows_write(remote.r#type, remote.organization_id)
        && draft
            .folder_id
            .is_none_or(|id| snapshot.folders.contains_key(&id.to_string()));
    if !metadata {
        return false;
    }
    // Key wraps and attachment metadata use randomized encryption. Compare their
    // decrypted values, never ciphertext equality. Legacy keyless items may be
    // upgraded to per-item keys by the SDK.
    let keys = client.0.internal.get_key_store();
    let remote: Result<bitwarden_vault::CipherView, _> = keys.decrypt(remote);
    let draft: Result<bitwarden_vault::CipherView, _> = keys.decrypt(draft);
    match (remote, draft) {
        (Ok(remote), Ok(draft)) => {
            (remote.key.is_none() || remote.key == draft.key)
                && remote.attachments == draft.attachments
        }
        _ => false,
    }
}
/// Reconcile against a fresh full server snapshot before any write. Conflicts do
/// not overlay current data or borrow previous organization permissions/keys.
pub(super) fn reconcile(
    client: &PasswordManagerClient,
    fresh: Arc<Snapshot>,
    old: Option<&Snapshot>,
) -> Result<Arc<Snapshot>, String> {
    let Some(old) = old else {
        return Ok(fresh);
    };
    let mut persisted = fresh.persisted.clone();
    persisted.folder_intent = old
        .persisted
        .folder_intent
        .clone()
        .filter(|intent| !intent.confirmed(&fresh));
    persisted.placement_intent = old
        .persisted
        .placement_intent
        .clone()
        .filter(|i| !i.confirmed(&fresh));
    persisted.archive_intent = old
        .persisted
        .archive_intent
        .clone()
        .filter(|intent| !intent.confirmed(&fresh));
    persisted.created_ids = old.persisted.created_ids.clone();
    persisted.attachment_intent = old.persisted.attachment_intent.clone();
    for (id, draft) in &old.persisted.local_edits {
        if draft.created {
            let matched = fresh
                .ciphers
                .values()
                .find(|remote| same_content(remote, &draft.cipher).unwrap_or(false));
            if let Some(remote) = matched {
                if let (Some(local), Some(server)) = (draft.cipher.id, remote.id) {
                    persisted.created_ids.push((local, server));
                }
                continue;
            }
            let mut draft = draft.clone();
            if draft.state != LocalEditState::Pending || !creation_allowed(&fresh, &draft.cipher) {
                draft.state = LocalEditState::Conflict;
            }
            persisted.local_edits.insert(id.clone(), draft);
            continue;
        }
        let remote = fresh.ciphers.get(id);
        if let Some(remote) = remote
            && same_content(remote, &draft.cipher)?
        {
            continue; // Includes successful writes whose response was lost.
        }
        let mut draft = draft.clone();
        if draft.state != LocalEditState::Pending
            || remote.is_none_or(|remote| {
                remote.revision_date != draft.base_revision
                    || !permitted(client, &fresh, remote, &draft.cipher)
            })
        {
            draft.state = LocalEditState::Conflict;
        }
        persisted.local_edits.insert(id.clone(), draft);
    }
    if persisted.created_ids.len() > 256 {
        persisted
            .created_ids
            .drain(..persisted.created_ids.len() - 256);
    }
    fresh.rebuild(persisted)
}

pub(super) fn creation_allowed(snapshot: &Snapshot, cipher: &Cipher) -> bool {
    if !snapshot
        .policies
        .allows_write(cipher.r#type, cipher.organization_id)
        || cipher
            .folder_id
            .is_some_and(|id| !snapshot.folders.contains_key(&id.to_string()))
    {
        return false;
    }
    match cipher.organization_id {
        None => cipher.collection_ids.is_empty(),
        Some(org) => {
            snapshot
                .persisted
                .response
                .profile
                .as_ref()
                .and_then(|p| p.organizations_new.as_ref().or(p.organizations.as_ref()))
                .is_some_and(|orgs| {
                    orgs.iter().any(|o| {
                        o.id == Some(org.into())
                        && o.enabled != Some(false)
                        && o.status
                            == Some(
                                bitwarden_api_api::models::OrganizationUserStatusType::Confirmed,
                            )
                    })
                })
                && !cipher.collection_ids.is_empty()
                && snapshot
                    .organizations
                    .iter()
                    .any(|o| o.id == org && o.wrapped_key.is_some())
                && cipher.collection_ids.iter().all(|id| {
                    snapshot.collections.iter().any(|c| {
                        c.id == Some(*id)
                            && c.organization_id == org
                            && !c.read_only
                            && !c.hide_passwords
                    })
                })
        }
    }
}

#[derive(Default)]
pub(super) struct UploadReport {
    pub uploaded: usize,
    pub problems: Vec<String>,
}
/// Caller holds the sync coordinator and exclusive session lease. Ciphertext
/// intent is durable before dispatch, and every next dispatch rechecks revocation.
pub(super) async fn upload(
    session: &SessionToken,
    client: &PasswordManagerClient,
    root: PathBuf,
    store: ProfileStore,
) -> Result<UploadReport, String> {
    let mut report = UploadReport::default();
    let current = session
        .state
        .snapshot
        .current()
        .ok_or("No server snapshot")?;
    let mut ids: Vec<_> = current
        .persisted
        .local_edits
        .iter()
        .filter(|(_, e)| e.state == LocalEditState::Pending)
        .map(|(id, _)| id.clone())
        .collect();
    ids.sort();
    for id in ids {
        session.ensure_unlocked()?;
        let authority = match refresh::authorized(session, root.clone(), store.clone(), false).await
        {
            Ok(authority) => authority,
            Err(error) => {
                report.problems.push(error);
                break;
            }
        };
        let snapshot = session
            .state
            .snapshot
            .current()
            .ok_or("No server snapshot")?;
        let draft = snapshot
            .persisted
            .local_edits
            .get(&id)
            .ok_or("Missing local draft")?;
        if draft.state != LocalEditState::Pending {
            continue;
        }
        let mut body = request(&draft.cipher)?;
        body.last_known_revision_date = if draft.created {
            None
        } else {
            Some(draft.base_revision.to_rfc3339())
        };
        body.encrypted_for = Some(authority.record.server_user_id.into());
        body.encrypted_by_key_id = if draft.cipher.organization_id.is_none() {
            snapshot
                .persisted
                .response
                .user_decryption
                .as_ref()
                .and_then(|u| u.user_key_id.clone())
        } else {
            None
        };
        let path = if draft.created {
            if draft.cipher.organization_id.is_some() {
                "ciphers/create".into()
            } else {
                "ciphers".into()
            }
        } else {
            format!("ciphers/{id}")
        };
        let url = url::Url::parse(&authority.record.api)
            .and_then(|u| u.join(&path))
            .map_err(|_| "Invalid cipher endpoint")?;
        let body = if draft.created && draft.cipher.organization_id.is_some() {
            serde_json::json!({"cipher":body,"collectionIds":draft.cipher.collection_ids})
        } else {
            serde_json::to_value(body).map_err(|_| "Could not encode encrypted item")?
        };
        let request = authority
            .transport
            .request(
                if draft.created {
                    reqwest_account::Method::POST
                } else {
                    reqwest_account::Method::PUT
                },
                url,
            )?
            .bearer_auth(&authority.credentials.access_token)
            .json(&body)
            .build()
            .map_err(|_| "Cannot prepare encrypted item")?;
        // Mark intent BEFORE HTTP. A crash between commit and dispatch is safe:
        // it may require user review, but can never lose or silently replay edits.
        let mut pending = snapshot.persisted.clone();
        pending.local_edits.get_mut(&id).unwrap().state = LocalEditState::Uncertain;
        let pending = snapshot.rebuild(pending)?;
        session.ensure_unlocked()?;
        local_edits::commit(client, &pending).await?;
        session.state.snapshot.install(pending);
        session.ensure_unlocked()?;
        let response = match authority.transport.execute(request).await {
            Ok(response) => response,
            Err(_) => {
                report.problems.push("The update outcome is unknown. Synchronize again to check the server; your encrypted draft is retained and will not be resent automatically.".into());
                break;
            }
        };
        session.ensure_unlocked()?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            if status == 401 {
                session
                    .state
                    .network_authorized
                    .store(false, Ordering::SeqCst);
            }
            report.problems.push(match status {
                400 | 409 => "The server rejected an update, possibly because the item changed. Your draft is retained. Synchronize again to load the current server version.",
                401 | 403 => "The server refused the update. Sign in again or check your permissions; your draft is retained.",
                _ => "The server did not confirm the update. Your draft is retained; synchronize again before reviewing or retrying.",
            }.into());
            // No blind token-refresh retry of a mutation. The next sync reconciles it.
            break;
        }
        let model = match crate::services::accounts::transport::json_body_limited(
            response,
            8 * 1024 * 1024,
        )
        .await
        .and_then(|v| {
            serde_json::from_value::<CipherDetailsResponseModel>(v)
                .map_err(|_| "Invalid update response".into())
        }) {
            Ok(model) => model,
            Err(_) => {
                report.problems.push("The update response could not be verified. Synchronize again; your draft is retained.".into());
                break;
            }
        };
        let snapshot = session
            .state
            .snapshot
            .current()
            .ok_or("No server snapshot")?;
        let mut persisted = snapshot.persisted.clone();
        let models = persisted
            .response
            .ciphers
            .as_mut()
            .ok_or("Missing server entries")?;
        let original = models
            .iter()
            .find(|m| m.id.map(|id| id.to_string()).as_ref() == Some(&id));
        if !draft.created && original.is_none() {
            return Err("Missing original server entry".into());
        }
        let mut model = model;
        if model.collection_ids.is_none() {
            model.collection_ids = Some(
                draft
                    .cipher
                    .collection_ids
                    .iter()
                    .map(|id| (*id).into())
                    .collect(),
            );
        }
        let confirmed = Cipher::try_from(model.clone())
            .map_err(|_| "The update response is incomplete; the encrypted draft is retained")?;
        if confirmed.id.is_none()
            || (!draft.created && confirmed.id != draft.cipher.id)
            || !same_content(&confirmed, &draft.cipher)?
        {
            report.problems.push("The returned item does not match the update. Synchronize again; your draft is retained.".into());
            break;
        }
        // Successful HTTP alone is not enough: only verified ciphertext plus the
        // server revision is published, in the same SQLite commit that clears intent.
        if draft.created {
            let server = confirmed.id.ok_or("Server did not assign an item ID")?;
            if models.iter().any(|m| m.id == Some(server.into())) {
                return Err(
                    "The server returned an existing item ID; synchronize to check the outcome"
                        .into(),
                );
            }
            persisted
                .created_ids
                .push((draft.cipher.id.ok_or("Missing local item ID")?, server));
            models.push(model);
        } else {
            *models
                .iter_mut()
                .find(|m| m.id.map(|id| id.to_string()).as_ref() == Some(&id))
                .ok_or("Missing original item")? = model;
        }
        persisted.local_edits.remove(&id);
        let next = snapshot.rebuild(persisted)?;
        session.ensure_unlocked()?;
        local_edits::commit(client, &next).await?;
        session.state.snapshot.install(next);
        session.state.grants.clear();
        report.uploaded += 1;
    }
    Ok(report)
}
