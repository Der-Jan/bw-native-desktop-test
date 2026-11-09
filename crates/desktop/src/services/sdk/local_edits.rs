//! Durable encrypted local drafts, preserving the original server revision for sync.
use super::{Action, ClientExt, SessionClient, snapshot};
use bitwarden_vault::EncryptionContext;
use bitwarden_vault::{CipherId, CipherView};
use std::sync::Arc;

impl SessionClient {
    pub(super) async fn save_local(self, mut view: CipherView) -> Result<CipherView, String> {
        if view.id.is_none() {
            return self.create_local(view).await;
        }
        let session = self.session;
        let _coordinator = session.state.sync_gate.lock().await;
        let _lease = session.state.gate.write().await;
        session.ensure_unlocked()?;
        let client = session.state.client()?;
        let rules = session.policy_rules();
        if !rules.allows_write(view.r#type, view.organization_id) {
            return Err(super::policies::DENIED.into());
        }
        let id = view.id.ok_or("Missing item ID")?;
        if session.state.snapshot.current().is_some_and(|s| {
            s.persisted
                .archive_intent
                .as_ref()
                .is_some_and(|intent| intent.cipher == id)
        }) {
            return Err(
                "Check or dismiss this item's unconfirmed change before editing it.".into(),
            );
        }
        super::session_client::check_cipher(
            &client,
            &session,
            id,
            Action::Write,
            Some(view.revision_date),
        )
        .await?;
        let current = client.clone().full_cipher(id).await?;
        // Accept content edits only. Ownership, permissions, keys, attachments,
        // type and lifecycle timestamps cannot be changed by a crafted draft.
        if view.organization_id != current.organization_id
            || view.collection_ids != current.collection_ids
            || view.key != current.key
            || view.r#type != current.r#type
            || view.organization_use_totp != current.organization_use_totp
            || view.edit != current.edit
            || view.permissions != current.permissions
            || view.view_password != current.view_password
            || view.local_data != current.local_data
            || view.attachments != current.attachments
            || view.attachment_decryption_failures != current.attachment_decryption_failures
            || view.password_history != current.password_history
            || view.creation_date != current.creation_date
            || view.deleted_date != current.deleted_date
            || view.archived_date != current.archived_date
            || view.partial != current.partial
        {
            return Err(
                "Changing item ownership, permissions or attachments is not supported".into(),
            );
        }
        if current.partial
            || current
                .attachment_decryption_failures
                .as_ref()
                .is_some_and(|v| !v.is_empty())
        {
            return Err("Incomplete item decryption; editing would risk losing data".into());
        }
        // SDK encryption upgrades keyless items to per-item keys. Legacy files
        // encrypted directly under the old user/org key cannot follow that
        // upgrade without downloading and re-encrypting their contents.
        if current.key.is_none()
            && current
                .attachments
                .iter()
                .flatten()
                .any(|a| a.key.is_none())
        {
            return Err("This item has legacy attachments. Upgrade them in the web vault before editing here.".into());
        }
        if view.name.trim().is_empty() {
            return Err("Enter an item name".into());
        }
        let snapshot = session
            .state
            .snapshot
            .current()
            .ok_or("Synchronize once before editing")?;
        if view
            .folder_id
            .is_some_and(|id| !snapshot.folders.contains_key(&id.to_string()))
        {
            return Err("The selected folder no longer exists".into());
        }
        update_history(&mut view, &current);
        view.revision_date =
            chrono::Utc::now().max(current.revision_date + chrono::Duration::microseconds(1));
        let EncryptionContext { cipher, .. } = client
            .vault()
            .ciphers()
            .encrypt(view)
            .await
            .map_err(|e| e.to_string())?;
        let result = client
            .vault()
            .ciphers()
            .decrypt(cipher.clone())
            .await
            .map_err(|e| e.to_string())?;
        let next = snapshot.with_edit(cipher)?;
        session.ensure_unlocked()?;
        // A revocation after this point drains the lease before key cleanup or
        // durable logout. Only ciphertext is committed; stale plaintext is dropped.
        commit(&client, &next).await?;
        session.state.snapshot.install(next);
        session.state.grants.clear();
        session.ensure_unlocked()?;
        Ok(result)
    }

    async fn create_local(self, mut view: CipherView) -> Result<CipherView, String> {
        let session = self.session;
        let _coordinator = session.state.sync_gate.lock().await;
        let _lease = session.state.gate.write().await;
        session.ensure_unlocked()?;
        let client = session.state.client()?;
        let snapshot = session
            .state
            .snapshot
            .current()
            .ok_or("Synchronize before creating entries")?;
        if view.name.trim().is_empty() {
            return Err("Enter an item name".into());
        }
        if view.attachments.as_ref().is_some_and(|v| !v.is_empty()) || view.partial {
            return Err("Save the new entry before attaching files".into());
        }
        view.id = Some(CipherId::new_v4());
        view.key = None;
        view.edit = true;
        view.view_password = true;
        view.permissions = None;
        view.deleted_date = None;
        view.archived_date = None;
        view.local_data = None;
        view.password_history = None;
        view.attachment_decryption_failures = None;
        view.creation_date = chrono::Utc::now();
        view.revision_date = view.creation_date;
        let EncryptionContext { cipher, .. } = client
            .vault()
            .ciphers()
            .encrypt(view)
            .await
            .map_err(|_| "Could not encrypt the new item")?;
        if !super::vault_writes::creation_allowed(&snapshot, &cipher) {
            return Err(
                "The selected vault or collection does not permit creating this item".into(),
            );
        }
        let result = client
            .vault()
            .ciphers()
            .decrypt(cipher.clone())
            .await
            .map_err(|_| "Could not validate the new item")?;
        let next = snapshot.with_creation(cipher)?;
        session.ensure_unlocked()?;
        commit(&client, &next).await?;
        session.state.snapshot.install(next);
        session.state.grants.clear();
        session.ensure_unlocked()?;
        Ok(result)
    }

