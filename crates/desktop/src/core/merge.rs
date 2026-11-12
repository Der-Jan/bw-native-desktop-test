use super::*;
use bitwarden_vault::{CipherId, CipherView};
use services::{sdk::ClientExt, vault_navigation::VaultSection};

#[derive(PartialEq, Eq)]
pub enum Phase {
    Loading,
    Review,
    Saving,
    Uploading,
    Trash,
    Trashing,
    Stopped,
}
pub struct Merge {
    pub request: uuid::Uuid,
    pub ids: Vec<CipherId>,
    pub phase: Phase,
    pub error: Option<String>,
    pub loaded: Vec<Editor>,
    pub preview: Option<Editor>,
    pub revisions: Vec<chrono::DateTime<chrono::Utc>>,
    pub next: usize,
    pub primary_revision: Option<chrono::DateTime<chrono::Utc>>,
}
impl Core {
    pub fn begin_merge(&mut self, ids: Vec<CipherId>) {
        if self.filter.section != VaultSection::Duplicates
            || ids.len() < 2
            || self.merge.is_some()
            || self.sync.values().any(|s| s.busy)
            || self.detail_save.is_some()
        {
            return;
        }
        if let Some(uid) = self.active
            && ids.iter().any(|id| self.manager.has_local_edit(&uid, *id))
        {
            self.notify("Synchronize the selected entries before merging", true);
            return;
        }
        if self.editor.is_some() {
            self.notify("Save or cancel the open edit before merging", false);
            return;
        }
        let Some(cache) = self.current_cache() else {
            return;
        };
        let Some(report) = cache.duplicate_report.as_ref() else {
            return;
        };
        if ids.iter().any(|id| !report.ids.contains(id)) {
            return;
        }
        self.close_details();
        self.merge = Some(Merge {
            request: uuid::Uuid::new_v4(),
            ids: ids.clone(),
            phase: Phase::Loading,
            error: None,
            loaded: vec![],
            preview: None,
            revisions: vec![],
            next: 0,
            primary_revision: None,
        });
        self.request(Request::MergeRead(ids[0]));
    }
    pub(super) fn merge_read(
        &mut self,
        scope: Scope,
        client: services::sdk::SessionClient,
        id: CipherId,
    ) {
        let Some(merge) = self
            .merge
            .as_ref()
            .filter(|m| m.phase == Phase::Loading && m.ids.get(m.loaded.len()) == Some(&id))
        else {
            return;
        };
        let request = merge.request;
        self.spawn_scoped(scope, client.full_cipher(id), move |core, result| {
            if core.merge.as_ref().is_none_or(|m| m.request != request) {
                return;
            }
            match result {
                Err(error) => core.stop_merge(error),
                Ok(view) => {
                    let m = core.merge.as_mut().unwrap();
                    m.revisions.push(view.revision_date);
                    m.loaded.push(Editor::from_cipher(&view));
                    if let Some(next) = m.ids.get(m.loaded.len()).copied() {
                        core.request(Request::MergeRead(next));
                    } else {
                        let views: Result<Vec<_>, _> =
                            m.loaded.iter().map(Editor::cipher).collect();
                        let result =
                            views.and_then(|v| services::duplicate_merge::plan(&v[0], &v[1..]));
                        match result {
                            Ok(view) => {
                                let m = core.merge.as_mut().unwrap();
                                m.preview = Some(Editor::from_cipher(&view));
                                m.phase = Phase::Review;
                            }
                            Err(error) => core.stop_merge(error),
                        }
                    }
                }
            }
        });
    }
    pub fn confirm_merge(&mut self) {
        let Some(m) = self.merge.as_ref().filter(|m| m.phase == Phase::Review) else {
            return;
        };
        match m.preview.as_ref().unwrap().cipher() {
            Ok(view) => self.request(Request::MergeSave(Box::new(view))),
            Err(error) => self.stop_merge(error),
        }
    }
    pub(super) fn merge_save(
        &mut self,
        scope: Scope,
        client: services::sdk::SessionClient,
        view: CipherView,
    ) {
        let Some(m) = self
            .merge
            .as_mut()
            .filter(|m| m.phase == Phase::Review && view.id == m.ids.first().copied())
        else {
            return;
        };
        if m.preview.as_ref().and_then(|v| v.cipher().ok()).as_ref() != Some(&view) {
            self.stop_merge("Merge contents changed; review the selection again".into());
            return;
        }
        let request = m.request;
        m.phase = Phase::Saving;
        self.detail_save = Some(request);
        self.events
            .spawn(client.save_cipher(view), move |core, result| {
                if result.is_ok() && core.manager.is_remote(&scope.uid) {
                    core.saved_uploads.insert(scope.uid);
                }
                if scope.current(core) && core.detail_save == Some(request) {
                    core.detail_save = None;
                }
                if !scope.current(core) || core.merge.as_ref().is_none_or(|m| m.request != request)
                {
                    return;
                }
                core.detail_save = None;
                match result {
                    Err(error) => core.stop_merge(error),
                    Ok(view) => {
                        let m = core.merge.as_mut().unwrap();
                        m.loaded.clear();
                        m.preview = None;
                        m.next = 1;
                        m.primary_revision = if core.manager.is_remote(&scope.uid) {
                            None
                        } else {
                            Some(view.revision_date)
                        };
                        m.phase = if core.manager.is_remote(&scope.uid) {
                            Phase::Uploading
                        } else {
                            Phase::Trash
                        };
                        core.load_vault();
                    }
                }
            });
    }
    pub(super) fn poll_merge(&mut self) {
        let Some(uid) = self.active else {
            return;
        };
        if self.sync.get(&uid).is_some_and(|s| s.busy) || self.saved_uploads.contains(&uid) {
            return;
        }
        let Some(m) = self.merge.as_ref() else {
            return;
        };
        if m.phase == Phase::Uploading {
            if self.manager.has_local_edit(&uid, m.ids[0])
                || self.sync.get(&uid).is_some_and(|s| s.error.is_some())
            {
                self.stop_merge("Primary save is not confirmed. Secondary entries were kept. Resolve synchronization before trying again.".into());
                return;
            }
            self.merge.as_mut().unwrap().phase = Phase::Trash;
        }
        let Some(m) = self.merge.as_ref().filter(|m| m.phase == Phase::Trash) else {
            return;
        };
        if m.next == m.ids.len() {
            self.merge = None;
            self.duplicate_selection.clear();
            self.load_vault();
            self.notify("Merge complete; secondary entries moved to trash", false);
            return;
        }
        if m.primary_revision.is_none() {
            let Some(primary) = self
                .current_cache()
                .and_then(|c| c.all.iter().find(|v| v.id == Some(m.ids[0])))
            else {
                return;
            };
            self.merge.as_mut().unwrap().primary_revision = Some(primary.revision_date);
        }
        let m = self.merge.as_ref().unwrap();
        let id = m.ids[m.next];
        let revision = m.revisions[m.next];
        let Some(item) = self
            .current_cache()
            .and_then(|c| c.all.iter().find(|i| i.id == Some(id)))
        else {
            return;
        };
        if item.revision_date != revision || item.deleted_date.is_some() {
            self.stop_merge(
                "A secondary entry changed. The primary is saved; remaining entries were kept."
                    .into(),
            );
            return;
        }
        self.merge.as_mut().unwrap().phase = Phase::Trashing;
        self.request(Request::MergeTrash(id, revision));
    }
    pub(super) fn merge_trash_done(&mut self, id: CipherId, applied: bool) {
        let Some(m) = self
            .merge
            .as_mut()
            .filter(|m| m.phase == Phase::Trashing && m.ids.get(m.next) == Some(&id))
        else {
            return;
        };
        if applied {
            m.next += 1;
            m.phase = Phase::Trash;
        } else {
            self.stop_merge("The primary is saved, but trashing a secondary failed or is unconfirmed. Remaining entries were kept.".into());
        }
    }
    pub(super) fn stop_merge(&mut self, error: String) {
        if let Some(m) = self.merge.as_mut() {
            m.phase = Phase::Stopped;
            m.error = Some(error);
            m.loaded.clear();
            m.preview = None;
        }
    }
}
