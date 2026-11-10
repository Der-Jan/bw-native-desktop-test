use super::*;
use bitwarden_vault::FolderId;
use services::sdk::folder_management::{self, Change};

impl Core {
    pub fn open_folder(&mut self, id: Option<FolderId>, deleting: bool) {
        let Some(uid) = self.active.filter(|uid| self.manager.is_unlocked(uid)) else {
            return;
        };
        if self.editor.is_some()
            || self.detail_save.is_some()
            || self.sync.get(&uid).is_some_and(|s| s.busy)
        {
            self.notify(
                "Finish the open edit or synchronization before managing folders.",
                false,
            );
            return;
        }
        if self.manager.has_folder_intent(&uid) {
            self.notify(
                "A folder request is awaiting reconciliation. Synchronize before another change.",
                true,
            );
            return;
        }
        let folder = id.and_then(|id| self.current_cache()?.folders.iter().find(|f| f.id == id));
        if id.is_some() && folder.is_none() {
            self.notify("Folder no longer exists", true);
            return;
        }
        let target = folder.map(|f| (f.id, f.revision));
        let name = folder.map_or_else(String::new, |f| f.name.clone());
        self.folder_editor = Some(model::FolderEditor {
            id: uuid::Uuid::new_v4(),
            target,
            name,
            deleting,
            busy: false,
            error: None,
        });
    }
    pub fn submit_folder(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(editor) = self.folder_editor.as_mut().filter(|e| !e.busy) else {
            return;
        };
        let change = match (editor.target, editor.deleting) {
            (Some((id, revision)), true) => Change::Delete { id, revision },
            (Some((id, revision)), false) => Change::Rename {
                id,
                revision,
                name: editor.name.clone(),
            },
            (None, false) => Change::Create(editor.name.clone()),
            _ => return,
        };
        if !editor.deleting && editor.name.trim().is_empty() {
            editor.error = Some("Enter a folder name".into());
            return;
        }
        let operation = editor.id;
        let uid = scope.uid;
        editor.busy = true;
        editor.error = None;
        let Some(client) = self.manager.client_for(&uid) else {
            return;
        };
        let deleted = if matches!(&change, Change::Delete { .. }) {
            change.target()
        } else {
            None
        };
        if !self.remote() {
            self.spawn_scoped(
                scope,
                client.change_local_folder(change),
                move |core, result| match result {
                    Ok(()) => {
                        core.folder_editor = None;
                        core.folder_deleted_filter(deleted);
                        core.load_vault();
                        core.notify("Folder saved", false);
                    }
                    Err(error) => core.folder_error(operation, error),
                },
            );
            return;
        }
        let Some(store) = self.manager.profiles() else {
            return;
        };
        let root = store.root().to_path_buf();
        self.sync.insert(
            uid,
            SyncState {
                request: operation,
                error: None,
                busy: true,
            },
        );
        self.events.spawn(
            folder_management::change(client, change, root, store),
            move |core, result| {
                if core.sync.get(&uid).is_none_or(|s| s.request != operation) {
                    return;
                }
                core.sync.get_mut(&uid).unwrap().busy = false;
                match result {
                    Ok(outcome) if core.manager.accepts_session(&outcome.report.session) => {
                        core.caches.remove(&uid);
                        let error = outcome.report.problems.first().cloned();
                        core.sync.get_mut(&uid).unwrap().error = error.clone();
                        if core.active == Some(uid) {
                            let mut editor =
                                core.folder_editor.take().filter(|e| e.id == operation);
                            core.reload_after_sync();
                            if outcome.applied {
                                core.folder_deleted_filter(deleted);
                            }
                            if let Some(error) = error {
                                if !outcome.applied && !core.manager.has_folder_intent(&uid) {
                                    if let Some(editor) = editor.as_mut() {
                                        editor.busy = false;
                                        editor.error = Some(error.clone());
                                    }
                                    core.folder_editor = editor;
                                }
                                core.notify(error, true);
                            } else {
                                core.notify("Folder synchronized", false);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        core.sync.get_mut(&uid).unwrap().error = Some(error.clone());
                        if core.active == Some(uid) {
                            core.folder_error(operation, error);
                        }
                    }
                }
                core.refresh_agent();
            },
        );
    }
    fn folder_error(&mut self, id: uuid::Uuid, error: String) {
        if let Some(editor) = self.folder_editor.as_mut().filter(|e| e.id == id) {
            editor.busy = false;
            editor.error = Some(error.clone());
        }
        self.notify(error, true);
    }
    fn folder_deleted_filter(&mut self, id: Option<FolderId>) {
        if id.is_some_and(|id| {
            self.filter.folder == services::vault_navigation::FolderFilter::Folder(id)
        }) {
            let mut filter = self.filter;
            filter.folder = services::vault_navigation::FolderFilter::None;
            self.set_filter(filter);
        }
    }
    pub fn dismiss_folder_intent(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        if self.sync.get(&scope.uid).is_some_and(|s| s.busy) {
            return;
        }
        let Some(client) = self.manager.client_for(&scope.uid) else {
            return;
        };
        self.spawn_scoped(scope,client.dismiss_folder_intent(),|core,result|match result {
            Ok(())=>core.notify("Unconfirmed folder request dismissed. Check the server before repeating a creation.",false),
            Err(error)=>core.notify(error,true),
        });
    }
}
