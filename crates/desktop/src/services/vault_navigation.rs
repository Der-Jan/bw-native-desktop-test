//! Persisted navigation choices contain only account-local IDs and enum values.
use bitwarden_collections::collection::CollectionId;
use bitwarden_core::OrganizationId;
use bitwarden_vault::FolderId;
use serde::{Deserialize, Serialize};

/// Independent facets: ownership, item type and active/favorites/archive/trash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VaultScope {
    #[default]
    All,
    Personal,
    Organization(OrganizationId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VaultSection {
    #[default]
    Active,
    Favorites,
    Duplicates,
    Archive,
    Trash,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FolderFilter {
    #[default]
    All,
    None,
    Folder(FolderId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CollectionFilter {
    #[default]
    All,
    None,
    Collection(CollectionId),
}
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct VaultFilter {
    pub scope: VaultScope,
    pub folder: FolderFilter,
    pub collection: CollectionFilter,
    pub category: Option<bitwarden_vault::CipherType>,
    pub section: VaultSection,
}
impl VaultFilter {
    pub fn with_folder(mut self, folder: FolderFilter) -> Self {
        self.folder = folder;
        self
    }
    pub fn with_collection(mut self, collection: CollectionFilter) -> Self {
        self.collection = collection;
        self
    }

    pub fn with_scope(mut self, scope: VaultScope) -> Self {
        self.scope = scope;
        self
    }
    pub fn with_category(mut self, category: Option<bitwarden_vault::CipherType>) -> Self {
        self.category = category;
        self
    }
    pub fn with_section(mut self, section: VaultSection) -> Self {
        self.section = section;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationGroup {
    Vaults,
    View,
    Types,
    Folders,
    Collections,
    Send,
}
