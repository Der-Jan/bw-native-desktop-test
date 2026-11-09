//! Revocable SDK handles. Raw clients stay inside the SDK service.

use std::future::Future;

use bitwarden_core::OrganizationId;
use bitwarden_generators::{
    PassphraseGeneratorRequest, PasswordGeneratorRequest, UsernameGeneratorRequest,
};
use bitwarden_pm::PasswordManagerClient;
use bitwarden_vault::{CipherId, CipherListView, CipherView, FolderView};

use super::authorization::{PERMISSION_DENIED, REPROMPT_REQUIRED};
use super::{Action, Authorization, ClientExt, SessionToken};

pub struct SessionClient {
    pub(super) session: SessionToken,
}

impl SessionClient {
    /// Decrypt item and navigation metadata under the same revocable operation lease.
    pub async fn list_vault(self) -> Result<(Vec<CipherListView>, Vec<FolderView>), String> {
        let session = self.session.clone();
        self.run(move |client| async move {
            let items = permitted_list(&client, &session).await?;
            let folders = client.list_folders().await?;
            Ok((items, folders))
        })
        .await
    }
    pub fn session(&self) -> SessionToken {
        self.session.clone()
    }

    pub(super) async fn run<T, F, Fut>(self, operation: F) -> Result<T, String>
    where
        T: Send,
        F: FnOnce(PasswordManagerClient) -> Fut + Send,
        Fut: Future<Output = Result<T, String>> + Send,
    {
        let _lease = self.session.state.gate.read().await;
        self.session.ensure_unlocked()?;
        let result = operation(self.session.state.client()?).await;
        // Drop even a successful plaintext result if authority changed mid-call.
        self.session.ensure_unlocked()?;
        result
    }
}

macro_rules! forward {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $result:ty;)*) => {
        #[async_trait::async_trait]
        impl ClientExt for SessionClient {
            async fn full_cipher(self, cipher_id: CipherId) -> Result<CipherView,String> {
                self.full_cipher_for(cipher_id,Action::Read).await
            }
            async fn list_ciphers(self) -> Result<Vec<CipherListView>,String> {
                let session=self.session.clone();
                self.run(move |client|async move { permitted_list(&client,&session).await }).await
            }
            async fn generate_password(self, req: PasswordGeneratorRequest) -> Result<String,String> {
                let session=self.session.clone();
                self.run(move |client|async move {client.generate_password(session.generator_rules()?.password(req)?).await}).await
            }
            async fn generate_passphrase(self, req: PassphraseGeneratorRequest) -> Result<String,String> {
                let session=self.session.clone();
                self.run(move |client|async move {client.generate_passphrase(session.generator_rules()?.passphrase(req)?).await}).await
            }
            async fn save_cipher(self, cipher_view: CipherView) -> Result<CipherView,String> {
                if !self.session.state.writable.load(std::sync::atomic::Ordering::SeqCst) {
                    return self.save_local(cipher_view).await;
                }
                let session=self.session();
                self.run(move |client|async move {
                    ensure_writable(&session)?;
                    if !session.policy_rules().allows_write(cipher_view.r#type,cipher_view.organization_id) {return Err(super::policies::DENIED.into());}
                    if let Some(id)=cipher_view.id { check_cipher(&client,&session,id,Action::Write,None).await?; }
                    let result=client.save_cipher(cipher_view).await;
                    session.state.grants.clear();result
                }).await
            }
            async fn soft_delete_cipher(self,cipher_id:CipherId)->Result<(),String>{
                let session=self.session();self.run(move |client|async move {
                    ensure_writable(&session)?;check_cipher(&client,&session,cipher_id,Action::Delete,None).await?;
                    let result=client.soft_delete_cipher(cipher_id).await;session.state.grants.clear();result
                }).await
            }
            async fn create_folder(self,name:String)->Result<FolderView,String>{
                let session=self.session();self.run(move |client|async move {ensure_writable(&session)?;client.create_folder(name).await}).await
            }
            async fn export_organization_vault(self,organization_id:OrganizationId,format:bitwarden_exporters::ExportFormat)->Result<String,String>{
                let session=self.session();self.run(move|client|async move {
                    let deadline=check_export(&client,&session).await?;
                    let result=client.export_organization_vault(organization_id,format).await?;
                    if std::time::Instant::now()>=deadline{return Err(REPROMPT_REQUIRED.into())} Ok(result)
                }).await
            }
            async fn export_vault(self,format:bitwarden_exporters::ExportFormat)->Result<String,String>{
                let session=self.session();self.run(move |client|async move {
                    let deadline=check_export(&client,&session).await?;let result=client.export_vault(format).await?;
                    if std::time::Instant::now()>=deadline{return Err(REPROMPT_REQUIRED.into())} Ok(result)
                }).await
            }
            $(async fn $name(self, $($arg: $ty),*) -> Result<$result, String> {
                self.run(move |client| client.$name($($arg),*)).await
            })*
        }
    };
}

forward! {
    validate_master_password(encrypted_user_key: String, password: String) -> ();

    list_folders() -> Vec<FolderView>;

    generate_username(req: UsernameGeneratorRequest) -> String;
}

