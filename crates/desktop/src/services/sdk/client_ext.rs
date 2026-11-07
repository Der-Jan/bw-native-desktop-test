//! Extension trait that bundles every SDK call we make on a
//! [`PasswordManagerClient`]. Async tasks are dispatched from views as
//! `client.list_ciphers().await` etc. — the client is consumed by value
//! (cheap, since its inner is `Arc`-backed) so the resulting future is
//! `'static + Send` and slots straight into `Task::perform`.
//!
//! `unlock` lives on [`super::UnlockData`] instead because it needs more
//! than the client (kdf, email, key envelopes).

use std::sync::Arc;

use bitwarden_core::OrganizationId;
use bitwarden_generators::{
    PassphraseGeneratorRequest, PasswordGeneratorRequest, UsernameGeneratorRequest,
};
use bitwarden_pm::PasswordManagerClient;
use bitwarden_state::repository::Repository;
use bitwarden_vault::{
    Cipher, CipherId, CipherListView, CipherView, EncryptionContext, Folder, FolderView,
};

/// Acquire the per-user `Cipher` repository, with the registry's error
/// type stringified to match the rest of the trait.
fn cipher_repo(client: &PasswordManagerClient) -> Result<Arc<dyn Repository<Cipher>>, String> {
    client
        .platform()
        .state()
        .get::<Cipher>()
        .map_err(|e| e.to_string())
}

/// Acquire the per-user `Folder` repository, with the registry's error
/// type stringified to match the rest of the trait.
fn folder_repo(client: &PasswordManagerClient) -> Result<Arc<dyn Repository<Folder>>, String> {
    client
        .platform()
        .state()
        .get::<Folder>()
        .map_err(|e| e.to_string())
}

#[async_trait::async_trait]
pub trait ClientExt {
    /// Verify the user's master password against the cached user-key envelope
    /// without touching the keystore. Used by Export to gate vault data
    /// leaving the encrypted store behind a fresh master-password check.
    async fn validate_master_password(
        self,
        encrypted_user_key: String,
        password: String,
    ) -> Result<(), String>;

    /// Requires `unlock` to have been called first.
    async fn list_ciphers(self) -> Result<Vec<CipherListView>, String>;

    /// Fully decrypt a single cipher (including secrets). Used by the detail pane.
    async fn full_cipher(self, cipher_id: CipherId) -> Result<CipherView, String>;

    /// Encrypt an edited `CipherView` and persist to the local SQLite repo.
    /// Returns the re-decrypted view so callers can refresh with
    /// normalizations the encrypt step applied (cipher key, TOTP checksums, …).
    async fn save_cipher(self, cipher_view: CipherView) -> Result<CipherView, String>;

    /// Soft-delete by marking `deleted_date` and writing it back.
    async fn soft_delete_cipher(self, cipher_id: CipherId) -> Result<(), String>;

    /// Encrypt a fresh `FolderView` and write it into the user's local
    /// `Folder` repo. Skips the API call (same shape as `save_cipher`) since
    /// the fake-data harness has no remote.
    async fn create_folder(self, name: String) -> Result<FolderView, String>;

    async fn list_folders(self) -> Result<Vec<FolderView>, String>;

    /// Export the user's personal vault via the SDK's `ExporterClient`. The
    /// SDK decrypts internally using the unlocked keystore. Returns the
    /// serialized export string; caller writes to disk.
    async fn export_vault(
        self,
        format: bitwarden_exporters::ExportFormat,
    ) -> Result<String, String>;

    /// Stub — the SDK's `export_organization_vault` is currently `todo!()`
    /// in the pinned `bitwarden-exporters` rev.
    async fn export_organization_vault(
        self,
        organization_id: OrganizationId,
        format: bitwarden_exporters::ExportFormat,
    ) -> Result<String, String>;

    async fn generate_password(self, req: PasswordGeneratorRequest) -> Result<String, String>;
    async fn generate_passphrase(self, req: PassphraseGeneratorRequest) -> Result<String, String>;
    async fn generate_username(self, req: UsernameGeneratorRequest) -> Result<String, String>;
}

