use crate::services::sdk::{Authorization, Collection, SessionToken};
use bitwarden_vault::{CipherId, CipherListView, CipherView, FolderId};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use zeroize::{Zeroize, Zeroizing};

pub struct Folder {
    pub id: FolderId,
    pub name: String,
    pub revision: chrono::DateTime<chrono::Utc>,
}
pub struct FolderEditor {
    pub id: uuid::Uuid,
    pub target: Option<(FolderId, chrono::DateTime<chrono::Utc>)>,
    pub name: String,
    pub deleting: bool,
    pub busy: bool,
    pub error: Option<String>,
}
impl Drop for FolderEditor {
    fn drop(&mut self) {
        self.name.zeroize();
    }
}
#[derive(Default)]
pub struct VaultCache {
    pub all: Vec<Arc<CipherListView>>,
    pub visible: Vec<Arc<CipherListView>>,
    pub folders: Vec<Folder>,
    pub collections: Vec<Collection>,
    pub duplicate_filter: Option<crate::services::vault_navigation::VaultFilter>,
    pub duplicate_report: Option<crate::services::uri_matching::Report>,
    pub duplicate_error: Option<String>,
    pub sort_keys: HashMap<CipherId, crate::services::vault_sort::SortKey>,
    pub search_keys: HashMap<CipherId, String>,
}
#[derive(Clone)]
pub enum CopyField {
    Json(String),
    Username,
    Password,
    Totp,
    SshPrivate,
    SshPublic,
    SshFingerprint,
    Url(usize),
    Custom(usize),
}
pub enum Request {
    Read(CipherId),
    CopyText(CipherId, Zeroizing<String>),
    Copy(CipherId, CopyField),
    Edit(CipherId),
    ReviewLocal(CipherId),
    Save(Box<CipherView>),
    Delete(CipherId),
    Archive {
        cipher: CipherId,
        archived: bool,
    },
    DiscardLocal(CipherId),
    Attachment {
        cipher: CipherId,
        operation: crate::services::sdk::attachments::Operation,
    },
    Export {
        organization: Option<bitwarden_core::OrganizationId>,
        format: bitwarden_exporters::ExportFormat,
        path: PathBuf,
    },
}
impl Request {
    pub fn id(&self) -> Option<CipherId> {
        match self {
            Self::Read(id)
            | Self::CopyText(id, _)
            | Self::Copy(id, _)
            | Self::Edit(id)
            | Self::ReviewLocal(id)
            | Self::DiscardLocal(id)
            | Self::Delete(id) => Some(*id),
            Self::Save(v) => v.id,
            Self::Attachment { cipher, .. } | Self::Archive { cipher, .. } => Some(*cipher),
            Self::Export { .. } => None,
        }
    }
    pub fn action(&self) -> crate::services::sdk::Action {
        use crate::services::sdk::Action;
        match self {
            Self::Read(_) => Action::Read,
            Self::Copy(..) | Self::CopyText(..) => Action::Copy,
            Self::Edit(_) | Self::ReviewLocal(_) => Action::Edit,
            Self::Save(_) | Self::DiscardLocal(_) => Action::Write,
            Self::Delete(_) => Action::Delete,
            Self::Archive { .. } => Action::Archive,
            Self::Export { .. } => Action::Export,
            Self::Attachment {
                operation: crate::services::sdk::attachments::Operation::Download { .. },
                ..
            } => Action::Copy,
            Self::Attachment { .. } => Action::Write,
        }
    }
}
pub struct Prompt {
    pub id: uuid::Uuid,
    pub target: Authorization,
    pub request: Option<Request>,
    pub session: SessionToken,
    pub generation: u64,
    pub busy: bool,
    pub error: bool,
    pub cancelled: Arc<AtomicBool>,
}
impl Drop for Prompt {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}
#[derive(Default)]
pub struct Generator {
    pub output: Zeroizing<String>,
    pub busy: bool,
    pub sequence: u64,
}
impl Generator {
    pub fn clear(&mut self) {
        self.output.zeroize();
        self.busy = false;
        self.sequence = self.sequence.wrapping_add(1);
    }
}
/// Editable SDK view as a field schema, never persisted in widget state.
pub struct Editor {
    pub id: uuid::Uuid,
    pub data: serde_json::Value,
    pub source: Option<CipherId>,
    pub busy: bool,
}
impl Editor {
    pub fn from_cipher(view: &CipherView) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            data: serde_json::to_value(view).expect("cipher schema"),
            source: view.id,
            busy: false,
        }
    }
    pub fn cipher(&self) -> Result<CipherView, String> {
        serde_json::from_value(self.data.clone()).map_err(|_| "Invalid item fields".into())
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        zero_value(&mut self.data);
    }
}
pub(crate) fn zero_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => s.zeroize(),
        serde_json::Value::Array(a) => a.iter_mut().for_each(zero_value),
        serde_json::Value::Object(o) => o.values_mut().for_each(zero_value),
        _ => {}
    }
}
