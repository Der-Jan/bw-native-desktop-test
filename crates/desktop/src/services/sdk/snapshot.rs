//! One SQLite setting is the authoritative encrypted snapshot. SDK-facing read-only
//! repositories share one Arc swap, so folders/ciphers never come from mixed commits.
use super::{
    SessionToken,
    types::{Collection, Organization},
};
use bitwarden_api_api::models::SyncResponseModel;
use bitwarden_core::{OrganizationId, UserId, key_management::SymmetricKeySlotId};
use bitwarden_pm::PasswordManagerClient;
use bitwarden_state::{
    registry::StateRegistry,
    repository::{Repository, RepositoryError},
};
use bitwarden_vault::{Cipher, Folder};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Persisted {
    version: u8,
    server_user_id: UserId,
    local_account_id: UserId,
    pub downloaded_at: DateTime<Utc>,
    pub response: SyncResponseModel,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub local_edits: HashMap<String, LocalEdit>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct LocalEdit {
    #[serde(default)]
    pub state: LocalEditState,
    pub cipher: Cipher,
    pub base_revision: DateTime<Utc>,
}
#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum LocalEditState {
    #[default]
    Pending,
    Conflict,
    // Persisted before dispatch: cancellation/crash/timeout must never replay blindly.
    Uncertain,
}
bitwarden_state::register_setting_key!(pub(super) const CACHE:Persisted="native_encrypted_vault_snapshot_v1");
pub(super) struct Snapshot {
    pub policies: Arc<super::policies::Rules>,
    pub persisted: Persisted,
    pub ciphers: HashMap<String, Cipher>,
    pub folders: HashMap<String, Folder>,
    pub organizations: Vec<Organization>,
    pub collections: Vec<bitwarden_collections::collection::Collection>,
}
#[derive(Clone, Default)]
pub(super) struct State(pub Arc<Mutex<Option<Arc<Snapshot>>>>);
impl State {
    pub fn current(&self) -> Option<Arc<Snapshot>> {
        self.0.lock().unwrap().clone()
    }
    pub fn install(&self, snapshot: Arc<Snapshot>) {
        *self.0.lock().unwrap() = Some(snapshot);
    }
}
impl Snapshot {
    pub fn prepare(
        response: SyncResponseModel,
        server_user_id: UserId,
        local_account_id: UserId,
        downloaded_at: DateTime<Utc>,
    ) -> Result<Arc<Self>, String> {
        Self::restore(
            Persisted {
                version: 1,
                server_user_id,
                local_account_id,
                downloaded_at,
                response,
                local_edits: HashMap::new(),
            },
            server_user_id,
            local_account_id,
        )
    }
    pub fn restore(
        persisted: Persisted,
        expected: UserId,
        local_account_id: UserId,
    ) -> Result<Arc<Self>, String> {
        if !matches!(persisted.version, 1 | 2)
            || persisted.server_user_id != expected
            || persisted.local_account_id != local_account_id
        {
            return Err("Ungültige Snapshot-Version oder Kontozuordnung".into());
        }
        let response = &persisted.response;
        let profile = response
            .profile
            .as_ref()
            .ok_or("Die Sync-Antwort enthält kein Kontoprofil")?;
        if profile.id.map(UserId::new) != Some(expected) {
            return Err("Die Sync-Antwort gehört zu einem anderen Konto".into());
        }
        let mut organizations = Vec::new();
        let mut org_ids = HashSet::new();
        for org in profile
            .organizations_new
            .as_ref()
            .or(profile.organizations.as_ref())
            .ok_or("Die Sync-Antwort enthält keine Organisationsliste")?
        {
            let id = OrganizationId::new(org.id.ok_or("Organisations-ID fehlt")?);
            if !org_ids.insert(id) {
                return Err("Doppelte Organisations-ID in der Sync-Antwort".into());
            }
            // Invited/accepted memberships can have no key; never borrow an old one.
            let wrapped_key = org
                .key
                .as_ref()
                .map(|key| {
                    key.parse()
                        .map_err(|_| "Ungültiger verschlüsselter Organisationsschlüssel")
                })
                .transpose()?;
            organizations.push(Organization {
                id,
                name: org.name.clone().ok_or("Organisationsname fehlt")?,
                wrapped_key,
            });
        }
        let policies = super::policies::Rules::from_response(response)?;
        let mut ciphers = HashMap::new();
        for model in response
            .ciphers
            .as_ref()
            .ok_or("Die Sync-Antwort enthält keine Tresoreinträge")?
        {
            let cipher=Cipher::try_from(model.clone()).map_err(|_|"Nicht unterstützter oder ungültiger Tresoreintrag. Der bisherige Snapshot bleibt erhalten.")?;
            let id = cipher.id.ok_or("Tresoreintrag ohne ID")?.to_string();
            if cipher
                .organization_id
                .is_some_and(|org| !org_ids.contains(&org))
            {
                return Err("Tresoreintrag verweist auf eine unbekannte Organisation".into());
            }
            if ciphers.insert(id, cipher).is_some() {
                return Err("Doppelte Tresoreintrags-ID".into());
            }
        }
        let mut folders = HashMap::new();
        for model in response
            .folders
            .as_ref()
            .ok_or("Die Sync-Antwort enthält keine Ordnerliste")?
        {
            let folder =
                Folder::try_from(model.clone()).map_err(|_| "Ungültiger verschlüsselter Ordner")?;
            let id = folder.id.ok_or("Ordner ohne ID")?.to_string();
            if folders.insert(id, folder).is_some() {
                return Err("Doppelte Ordner-ID".into());
            }
        }
        let mut collections = Vec::new();
        let mut collection_ids = HashSet::new();
        for model in response
            .collections
            .as_ref()
            .ok_or("Die Sync-Antwort enthält keine Sammlungsliste")?
        {
            let mut model = model.clone();
            // Classic server collections predate this field; they are organization-owned shared collections.
            if model.r#type.is_none() {
                model.r#type = Some(bitwarden_api_api::models::CollectionType::SharedCollection);
            }
            let collection = bitwarden_collections::collection::Collection::try_from(model)
                .map_err(|_| "Ungültige verschlüsselte Sammlung")?;
            let id = collection.id.ok_or("Sammlung ohne ID")?;
            if !org_ids.contains(&collection.organization_id) || !collection_ids.insert(id) {
                return Err("Ungültige Sammlung/Kontozuordnung".into());
            }
            collections.push(collection);
        }
        for (id, edit) in &persisted.local_edits {
            if edit.cipher.id.map(|id| id.to_string()).as_ref() != Some(id) {
                return Err("Invalid local draft identity".into());
            }
            if edit.state != LocalEditState::Pending {
                continue; // Conflicting drafts are retained separately, never served as current items.
            }
            let original = ciphers.get(id).ok_or("Local edit has no server entry")?;
            if edit.cipher.id != original.id
                || edit.cipher.organization_id != original.organization_id
                || edit.cipher.collection_ids != original.collection_ids
                || edit.cipher.r#type != original.r#type
                || edit.base_revision != original.revision_date
                || edit
                    .cipher
                    .folder_id
                    .is_some_and(|id| !folders.contains_key(&id.to_string()))
            {
                return Err("Invalid local edit or account ownership".into());
            }
            let mut cipher = edit.cipher.clone();
            cipher.edit = original.edit;
            cipher.view_password = original.view_password;
            cipher.permissions = original.permissions.clone();
            cipher.organization_use_totp = original.organization_use_totp;
            ciphers.insert(id.clone(), cipher);
        }
        Ok(Arc::new(Self {
            policies,
            persisted,
            ciphers,
            folders,
            organizations,
            collections,
        }))
    }
    pub fn with_edit(&self, cipher: Cipher) -> Result<Arc<Self>, String> {
        let id = cipher
            .id
            .ok_or("Local edits require an existing entry")?
            .to_string();
        let original = self.ciphers.get(&id).ok_or("Item no longer exists")?;
        let mut persisted = self.persisted.clone();
        persisted.version = 2;
        let base_revision = persisted
            .local_edits
            .get(&id)
            .filter(|edit| edit.state == LocalEditState::Pending)
            .map_or(original.revision_date, |edit| edit.base_revision);
        persisted.local_edits.insert(
            id,
            LocalEdit {
                state: LocalEditState::Pending,
                cipher,
                base_revision,
            },
        );
        Self::restore(
            persisted,
            self.persisted.server_user_id,
            self.persisted.local_account_id,
        )
    }
    pub fn without_edit(&self, id: bitwarden_vault::CipherId) -> Result<Arc<Self>, String> {
        let mut persisted = self.persisted.clone();
        persisted.version = 2;
        if persisted.local_edits.remove(&id.to_string()).is_none() {
            return Err("This item has no local changes".into());
        }
        Self::restore(
            persisted,
            self.persisted.server_user_id,
            self.persisted.local_account_id,
        )
    }
    pub fn rebuild(&self, persisted: Persisted) -> Result<Arc<Self>, String> {
        Self::restore(
            persisted,
            self.persisted.server_user_id,
            self.persisted.local_account_id,
        )
    }
    pub fn collection_names(
        &self,
        client: &PasswordManagerClient,
    ) -> Result<Vec<Collection>, String> {
        client
            .collections()
            .decrypt_list(self.collections.clone())
            .map_err(|_| "Sammlungen konnten nicht entschlüsselt werden")?
            .into_iter()
            .map(|view| {
                Ok(Collection {
                    id: view.id.ok_or("Sammlungs-ID fehlt")?,
                    organization_id: view.organization_id,
                    name: view.name,
                })
            })
            .collect()
    }
}
/// SDK org initialization does not clear keys for an empty membership set. Clear
/// explicitly so removal of the last organization also revokes its old key.
pub(super) fn org_keys(
    client: &PasswordManagerClient,
    organizations: &[Organization],
) -> Result<(), String> {
    client
        .0
        .internal
        .get_key_store()
        .context_mut()
        .retain_symmetric_keys(|id| !matches!(id, SymmetricKeySlotId::Organization(_)));
    client
        .0
        .internal
        .initialize_org_crypto(
            organizations
                .iter()
                .filter_map(|o| o.wrapped_key.clone().map(|key| (o.id, key)))
                .collect(),
        )
        .map_err(|_| "Organisationsschlüssel konnten nicht entschlüsselt werden".into())
}
/// Cancellation and validation/storage failure restore old keys before releasing
/// the exclusive lease. After revocation, clear keys rather than restoring them.
pub(super) struct OrgRollback {
    client: PasswordManagerClient,
    session: SessionToken,
    old: Vec<Organization>,
    pub committed: bool,
}
impl OrgRollback {
    pub fn new(
        client: &PasswordManagerClient,
        session: SessionToken,
        old: Vec<Organization>,
    ) -> Self {
        Self {
            client: PasswordManagerClient(client.0.clone()),
            session,
            old,
            committed: false,
        }
    }
}
impl Drop for OrgRollback {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        if self.session.ensure_unlocked().is_err() || org_keys(&self.client, &self.old).is_err() {
            super::session::clear_keys(&self.client);
        }
    }
}