#[async_trait::async_trait]
impl ClientExt for PasswordManagerClient {
    async fn validate_master_password(
        self,
        encrypted_user_key: String,
        password: String,
    ) -> Result<(), String> {
        self.0
            .auth()
            .validate_password_user_key(password, encrypted_user_key)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn list_ciphers(self) -> Result<Vec<CipherListView>, String> {
        let result = self
            .vault()
            .ciphers()
            .list()
            .await
            .map_err(|e| e.to_string())?;

        if !result.failures.is_empty() {
            tracing::warn!(
                count = result.failures.len(),
                "some ciphers failed to decrypt"
            );
        }

        let mut list = result.successes;
        list.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(list)
    }

    async fn full_cipher(self, cipher_id: CipherId) -> Result<CipherView, String> {
        let repo = cipher_repo(&self)?;
        let cipher = repo
            .get(cipher_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("cipher {cipher_id} not found"))?;

        self.vault()
            .ciphers()
            .decrypt(cipher)
            .await
            .map_err(|e| e.to_string())
    }

    async fn save_cipher(self, mut cipher_view: CipherView) -> Result<CipherView, String> {
        // Mock-only: a real backend would assign the id on POST and return it.
        // With no server, generate one here so the repo has a key to store under.
        if cipher_view.id.is_none() {
            cipher_view.id = Some(CipherId::new_v4());
        }

        let EncryptionContext { cipher, .. } = self
            .vault()
            .ciphers()
            .encrypt(cipher_view)
            .await
            .map_err(|e| e.to_string())?;
        let id = cipher
            .id
            .ok_or_else(|| "encrypted cipher missing id".to_string())?;

        let repo = cipher_repo(&self)?;
        repo.set(id, cipher.clone())
            .await
            .map_err(|e| e.to_string())?;

        self.vault()
            .ciphers()
            .decrypt(cipher)
            .await
            .map_err(|e| e.to_string())
    }

    async fn soft_delete_cipher(self, cipher_id: CipherId) -> Result<(), String> {
        let repo = cipher_repo(&self)?;
        let mut cipher = repo
            .get(cipher_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("cipher {cipher_id} not found"))?;
        // `Cipher::soft_delete()` is `pub(crate)` in bitwarden-vault, but
        // `deleted_date` is public so we set it directly.
        cipher.deleted_date = Some(chrono::Utc::now());
        repo.set(cipher_id, cipher).await.map_err(|e| e.to_string())
    }

    async fn create_folder(self, name: String) -> Result<FolderView, String> {
        use bitwarden_vault::FolderId;

        let folder_id = FolderId::new_v4();
        let revision_date = chrono::Utc::now();
        let view = FolderView {
            id: Some(folder_id),
            name,
            revision_date,
        };

        // `FoldersClient::encrypt` is marked deprecated upstream in favour of
        // a higher-level `create()` that posts to the API — we want only the
        // encrypt step, since we're persisting locally.
        #[expect(deprecated)]
        let encrypted = self
            .vault()
            .folders()
            .encrypt(view)
            .map_err(|e| e.to_string())?;
        let id = encrypted
            .id
            .ok_or_else(|| "encrypted folder missing id".to_string())?;

        // Decrypt the encrypted folder back into a `FolderView` for the
        // return value. Cheaper than re-encrypting/cloning, and keeps the
        // returned name in sync with what was persisted.
        #[expect(deprecated)]
        let decrypted = self
            .vault()
            .folders()
            .decrypt(encrypted.clone())
            .map_err(|e| e.to_string())?;

        let repo = folder_repo(&self)?;
        repo.set(id, encrypted).await.map_err(|e| e.to_string())?;
        Ok(decrypted)
    }

    async fn list_folders(self) -> Result<Vec<FolderView>, String> {
        self.vault()
            .folders()
            .list()
            .await
            .map_err(|e| e.to_string())
    }

    async fn export_vault(
        self,
        format: bitwarden_exporters::ExportFormat,
    ) -> Result<String, String> {
        let ciphers = cipher_repo(&self)?
            .list()
            .await
            .map_err(|e| e.to_string())?;
        let folders = folder_repo(&self)?
            .list()
            .await
            .map_err(|e| e.to_string())?;

        self.exporters()
            .export_vault(folders, ciphers, format)
            .await
            .map_err(|e| e.to_string())
    }

    async fn export_organization_vault(
        self,
        _organization_id: OrganizationId,
        _format: bitwarden_exporters::ExportFormat,
    ) -> Result<String, String> {
        Err("Organization vault export isn't implemented in the SDK yet".to_string())
    }

    async fn generate_password(self, req: PasswordGeneratorRequest) -> Result<String, String> {
        self.generator().password(req).map_err(|e| e.to_string())
    }

    async fn generate_passphrase(self, req: PassphraseGeneratorRequest) -> Result<String, String> {
        self.generator().passphrase(req).map_err(|e| e.to_string())
    }

    /// SDK `username` is `async` because the `Forwarded` variant does HTTP;
    /// `Word`/`Subaddress`/`Catchall` resolve synchronously inside the future.
    async fn generate_username(self, req: UsernameGeneratorRequest) -> Result<String, String> {
        self.generator()
            .username(req)
            .await
            .map_err(|e| e.to_string())
    }
}