impl SessionClient {
    pub async fn full_cipher_for(
        self,
        cipher_id: CipherId,
        action: Action,
    ) -> Result<CipherView, String> {
        let session = self.session();
        self.run(move |client| async move {
            check_cipher(&client, &session, cipher_id, action, None).await?;
            client.full_cipher(cipher_id).await
        })
        .await
    }
    #[cfg(test)]
    pub async fn authorize_password(
        self,
        envelope: String,
        password: String,
        target: Authorization,
    ) -> Result<(), String> {
        self.authorize_password_scoped(
            envelope,
            password,
            target,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await
    }
    pub async fn authorize_password_scoped(
        self,
        envelope: String,
        password: String,
        target: Authorization,
        cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), String> {
        let session = self.session();
        self.run(move |client| async move {
            if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(REPROMPT_REQUIRED.into());
            }
            client
                .clone()
                .validate_master_password(envelope, password)
                .await?;
            if let Some(id) = target.cipher {
                let item = client
                    .platform()
                    .state()
                    .get::<bitwarden_vault::Cipher>()
                    .map_err(|_| PERMISSION_DENIED)?
                    .get(id)
                    .await
                    .map_err(|_| PERMISSION_DENIED)?
                    .ok_or(PERMISSION_DENIED)?;
                if Some(item.revision_date) != target.revision || !item.view_password {
                    return Err(PERMISSION_DENIED.into());
                }
                if matches!(target.action, Action::Edit | Action::Write | Action::Delete)
                    && !item.edit
                {
                    return Err(PERMISSION_DENIED.into());
                }
            }
            session.ensure_unlocked()?;
            if !session
                .state
                .grants
                .authorize_if(target, session.generation, &cancelled)
            {
                return Err(REPROMPT_REQUIRED.into());
            }
            Ok(())
        })
        .await
    }
    #[cfg(test)]
    pub async fn authorize_export(self, envelope: String, password: String) -> Result<(), String> {
        self.authorize_password(envelope, password, Authorization::export())
            .await
    }
    /// Stage plaintext output privately; publish only while authority is current.
    /// The read lease keeps lock completion behind the entire export, not just decryption.
    pub async fn export_to_path(
        self,
        organization_id: Option<OrganizationId>,
        format: bitwarden_exporters::ExportFormat,
        path: std::path::PathBuf,
    ) -> Result<String, String> {
        let session = self.session();
        self.run(move |client| async move {
            let deadline = check_export(&client, &session).await?;
            let content = zeroize::Zeroizing::new(match organization_id {
                None => client.export_vault(format).await?,
                Some(id) => client.export_organization_vault(id, format).await?,
            });
            stage_export(&session, &path, content.as_bytes(), deadline).await?;
            Ok(path.display().to_string())
        })
        .await
    }
}

