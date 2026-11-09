//! SDK-encrypted Vaultwarden attachments; binary transfers use the same scoped
//! TLS/mTLS identity, never redirects, third-party storage or OAuth on file URLs.
use super::{Action, ClientExt, SessionClient, SessionToken, local_edits, refresh, vault_sync};
use crate::services::accounts::{
    profile::{Origin, ProfileStore},
    transport::{Transport, json_body_limited},
};
use bitwarden_api_api::models::{
    AttachmentRequestModel, AttachmentResponseModel, AttachmentUploadDataResponseModel,
    FileUploadType,
};
use bitwarden_pm::PasswordManagerClient;
use bitwarden_vault::{Attachment, AttachmentView, Cipher, CipherId};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
const MAX_FILE: usize = 500 * 1024 * 1024;
const MAX_ENCRYPTED: usize = MAX_FILE + 1024;
#[derive(Clone)]
pub enum Operation {
    Upload(PathBuf),
    Download { id: String, path: PathBuf },
    Delete(String),
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Intent {
    #[serde(default)]
    pub operation: uuid::Uuid,
    pub cipher: CipherId,
    pub attachment: Attachment,
    pub digest: Option<Vec<u8>>,
    pub data_sent: bool,
    pub deleting: bool,
}
pub struct Outcome {
    pub report: vault_sync::Report,
    pub applied: bool,
}
fn valid_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("Invalid attachment identifier".into());
    }
    Ok(())
}
fn api_url(authority: &refresh::Authorized, path: &str) -> Result<url::Url, String> {
    url::Url::parse(&authority.record.api)
        .and_then(|u| u.join(path))
        .map_err(|_| "Invalid attachment endpoint".into())
}
fn file_url(
    transport: &Transport,
    cipher: CipherId,
    id: &str,
    value: &str,
) -> Result<url::Url, String> {
    valid_id(id)?;
    let base = url::Url::parse(&transport.profile.base).map_err(|_| "Invalid account endpoint")?;
    let expected = base
        .join(&format!("attachments/{cipher}/{id}"))
        .map_err(|_| "Invalid attachment endpoint")?;
    let url = url::Url::parse(value).map_err(|_| "Invalid attachment download URL")?;
    if Origin::of(&url)? != Origin::of(&base)?
        || url.path() != expected.path()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(
            "The attachment URL does not belong to this account's trusted file endpoint".into(),
        );
    }
    transport.check(&url)?;
    Ok(url)
}
async fn revocable<T>(
    session: &SessionToken,
    future: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    session.ensure_unlocked()?;
    tokio::select! {
        result=future=>result,
        error=async {loop {tokio::time::sleep(Duration::from_millis(100)).await;if let Err(error)=session.ensure_unlocked() {break error;}}}=>Err(error),
    }
}
async fn bytes_limited(
    session: &SessionToken,
    mut response: reqwest_account::Response,
) -> Result<Vec<u8>, String> {
    if !response.status().is_success() {
        return Err("The server could not provide the attachment file".into());
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_ENCRYPTED as u64)
    {
        return Err("Attachment exceeds the 500 MiB limit".into());
    }
    let mut data = Vec::new();
    while let Some(chunk) = revocable(session, async {
        response
            .chunk()
            .await
            .map_err(|_| "Attachment transfer was interrupted".to_owned())
    })
    .await?
    {
        if data.len().saturating_add(chunk.len()) > MAX_ENCRYPTED {
            return Err("Attachment exceeds the 500 MiB limit".into());
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}
async fn encrypted_download(
    session: &SessionToken,
    authority: &refresh::Authorized,
    cipher: CipherId,
    id: &str,
) -> Result<Vec<u8>, String> {
    valid_id(id)?;
    session.ensure_unlocked()?;
    let endpoint = api_url(authority, &format!("ciphers/{cipher}/attachment/{id}"))?;
    let response = authority
        .transport
        .execute(
            authority
                .transport
                .request(reqwest_account::Method::GET, endpoint)?
                .bearer_auth(&authority.credentials.access_token)
                .build()
                .map_err(|_| "Could not prepare attachment lookup")?,
        )
        .await?;
    if !response.status().is_success() {
        return Err("Attachment is unavailable or access was removed".into());
    }
    let model: AttachmentResponseModel =
        serde_json::from_value(json_body_limited(response, 32 * 1024).await?)
            .map_err(|_| "Invalid attachment metadata")?;
    if model.id.as_deref() != Some(id) {
        return Err("The server returned a different attachment".into());
    }
    let url = file_url(
        &authority.transport,
        cipher,
        id,
        model.url.as_deref().ok_or("Missing attachment URL")?,
    )?;
    session.ensure_unlocked()?;
    // The signed file URL needs no OAuth token. mTLS and origin checks still apply.
    let request = authority
        .transport
        .request(reqwest_account::Method::GET, url.clone())?
        .timeout(Duration::from_secs(900))
        .build()
        .map_err(|_| "Could not prepare attachment download")?;
    let data = bytes_limited(
        session,
        revocable(session, authority.transport.execute(request)).await?,
    )
    .await?;
    session.ensure_unlocked()?;
    authority.transport.check(&url)?;
    Ok(data)
}
async fn install_intent(
    session: &SessionToken,
    client: &PasswordManagerClient,
    intent: Option<Intent>,
) -> Result<(), String> {
    let snapshot = session
        .state
        .snapshot
        .current()
        .ok_or("No synchronized vault")?;
    let mut persisted = snapshot.persisted.clone();
    persisted.attachment_intent = intent;
    let next = snapshot.rebuild(persisted)?;
    session.ensure_unlocked()?;
    local_edits::commit(client, &next).await?;
    session.state.snapshot.install(next);
    session.ensure_unlocked()
}
/// Read-only outcome reconciliation; never recreate a slot or replay a file upload.
pub(super) async fn reconcile(
    session: &SessionToken,
    client: &PasswordManagerClient,
    root: PathBuf,
    store: ProfileStore,
) -> Result<bool, String> {
    let snapshot = session
        .state
        .snapshot
        .current()
        .ok_or("No synchronized vault")?;
    let Some(mut intent) = snapshot.persisted.attachment_intent.clone() else {
        return Ok(true);
    };
    let cipher = snapshot.ciphers.get(&intent.cipher.to_string());
    if intent.deleting {
        if cipher.is_none_or(|c| {
            !c.attachments
                .iter()
                .flatten()
                .any(|a| a.id == intent.attachment.id)
        }) {
            install_intent(session, client, None).await?;
            return Ok(true);
        }
        return Ok(false);
    }
    let Some(cipher) = cipher else {
        return Ok(false);
    };
    if !cipher.view_password
        || !snapshot
            .policies
            .allows_read(cipher.r#type, cipher.organization_id)
    {
        return Ok(false);
    }
    let matched = cipher
        .attachments
        .iter()
        .flatten()
        .find(|a| a.file_name == intent.attachment.file_name && a.key == intent.attachment.key);
    let Some(attachment) = matched else {
        return Ok(false);
    };
    if intent.attachment.id != attachment.id {
        intent.attachment.id = attachment.id.clone();
        install_intent(session, client, Some(intent.clone())).await?;
    }
    if !intent.data_sent {
        return Ok(false);
    }
    let id = attachment.id.as_deref().ok_or("Attachment ID missing")?;
    let authority = refresh::authorized(session, root, store, false).await?;
    let data = encrypted_download(session, &authority, intent.cipher, id).await?;
    if intent.digest.as_deref() != Some(openssl::sha::sha256(&data).as_slice()) {
        return Err(
            "The uploaded file is not confirmed. The encrypted upload record is retained.".into(),
        );
    }
    install_intent(session, client, None).await?;
    Ok(true)
}
fn read_source(path: &Path) -> Result<Zeroizing<Vec<u8>>, String> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|_| "Could not open the selected file")?;
    let meta = file
        .metadata()
        .map_err(|_| "Could not inspect the selected file")?;
    if !meta.is_file() || meta.len() > MAX_FILE as u64 {
        return Err("Select a regular file no larger than 500 MiB".into());
    }
    let mut contents = Zeroizing::new(Vec::new());
    file.take((MAX_FILE + 1) as u64)
        .read_to_end(&mut contents)
        .map_err(|_| "Could not read the selected file")?;
    if contents.len() > MAX_FILE {
        return Err("Attachment exceeds the 500 MiB limit".into());
    }
    Ok(contents)
}
pub async fn perform(
    handle: SessionClient,
    cipher_id: CipherId,
    operation: Operation,
    root: PathBuf,
    store: ProfileStore,
) -> Result<Outcome, String> {
    let session = handle.session();
    let _coordinator = session.state.sync_gate.lock().await;
    let exclusive = session.state.gate.write().await;
    session.ensure_unlocked()?;
    let client = session.state.client()?;
    let operation_id = uuid::Uuid::new_v4();
    let mutating = !matches!(&operation, Operation::Download { .. });
    let action = if matches!(&operation, Operation::Download { .. }) {
        Action::Copy
    } else {
        Action::Write
    };
    super::session_client::check_cipher(&client, &session, cipher_id, action, None).await?;
    let before = session
        .state
        .snapshot
        .current()
        .ok_or("Synchronize first")?;
    if before
        .persisted
        .archive_intent
        .as_ref()
        .is_some_and(|intent| intent.cipher == cipher_id)
    {
        return Err("Check this item's unconfirmed change before managing attachments.".into());
    }
    if before
        .persisted
        .local_edits
        .contains_key(&cipher_id.to_string())
    {
        return Err("Synchronize this item's local changes before managing attachments".into());
    }
    let original = before
        .ciphers
        .get(&cipher_id.to_string())
        .cloned()
        .ok_or("Item no longer exists")?;
    let (fresh, record, transport) =
        vault_sync::fetch(&session, root.clone(), store.clone()).await?;
    vault_sync::apply(&session, &client, fresh, &record, &transport).await?;
    let current = session
        .state
        .snapshot
        .current()
        .ok_or("No synchronized vault")?;
    let validation = (|| {
        let cipher = current
            .ciphers
            .get(&cipher_id.to_string())
            .ok_or("The item was removed on the server")?;
        if cipher.revision_date != original.revision_date
            || cipher.key != original.key
            || cipher.organization_id != original.organization_id
            || cipher.reprompt != original.reprompt
            || cipher.deleted_date.is_some()
            || !cipher.view_password
            || !current
                .policies
                .allows_read(cipher.r#type, cipher.organization_id)
            || (action == Action::Write
                && (!cipher.edit
                    || !current
                        .policies
                        .allows_write(cipher.r#type, cipher.organization_id)))
        {
            return Err(
                "Item permissions or content changed. Review the current entry before retrying."
                    .to_owned(),
            );
        }
        if let Some(pending) = &current.persisted.attachment_intent
            && !matches!(&operation, Operation::Download { .. })
            && !matches!(&operation, Operation::Delete(id) if pending.cipher == cipher_id && pending.attachment.id.as_ref() == Some(id))
        {
            return Err("Another attachment operation is unconfirmed. Verify or remove the incomplete attachment first.".to_owned());
        }
        Ok(cipher.clone())
    })();
    // Keep reads responsive during long file transfers. The coordinator still
    // serializes mutations; the read lease keeps keys alive and lock revocable.
    drop(exclusive);
    let read_lease = session.state.gate.read().await;
    session.ensure_unlocked()?;
    let result = match validation {
        Ok(cipher) => {
            dispatch(
                &session,
                &client,
                cipher,
                operation_id,
                operation,
                root.clone(),
                store.clone(),
            )
            .await
        }
        Err(error) => Err(error),
    };
    drop(read_lease);
    let _exclusive = session.state.gate.write().await;
    session.ensure_unlocked()?;
    let ours = mutating
        && session.state.snapshot.current().is_some_and(|s| {
            s.persisted
                .attachment_intent
                .as_ref()
                .is_some_and(|i| i.operation == operation_id)
        });
    let mut problems = Vec::new();
    let mut applied = result.is_ok();
    if let Err(error) = result {
        problems.push(error);
    }
    match vault_sync::fetch(&session, root.clone(), store.clone()).await {
        Ok((fresh, record, transport)) => {
            if let Err(error) =
                vault_sync::apply(&session, &client, fresh, &record, &transport).await
            {
                problems.push(error);
            }
        }
        Err(error) => problems.push(error),
    }
    let pending = session
        .state
        .snapshot
        .current()
        .is_some_and(|s| s.persisted.attachment_intent.is_some());
    if pending && ours {
        match reconcile(&session, &client, root, store).await {
            Ok(true) => {
                applied = true;
                problems.clear();
            }
            Ok(false) => {}
            Err(error) => problems.push(error),
        }
    }
    let snapshot = session
        .state
        .snapshot
        .current()
        .ok_or("No synchronized vault")?;
    let token = session.advance_snapshot(snapshot.collection_names(&client)?)?;
    let mut report = vault_sync::report(token)?;
    report.problems = problems;
    Ok(Outcome { report, applied })
}
async fn dispatch(
    session: &SessionToken,
    client: &PasswordManagerClient,
    cipher: Cipher,
    operation_id: uuid::Uuid,
    operation: Operation,
    root: PathBuf,
    store: ProfileStore,
) -> Result<(), String> {
    let cid = cipher.id.ok_or("Missing item ID")?;
    let authority = refresh::authorized(session, root, store, false).await?;
    match operation {
        Operation::Upload(path) => {
            let filename = path
                .file_name()
                .and_then(|v| v.to_str())
                .filter(|v| !v.is_empty())
                .ok_or("The file name is invalid")?
                .to_owned();
            let crypto = PasswordManagerClient(client.0.clone());
            let crypt_cipher = cipher.clone();
            let encrypted = tokio::task::spawn_blocking(move || {
                let contents = read_source(&path)?;
                let attachment = AttachmentView {
                    id: None,
                    url: None,
                    size: None,
                    size_name: None,
                    file_name: Some(filename),
                    key: None,
                };
                crypto
                    .vault()
                    .attachments()
                    .encrypt_buffer(crypt_cipher, attachment, &contents)
                    .map_err(|_| "Could not encrypt the attachment".to_owned())
            })
            .await
            .map_err(|_| "Attachment encryption failed")??;
            session.ensure_unlocked()?;
            let digest = openssl::sha::sha256(&encrypted.contents).to_vec();
            let mut intent = Intent {
                operation: operation_id,
                cipher: cid,
                attachment: encrypted.attachment.clone(),
                digest: Some(digest),
                data_sent: false,
                deleting: false,
            };
            install_intent(session, client, Some(intent.clone())).await?;
            let metadata = AttachmentRequestModel {
                key: encrypted.attachment.key.as_ref().map(ToString::to_string),
                file_name: encrypted
                    .attachment
                    .file_name
                    .as_ref()
                    .map(ToString::to_string),
                file_size: Some(encrypted.contents.len() as i64),
                admin_request: Some(false),
                last_known_revision_date: Some(cipher.revision_date.to_rfc3339()),
            };
            let url = api_url(&authority, &format!("ciphers/{cid}/attachment/v2"))?;
            let response = authority
                .transport
                .execute(
                    authority
                        .transport
                        .request(reqwest_account::Method::POST, url)?
                        .bearer_auth(&authority.credentials.access_token)
                        .json(&metadata)
                        .build()
                        .map_err(|_| "Could not prepare attachment creation")?,
                )
                .await?;
            if !response.status().is_success() {
                if matches!(
                    response.status().as_u16(),
                    400 | 401 | 403 | 404 | 409 | 422
                ) {
                    install_intent(session, client, None).await?;
                }
                return Err(
                    "The server rejected the attachment. Check storage limits and permissions."
                        .into(),
                );
            }
            let model: AttachmentUploadDataResponseModel =
                serde_json::from_value(json_body_limited(response, 8 * 1024 * 1024).await?)
                    .map_err(|_| "Unverified attachment creation reply")?;
            let id = model.attachment_id.ok_or("No attachment ID was returned")?;
            valid_id(&id)?;
            if model.file_upload_type != Some(FileUploadType::Direct) {
                return Err("Only Vaultwarden direct attachment storage is supported".into());
            }
            let endpoint = format!("ciphers/{cid}/attachment/{id}");
            let expected = api_url(&authority, &endpoint)?;
            let returned = model.url.ok_or("No upload endpoint was returned")?;
            if returned != format!("/{endpoint}") && returned != expected.as_str() {
                return Err(
                    "The attachment upload endpoint does not match the trusted account API".into(),
                );
            }
            intent.attachment.id = Some(id);
            intent.data_sent = true;
            install_intent(session, client, Some(intent)).await?;
            let form = reqwest_account::multipart::Form::new().part(
                "data",
                reqwest_account::multipart::Part::bytes(encrypted.contents).file_name("encrypted"),
            );
            let request = authority
                .transport
                .request(reqwest_account::Method::POST, expected)?
                .bearer_auth(&authority.credentials.access_token)
                .multipart(form)
                .timeout(Duration::from_secs(900))
                .build()
                .map_err(|_| "Could not prepare encrypted file upload")?;
            session.ensure_unlocked()?;
            let response = revocable(session, authority.transport.execute(request)).await?;
            session.ensure_unlocked()?;
            if !response.status().is_success() {
                return Err("The file upload was not confirmed. Check or remove the incomplete attachment before retrying.".into());
            }
            install_intent(session, client, None).await
        }
        Operation::Delete(id) => {
            valid_id(&id)?;
            let attachment = cipher
                .attachments
                .iter()
                .flatten()
                .find(|a| a.id.as_deref() == Some(&id))
                .cloned()
                .ok_or("Attachment no longer exists")?;
            install_intent(
                session,
                client,
                Some(Intent {
                    operation: operation_id,
                    cipher: cid,
                    attachment,
                    digest: None,
                    data_sent: false,
                    deleting: true,
                }),
            )
            .await?;
            let url = api_url(&authority, &format!("ciphers/{cid}/attachment/{id}"))?;
            session.ensure_unlocked()?;
            let response = authority
                .transport
                .execute(
                    authority
                        .transport
                        .request(reqwest_account::Method::DELETE, url)?
                        .bearer_auth(&authority.credentials.access_token)
                        .build()
                        .map_err(|_| "Could not prepare attachment deletion")?,
                )
                .await?;
            session.ensure_unlocked()?;
            if !response.status().is_success() {
                return Err("The server did not confirm attachment deletion".into());
            }
            install_intent(session, client, None).await
        }
        Operation::Download { id, path } => {
            valid_id(&id)?;
            let deadline = Instant::now()
                + if cipher.reprompt == bitwarden_vault::CipherRepromptType::Password {
                    Duration::from_secs(60)
                } else {
                    Duration::from_secs(900)
                };
            let view = client.clone().full_cipher(cid).await?;
            let attachment = view
                .attachments
                .iter()
                .flatten()
                .find(|a| a.id.as_deref() == Some(&id))
                .cloned()
                .ok_or("Attachment cannot be decrypted")?;
            let data = encrypted_download(session, &authority, cid, &id).await?;
            session.ensure_unlocked()?;
            let crypto = PasswordManagerClient(client.0.clone());
            let plaintext = tokio::task::spawn_blocking(move || {
                crypto
                    .vault()
                    .attachments()
                    .decrypt_buffer(cipher, attachment, &data)
                    .map(Zeroizing::new)
                    .map_err(|_| "Attachment authentication or decryption failed".to_owned())
            })
            .await
            .map_err(|_| "Attachment decryption failed")??;
            session.ensure_unlocked()?;
            authority
                .transport
                .check(&api_url(&authority, "ciphers")?)?;
            super::session_client::stage_export(session, &path, &plaintext, deadline).await
        }
    }
}
impl SessionClient {
    pub async fn dismiss_attachment_intent(self) -> Result<(), String> {
        let session = self.session;
        let _coordinator = session.state.sync_gate.lock().await;
        let _lease = session.state.gate.write().await;
        session.ensure_unlocked()?;
        install_intent(&session, &session.state.client()?, None).await
    }
}
