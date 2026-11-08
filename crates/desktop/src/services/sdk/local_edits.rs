//! Durable encrypted local changes. No HTTP writes; full read-sync is paused
//! while changes exist, preserving the original server revision for future sync.
use super::{Action, ClientExt, SessionClient, snapshot};
use bitwarden_vault::EncryptionContext;
use bitwarden_vault::{CipherId, CipherView};
use std::sync::Arc;

impl SessionClient {
    pub(super) async fn save_local(self, mut view: CipherView) -> Result<CipherView, String> {
        let session = self.session;
        let _coordinator = session.state.sync_gate.lock().await;
        let _lease = session.state.gate.write().await;
        session.ensure_unlocked()?;
        let client = session.state.client()?;
        let id = view
            .id
            .ok_or("Creating server entries is not implemented yet")?;
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

    pub async fn discard_local(self, id: CipherId) -> Result<(), String> {
        let session = self.session;
        let _coordinator = session.state.sync_gate.lock().await;
        let _lease = session.state.gate.write().await;
        session.ensure_unlocked()?;
        let client = session.state.client()?;
        super::session_client::check_cipher(&client, &session, id, Action::Write, None).await?;
        let snapshot = session
            .state
            .snapshot
            .current()
            .ok_or("No server snapshot")?;
        let next = snapshot.without_edit(id)?;
        session.ensure_unlocked()?;
        commit(&client, &next).await?;
        session.state.snapshot.install(next);
        session.state.grants.clear();
        session.ensure_unlocked()
    }
}

async fn commit(
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