async fn permitted_list(
    client: &PasswordManagerClient,
    session: &SessionToken,
) -> Result<Vec<CipherListView>, String> {
    let rules = session.policy_rules();
    let repo = client
        .platform()
        .state()
        .get::<bitwarden_vault::Cipher>()
        .map_err(|_| PERMISSION_DENIED)?;
    let ciphers = repo
        .list()
        .await
        .map_err(|_| PERMISSION_DENIED)?
        .into_iter()
        .filter(|c| rules.allows_read(c.r#type, c.organization_id))
        .collect();
    let result = client
        .vault()
        .ciphers()
        .decrypt_list_with_failures(ciphers)
        .await;
    if !result.failures.is_empty() {
        tracing::warn!(
            count = result.failures.len(),
            "some ciphers failed to decrypt"
        );
    }
    let mut items = result.successes;
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

fn ensure_writable(session: &SessionToken) -> Result<(), String> {
    if !session
        .state
        .writable
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err("This server operation is not implemented; existing item edits use revision-checked synchronization".into());
    }
    Ok(())
}
pub(super) async fn check_cipher(
    client: &PasswordManagerClient,
    session: &SessionToken,
    id: CipherId,
    action: Action,
    expected: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<(), String> {
    let item = client
        .platform()
        .state()
        .get::<bitwarden_vault::Cipher>()
        .map_err(|_| PERMISSION_DENIED)?
        .get(id)
        .await
        .map_err(|_| PERMISSION_DENIED)?
        .ok_or(PERMISSION_DENIED)?;
    let rules = session.policy_rules();
    if !rules.allows_read(item.r#type, item.organization_id)
        || (action == Action::Edit && !rules.allows_write(item.r#type, item.organization_id))
    {
        return Err(super::policies::DENIED.into());
    }
    if !item.view_password
        || (matches!(action, Action::Write | Action::Edit | Action::Delete) && !item.edit)
        || (action == Action::Delete && item.permissions.as_ref().is_some_and(|p| !p.delete))
        || expected.is_some_and(|r| r != item.revision_date)
    {
        return Err(PERMISSION_DENIED.into());
    }
    if item.reprompt == bitwarden_vault::CipherRepromptType::Password
        && !session.state.grants.permits(
            &Authorization {
                cipher: Some(id),
                revision: Some(item.revision_date),
                action,
            },
            session.generation,
            true,
        )
    {
        return Err(REPROMPT_REQUIRED.into());
    }
    Ok(())
}
async fn check_export(
    client: &PasswordManagerClient,
    session: &SessionToken,
) -> Result<std::time::Instant, String> {
    ensure_writable(session)?;
    let ciphers = client
        .platform()
        .state()
        .get::<bitwarden_vault::Cipher>()
        .map_err(|_| PERMISSION_DENIED)?
        .list()
        .await
        .map_err(|_| PERMISSION_DENIED)?;
    let rules = session.policy_rules();
    if ciphers
        .iter()
        .any(|c| !c.view_password || !rules.allows_read(c.r#type, c.organization_id))
    {
        return Err(PERMISSION_DENIED.into());
    }
    session
        .state
        .grants
        .take(&Authorization::export(), session.generation)
        .ok_or(REPROMPT_REQUIRED.into())
}

struct StagedExport(std::path::PathBuf);
impl Drop for StagedExport {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

async fn stage_export(
    session: &SessionToken,
    path: &std::path::Path,
    content: &[u8],
    deadline: std::time::Instant,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    session.ensure_unlocked()?;
    let staged = StagedExport(
        path.with_file_name(format!(".bitwarden-export-{}.tmp", uuid::Uuid::new_v4())),
    );
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&staged.0).await.map_err(|e| e.to_string())?;
    file.write_all(content).await.map_err(|e| e.to_string())?;
    file.sync_all().await.map_err(|e| e.to_string())?;
    drop(file);
    session.ensure_unlocked()?;
    if std::time::Instant::now() >= deadline {
        return Err(REPROMPT_REQUIRED.into());
    }
    tokio::fs::rename(&staged.0, path)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::sdk::{LockOperation, session::SessionState, test_support::raw_client};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    fn unlocked() -> (Arc<SessionState>, SessionClient) {
        let state = SessionState::new(
            "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            raw_client(),
        );
        let session = state.begin_unlock().unwrap();
        session.finish_unlock(true).unwrap();
        let client = SessionClient { session };
        (state, client)
    }

    #[tokio::test]
    async fn lock_drains_work_and_discards_plaintext_computed_after_revocation() {
        let (state, client) = unlocked();
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(client.run(move |_| async move {
            entered_tx.send(()).unwrap();
            release_rx.await.unwrap();
            Ok("synthetic secret".to_string())
        }));
        entered_rx.await.unwrap();
        let cleanup = LockOperation {
            session: state.revoke(false),
        };
        let cleanup = tokio::spawn(cleanup.finish());
        tokio::task::yield_now().await;
        assert!(
            !cleanup.is_finished(),
            "keys cannot be finalized while SDK work holds a lease"
        );
        release_tx.send(()).unwrap();
        assert!(
            task.await.unwrap().is_err(),
            "late plaintext must be dropped"
        );
        cleanup.await.unwrap();
        assert!(
            state.begin_unlock().is_some(),
            "unlock is enabled again after cleanup"
        );
    }

    #[tokio::test]
    async fn replacing_the_manager_revokes_sdk_handles_with_the_same_user_id() {
        let uid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
        let mut manager = crate::services::sdk::test_support::unlocked_manager(uid);
        let client = manager.client_for(&uid).unwrap();
        manager = crate::services::sdk::test_support::unlocked_manager(uid);
        assert!(manager.is_unlocked(&uid));
        let ran = Arc::new(AtomicBool::new(false));
        let flag = ran.clone();
        let result = client
            .run(move |_| async move {
                flag.store(true, Ordering::SeqCst);
                Ok(())
            })
            .await;
        assert!(result.is_err());
        assert!(!ran.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn work_queued_before_logout_never_executes_after_logout() {
        let (state, client) = unlocked();
        let ran = Arc::new(AtomicBool::new(false));
        let ran_in_task = Arc::clone(&ran);
        let exclusive = state.gate.write().await;
        let task = tokio::spawn(client.run(move |_| async move {
            ran_in_task.store(true, Ordering::SeqCst);
            Ok(())
        }));
        state.revoke(true);
        drop(exclusive);
        assert!(task.await.unwrap().is_err());
        assert!(!ran.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn revoked_export_cannot_overwrite_a_destination() {
        let (state, client) = unlocked();
        let path = std::env::temp_dir().join(format!("bw-export-test-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"existing file").unwrap();
        state.revoke(false);
        assert!(
            stage_export(
                &client.session,
                &path,
                b"new secret",
                std::time::Instant::now() + std::time::Duration::from_secs(60)
            )
            .await
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"existing file");
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn authorized_export_is_private_and_leaves_no_staging_file() {
        let (_state, client) = unlocked();
        let dir = std::env::temp_dir().join(format!("bw-export-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("export.json");
        stage_export(
            &client.session,
            &path,
            b"synthetic secret",
            std::time::Instant::now() + std::time::Duration::from_secs(60),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"synthetic secret");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