macro_rules! read_repo {
    ($repo:ident,$ty:ty,$field:ident) => {
        struct $repo(State);
        #[async_trait::async_trait]
        impl Repository<$ty> for $repo {
            async fn get(
                &self,
                key: <$ty as bitwarden_state::repository::RepositoryItem>::Key,
            ) -> Result<Option<$ty>, RepositoryError> {
                Ok(self
                    .0
                    .current()
                    .and_then(|s| s.$field.get(&key.to_string()).cloned()))
            }
            async fn list(&self) -> Result<Vec<$ty>, RepositoryError> {
                Ok(self
                    .0
                    .current()
                    .map(|s| s.$field.values().cloned().collect())
                    .unwrap_or_default())
            }
            async fn set(
                &self,
                _key: <$ty as bitwarden_state::repository::RepositoryItem>::Key,
                _value: $ty,
            ) -> Result<(), RepositoryError> {
                Err(read_only())
            }
            async fn set_bulk(
                &self,
                _values: Vec<(
                    <$ty as bitwarden_state::repository::RepositoryItem>::Key,
                    $ty,
                )>,
            ) -> Result<(), RepositoryError> {
                Err(read_only())
            }
            async fn remove(
                &self,
                _key: <$ty as bitwarden_state::repository::RepositoryItem>::Key,
            ) -> Result<(), RepositoryError> {
                Err(read_only())
            }
            async fn remove_bulk(
                &self,
                _keys: Vec<<$ty as bitwarden_state::repository::RepositoryItem>::Key>,
            ) -> Result<(), RepositoryError> {
                Err(read_only())
            }
            async fn remove_all(&self) -> Result<(), RepositoryError> {
                Err(read_only())
            }
        }
    };
}
fn read_only() -> RepositoryError {
    RepositoryError::Internal("Der synchronisierte Tresor ist schreibgeschützt".into())
}
read_repo!(CipherRepo, Cipher, ciphers);
read_repo!(FolderRepo, Folder, folders);
pub(super) async fn register(
    registry: &StateRegistry,
    server_user_id: UserId,
    local_account_id: UserId,
) -> Result<State, String> {
    let state = State::default();
    if let Some(persisted) = registry
        .setting(CACHE)
        .map_err(|_| "Snapshot-Speicher fehlt")?
        .get()
        .await
        .map_err(|_| "Der lokale Snapshot konnte nicht gelesen werden")?
    {
        state.install(Snapshot::restore(
            persisted,
            server_user_id,
            local_account_id,
        )?);
    }
    registry.register_client_managed(Arc::new(CipherRepo(state.clone())));
    registry.register_client_managed(Arc::new(FolderRepo(state.clone())));
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persisted_snapshot_rejects_a_different_local_account_even_with_identical_server_id() {
        let server = UserId::new_v4();
        let first = UserId::new_v4();
        let second = UserId::new_v4();
        let response:SyncResponseModel=serde_json::from_value(serde_json::json!({"profile":{"id":server,"organizations":[]},"ciphers":[],"folders":[],"collections":[]})).unwrap();
        let snapshot = Snapshot::prepare(response, server, first, Utc::now()).unwrap();
        let encoded = serde_json::to_vec(&snapshot.persisted).unwrap();
        let decoded: Persisted = serde_json::from_slice(&encoded).unwrap();
        assert!(Snapshot::restore(decoded.clone(), server, second).is_err());
        assert!(Snapshot::restore(decoded, server, first).is_ok());
    }
}