    /// Explicit conflict review: preserve fresh ownership, permissions, revision and keys.
    pub async fn review_local(self, id: CipherId) -> Result<CipherView, String> {
        let session = self.session.clone();
        self.run(move |client| async move {
            let snapshot = session.state.snapshot.current().ok_or("No server snapshot")?;
            let edit = snapshot.persisted.local_edits.get(&id.to_string()).ok_or("No local draft")?;
            let current = snapshot.ciphers.get(&id.to_string()).ok_or("The server entry was deleted or access was removed; the encrypted draft is retained")?;
            if current.organization_id != edit.cipher.organization_id || current.r#type != edit.cipher.r#type {
                return Err("Ownership or item keys changed; this draft cannot be reapplied".into());
            }
            // A draft may itself have enabled reprompt after the server copy changed.
            if current.reprompt != bitwarden_vault::CipherRepromptType::Password && edit.cipher.reprompt == bitwarden_vault::CipherRepromptType::Password
                && !session.state.grants.permits(&super::Authorization { cipher: Some(id), revision: Some(current.revision_date), action: Action::Edit },session.generation,true) {
                return Err(super::authorization::REPROMPT_REQUIRED.into());
            }
            super::session_client::check_cipher(&client,&session,id,Action::Edit,None).await?;
            let draft = client.vault().ciphers().decrypt(edit.cipher.clone()).await.map_err(|_| "The local draft cannot be decrypted with the current item keys")?;
            let mut view = client.clone().full_cipher(id).await?;
            if view.key.is_some() && view.key != draft.key {
                return Err("The item key changed; this draft cannot be reapplied".into());
            }
            view.name = draft.name;
            view.notes = draft.notes;
            view.login = draft.login;
            view.card = draft.card;
            view.identity = draft.identity;
            view.secure_note = draft.secure_note;
            view.ssh_key = draft.ssh_key;
            view.bank_account = draft.bank_account;
            view.drivers_license = draft.drivers_license;
            view.passport = draft.passport;
            view.fields = draft.fields;
            view.favorite = draft.favorite;
            view.reprompt = draft.reprompt;
            view.folder_id = draft.folder_id.filter(|id| snapshot.folders.contains_key(&id.to_string()));
            Ok(view)
        }).await
    }

    pub async fn discard_local(self, id: CipherId) -> Result<(), String> {
        let session = self.session;
        let _coordinator = session.state.sync_gate.lock().await;
        let _lease = session.state.gate.write().await;
        session.ensure_unlocked()?;
        let client = session.state.client()?;
        let snapshot = session
            .state
            .snapshot
            .current()
            .ok_or("No server snapshot")?;
        if snapshot
            .persisted
            .local_edits
            .get(&id.to_string())
            .is_some_and(|edit| edit.state == snapshot::LocalEditState::Pending)
        {
            super::session_client::check_cipher(&client, &session, id, Action::Write, None).await?;
        }
        let next = snapshot.without_edit(id)?;
        session.ensure_unlocked()?;
        commit(&client, &next).await?;
        session.state.snapshot.install(next);
        session.state.grants.clear();
        session.ensure_unlocked()
    }
}

pub(super) async fn commit(
    client: &bitwarden_pm::PasswordManagerClient,
    next: &Arc<snapshot::Snapshot>,
) -> Result<(), String> {
    client
        .platform()
        .state()
        .setting(snapshot::CACHE)
        .map_err(|_| "Snapshot storage is unavailable")?
        .update(next.persisted.clone())
        .await
        .map_err(|_| "Local changes could not be saved; the previous state is intact".into())
}

/// Mirror the SDK edit path's history behavior; its helper is crate-private.
fn update_history(view: &mut CipherView, original: &CipherView) {
    use bitwarden_vault::{FieldType, PasswordHistoryView};
    let now = chrono::Utc::now();
    let mut changes = Vec::new();
    if let (Some(login), Some(previous)) = (&mut view.login, &original.login) {
        let old = previous.password.as_deref().unwrap_or("");
        let new = login.password.as_deref().unwrap_or("");
        if old != new {
            login.password_revision_date = Some(now);
            if !old.is_empty() {
                changes.push(PasswordHistoryView {
                    password: old.to_owned(),
                    last_used_date: now,
                });
            }
        } else {
            login.password_revision_date = previous.password_revision_date;
        }
    }
    let hidden = |view: &CipherView| -> std::collections::HashMap<String, String> {
        view.fields
            .iter()
            .flatten()
            .filter(|f| f.r#type == FieldType::Hidden)
            .filter_map(|f| Some((f.name.as_ref()?.clone(), f.value.as_ref()?.clone())))
            .filter(|(n, v)| !n.is_empty() && !v.is_empty())
            .collect()
    };
    let current = hidden(view);
    for (name, value) in hidden(original) {
        if current.get(&name) != Some(&value) {
            changes.push(PasswordHistoryView {
                password: format!("{name}: {value}"),
                last_used_date: now,
            });
        }
    }
    if !changes.is_empty() {
        view.password_history = Some(
            changes
                .into_iter()
                .rev()
                .chain(original.password_history.iter().flatten().cloned())
                .take(5)
                .collect(),
        );
    }
}
