use super::*;
use crate::services::{
    clipboard::Sensitivity,
    sdk::{Action, Authorization, ClientExt},
    vault_navigation::{CollectionFilter, FolderFilter, VaultFilter, VaultScope, VaultSection},
};
use bitwarden_vault::{CipherListViewType, CipherType, CipherView};
use model::CopyField;
use std::sync::atomic::AtomicBool;

impl Core {
    pub fn set_filter(&mut self, filter: VaultFilter) {
        self.cancel_prompt();
        self.filter = filter;
        if let Some(uid) = self.active {
            self.settings.vault_filters.insert(uid, filter);
            self.settings.save();
        }
        self.close_details();
        self.refilter();
    }
    pub fn set_query(&mut self, query: String) {
        self.cancel_prompt();
        self.query = query;
        self.refilter();
    }
    pub fn refilter(&mut self) {
        let Some(uid) = self.active else {
            return;
        };
        let Some(cache) = self.caches.get_mut(&uid) else {
            return;
        };
        cache.visible = filter(cache, self.filter, &self.query.to_lowercase());
        services::vault_sort::sort_items(
            &mut cache.visible,
            self.settings.vault_sort,
            &cache.sort_keys,
        );
        if self
            .selected
            .is_some_and(|id| !cache.visible.iter().any(|v| v.id == Some(id)))
        {
            self.selected = None;
            self.detail = None;
            self.detail_open = false;
            self.detail_request = None;
        }
        self.scroll_to = Some(0);
    }
    pub fn sort_by(&mut self, column: services::vault_sort::SortColumn) {
        self.cancel_prompt();
        self.settings.vault_sort = self.settings.vault_sort.clicked(column);
        self.settings.save();
        self.refilter();
    }
    pub fn close_details(&mut self) {
        self.detail = None;
        self.detail_open = false;
        self.detail_request = None;
        self.editor = None;
    }
    pub fn navigate(&mut self, down: bool) {
        let Some(cache) = self.current_cache() else {
            return;
        };
        if cache.visible.is_empty() {
            return;
        }
        let previous = self
            .selected
            .and_then(|id| cache.visible.iter().position(|v| v.id == Some(id)));
        let next = match previous {
            None => 0,
            Some(i) if down => (i + 1).min(cache.visible.len() - 1),
            Some(i) => i.saturating_sub(1),
        };
        if previous == Some(next) {
            return;
        }
        if let Some(id) = cache.visible[next].id {
            self.scroll_to = Some(next);
            self.request(Request::Read(id));
        }
    }
    fn authorization(&self, request: &Request) -> Option<(Authorization, bool, bool)> {
        let action = request.action();
        if matches!(request, Request::Export { .. }) {
            return Some((Authorization::export(), true, false));
        }
        let Some(id) = request.id() else {
            return Some((
                Authorization {
                    cipher: None,
                    revision: None,
                    action,
                },
                false,
                false,
            ));
        };
        let item = self
            .current_cache()?
            .all
            .iter()
            .find(|v| v.id == Some(id))?;
        let denied = !item.view_password
            || (matches!(action, Action::Write | Action::Edit | Action::Delete) && !item.edit)
            || (action == Action::Delete && item.permissions.as_ref().is_some_and(|p| !p.delete));
        Some((
            Authorization {
                cipher: Some(id),
                revision: Some(item.revision_date),
                action,
            },
            item.reprompt == bitwarden_vault::CipherRepromptType::Password,
            denied,
        ))
    }
    pub(super) fn request_action(&mut self, request: Request) {
        let Some(scope) = self.scope() else {
            return;
        };
        if self.remote()
            && matches!(
                request.action(),
                Action::Write | Action::Edit | Action::Delete | Action::Export
            )
        {
            self.notify("This synchronized vault is currently read-only.", true);
            return;
        }
        self.cancel_prompt();
        let Some((target, protected, denied)) = self.authorization(&request) else {
            self.notify("This item is no longer available.", true);
            return;
        };
        if denied {
            self.notify("You do not have permission for this action.", true);
            return;
        }
        if protected && !self.manager.permitted(&scope.uid, &target, false) {
            self.prompt = Some(Prompt {
                id: uuid::Uuid::new_v4(),
                target,
                request: Some(request),
                session: scope.session,
                generation: scope.generation,
                busy: false,
                error: false,
                cancelled: Arc::new(AtomicBool::new(false)),
            });
        } else {
            self.execute_request(request);
        }
    }
    pub fn confirm_password(&mut self, password: Zeroizing<String>) {
        let Some(prompt) = self.prompt.as_mut() else {
            return;
        };
        if prompt.busy {
            return;
        }
        let Some((client, envelope)) = self.manager.validation_data_for(&prompt.session.user_id())
        else {
            return;
        };
        let id = prompt.id;
        let target = prompt.target.clone();
        let cancelled = prompt.cancelled.clone();
        let scope = Scope {
            uid: prompt.session.user_id(),
            generation: prompt.generation,
            session: prompt.session.clone(),
        };
        prompt.busy = true;
        prompt.error = false;
        self.events.spawn(
            client.authorize_password_scoped(envelope, password.to_string(), target, cancelled),
            move |core, result| {
                if !scope.current(core) || core.prompt.as_ref().is_none_or(|p| p.id != id) {
                    return;
                }
                if result.is_err() {
                    if let Some(p) = core.prompt.as_mut() {
                        p.busy = false;
                        p.error = true;
                    }
                    return;
                }
                let mut prompt = core.prompt.take().unwrap();
                let Some(request) = prompt.request.take() else {
                    return;
                };
                if !core
                    .authorization(&request)
                    .is_some_and(|(target, _, denied)| target == prompt.target && !denied)
                {
                    core.manager.clear_grants();
                    core.notify("Item changed; select it again.", true);
                    return;
                }
                core.execute_request(request);
            },
        );
    }
    fn execute_request(&mut self, request: Request) {
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(client) = self.manager.client_for(&scope.uid) else {
            return;
        };
        match request {
            Request::Read(id) | Request::Edit(id) => {
                let editing = matches!(request, Request::Edit(_));
                let operation = uuid::Uuid::new_v4();
                self.detail_request = Some(operation);
                self.selected = Some(id);
                self.detail = None;
                self.editor = None;
                self.detail_open = true;
                self.spawn_scoped(
                    scope,
                    client.full_cipher_for(id, if editing { Action::Edit } else { Action::Read }),
                    move |core, result| {
                        if core.detail_request != Some(operation) || core.selected != Some(id) {
                            return;
                        }
                        match result {
                            Ok(view) => {
                                if editing {
                                    core.editor = Some(Editor::from_cipher(&view));
                                }
                                core.detail = Some(view);
                            }
                            Err(error) => {
                                core.detail_open = false;
                                core.notify(error, true);
                            }
                        }
                    },
                );
            }
            Request::CopyText(id, text) => {
                self.spawn_scoped(
                    scope,
                    async move {
                        client.full_cipher_for(id, Action::Copy).await?;
                        Ok::<_, String>(text)
                    },
                    |core, result| match result {
                        Ok(text) => core
                            .clipboard
                            .copy(text.to_string(), Sensitivity::Sensitive),
                        Err(e) => core.notify(e, true),
                    },
                );
            }
            Request::Copy(id, field) => {
                self.spawn_scoped(
                    scope,
                    async move {
                        let view = client.full_cipher_for(id, Action::Copy).await?;
                        copy_field(&view, field)
                    },
                    |core, result| match result {
                        Ok(Some((text, sensitivity))) => {
                            core.clipboard.copy(text, sensitivity);
                            if core.active.is_some_and(|uid| {
                                core.settings.preferences_for(&uid).minimize_on_copy
                            }) {
                                core.hidden = true;
                            }
                            core.notify("Copied", false)
                        }
                        Ok(None) => {}
                        Err(e) => core.notify(e, true),
                    },
                );
            }
            Request::Save(view) => {
                let editor_id = self.editor.as_ref().map(|e| e.id);
                if let Some(editor) = self.editor.as_mut() {
                    editor.busy = true;
                }
                self.spawn_scoped(
                    scope,
                    client.save_cipher(*view),
                    move |core, result| match result {
                        Ok(view) => {
                            if core.editor.as_ref().map(|e| e.id) != editor_id {
                                core.load_vault();
                                return;
                            }
                            core.selected = view.id;
                            core.detail = Some(view);
                            core.detail_open = true;
                            core.editor = None;
                            core.load_vault();
                            core.notify("Item saved", false);
                        }
                        Err(e) => {
                            if let Some(editor) =
                                core.editor.as_mut().filter(|e| Some(e.id) == editor_id)
                            {
                                editor.busy = false;
                            }
                            core.notify(e, true);
                        }
                    },
                );
            }
            Request::Delete(id) => {
                self.spawn_scoped(scope, client.soft_delete_cipher(id), move |core, result| {
                    match result {
                        Ok(()) => {
                            if core.selected == Some(id) {
                                core.close_details();
                                core.selected = None;
                            }
                            core.load_vault();
                            core.notify("Item moved to trash", false);
                        }
                        Err(e) => core.notify(e, true),
                    }
                });
            }
            Request::Export {
                organization,
                format,
                path,
            } => {
                self.spawn_scoped(
                    scope,
                    client.export_to_path(organization, format, path),
                    |core, result| match result {
                        Ok(path) => {
                            core.export_open = false;
                            core.notify(format!("Export saved: {path}"), false)
                        }
                        Err(e) => core.notify(e, true),
                    },
                );
            }
        }
    }
    pub fn new_item(&mut self, kind: CipherType) {
        if !self.unlocked() || self.remote() {
            return;
        }
        self.cancel_prompt();
        self.selected = None;
        self.detail_request = None;
        self.detail = None;
        self.detail_open = true;
        let now = chrono::Utc::now();
        let mut value = serde_json::json!({"type":kind,"name":"","collectionIds":[],"partial":false,"favorite":false,"reprompt":0,"organizationUseTotp":false,"edit":true,"viewPassword":true,"creationDate":now,"revisionDate":now});
        let field = match kind {
            CipherType::Login => "login",
            CipherType::SecureNote => "secureNote",
            CipherType::Card => "card",
            CipherType::Identity => "identity",
            CipherType::SshKey => "sshKey",
            CipherType::BankAccount => "bankAccount",
            CipherType::DriversLicense => "driversLicense",
            CipherType::Passport => "passport",
        };
        value[field] = if kind == CipherType::SecureNote {
            serde_json::json!({"type":0})
        } else {
            serde_json::json!({})
        };
        if kind == CipherType::SshKey {
            value["sshKey"] = serde_json::json!({"privateKey":"","publicKey":"","fingerprint":""});
        }
        match serde_json::from_value::<CipherView>(value) {
            Ok(view) => self.editor = Some(Editor::from_cipher(&view)),
            Err(e) => {
                self.detail_open = false;
                self.notify(format!("Cannot create item: {e}"), true);
            }
        }
    }
    pub fn save_editor(&mut self) {
        let Some(editor) = &self.editor else {
            return;
        };
        if editor.busy {
            return;
        }
        match editor.cipher() {
            Ok(view) if !view.name.trim().is_empty() => self.request(Request::Save(Box::new(view))),
            Ok(_) => self.notify("Enter an item name", true),
            Err(e) => self.notify(e, true),
        }
    }
    pub fn create_folder(&mut self, name: String) {
        let Some(scope) = self.scope() else {
            return;
        };
        if self.remote() || name.trim().is_empty() {
            return;
        }
        let Some(client) = self.manager.client_for(&scope.uid) else {
            return;
        };
        self.spawn_scoped(
            scope,
            client.create_folder(name),
            |core, result| match result {
                Ok(_) => {
                    core.new_folder_open = false;
                    core.load_vault();
                }
                Err(e) => core.notify(e, true),
            },
        );
    }
}
fn filter(
    cache: &VaultCache,
    filter: VaultFilter,
    query: &str,
) -> Vec<Arc<bitwarden_vault::CipherListView>> {
    let folder = match filter.folder {
        FolderFilter::Folder(id) => cache
            .folders
            .iter()
            .find(|f| f.id == id)
            .map(|f| f.name.as_str()),
        _ => None,
    };
    let collection = match filter.collection {
        CollectionFilter::Collection(id) => cache.collections.iter().find(|c| c.id == id),
        _ => None,
    };
    let folders: HashSet<_> = cache
        .folders
        .iter()
        .filter(|f| folder.is_some_and(|p| subtree(&f.name, p)))
        .map(|f| f.id)
        .collect();
    let collections: HashSet<_> = cache
        .collections
        .iter()
        .filter(|c| {
            collection.is_some_and(|p| {
                c.organization_id == p.organization_id && subtree(&c.name, &p.name)
            })
        })
        .map(|c| c.id)
        .collect();
    let terms: Vec<_> = query.split_whitespace().collect();
    cache
        .all
        .iter()
        .filter(|item| {
            (match filter.scope {
                VaultScope::All => true,
                VaultScope::Personal => item.organization_id.is_none(),
                VaultScope::Organization(id) => item.organization_id == Some(id),
            }) && filter.category.is_none_or(|c| kind(&item.r#type) == c)
                && (match filter.section {
                    VaultSection::Active => {
                        item.deleted_date.is_none() && item.archived_date.is_none()
                    }
                    VaultSection::Favorites => {
                        item.favorite && item.deleted_date.is_none() && item.archived_date.is_none()
                    }
                    VaultSection::Archive => {
                        item.archived_date.is_some() && item.deleted_date.is_none()
                    }
                    VaultSection::Trash => item.deleted_date.is_some(),
                })
                && (match filter.folder {
                    FolderFilter::All => true,
                    FolderFilter::None => item.folder_id.is_none(),
                    FolderFilter::Folder(id) => item
                        .folder_id
                        .is_some_and(|v| v == id || folders.contains(&v)),
                })
                && (match filter.collection {
                    CollectionFilter::All => true,
                    CollectionFilter::None => item.collection_ids.is_empty(),
                    CollectionFilter::Collection(id) => item
                        .collection_ids
                        .iter()
                        .any(|c| *c == id || collections.contains(c)),
                })
                && terms.iter().all(|term| {
                    item.id
                        .and_then(|id| cache.search_keys.get(&id))
                        .is_some_and(|text| text.contains(term))
                })
        })
        .cloned()
        .collect()
}
fn subtree(name: &str, parent: &str) -> bool {
    name == parent
        || name
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with('/'))
}
pub fn kind(t: &CipherListViewType) -> CipherType {
    match t {
        CipherListViewType::Login(_) => CipherType::Login,
        CipherListViewType::SecureNote => CipherType::SecureNote,
        CipherListViewType::Card(_) => CipherType::Card,
        CipherListViewType::Identity => CipherType::Identity,
        CipherListViewType::SshKey => CipherType::SshKey,
        CipherListViewType::BankAccount(_) => CipherType::BankAccount,
        CipherListViewType::DriversLicense => CipherType::DriversLicense,
        CipherListViewType::Passport => CipherType::Passport,
    }
}
fn copy_field(
    view: &CipherView,
    field: CopyField,
) -> Result<Option<(String, Sensitivity)>, String> {
    let mut sensitivity = Sensitivity::Sensitive;
    let value = match field {
        CopyField::Json(pointer) => serde_json::to_value(view).ok().and_then(|v| {
            v.pointer(&pointer)
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        }),
        CopyField::Username => {
            sensitivity = Sensitivity::Normal;
            view.login.as_ref().and_then(|l| l.username.clone())
        }
        CopyField::Password => view.login.as_ref().and_then(|l| l.password.clone()),
        CopyField::Url(i) => {
            sensitivity = Sensitivity::Normal;
            view.login
                .as_ref()
                .and_then(|l| l.uris.as_ref())
                .and_then(|u| u.get(i))
                .and_then(|u| u.uri.clone())
        }
        CopyField::Custom(i) => view.fields.as_ref().and_then(|f| f.get(i)).and_then(|f| {
            if f.r#type != bitwarden_vault::FieldType::Hidden {
                sensitivity = Sensitivity::Normal;
            }
            f.value.clone()
        }),
        CopyField::SshPrivate => view.ssh_key.as_ref().map(|k| k.private_key.clone()),
        CopyField::SshPublic => {
            sensitivity = Sensitivity::Normal;
            view.ssh_key.as_ref().map(|k| k.public_key.clone())
        }
        CopyField::SshFingerprint => {
            sensitivity = Sensitivity::Normal;
            view.ssh_key.as_ref().map(|k| k.fingerprint.clone())
        }
        CopyField::Totp => view
            .login
            .as_ref()
            .and_then(|l| l.totp.as_ref())
            .map(|seed| {
                bitwarden_vault::generate_totp(seed.clone(), Some(chrono::Utc::now()))
                    .map(|code| code.code)
                    .map_err(|_| "Invalid TOTP seed".to_string())
            })
            .transpose()?,
    };
    Ok(value.map(|v| (v, sensitivity)))
}
