use super::*;
use bitwarden_collections::collection::CollectionId;
use bitwarden_core::OrganizationId;
use bitwarden_vault::CipherId;
use services::sdk::{SessionClient, placement};
impl Core {
    pub fn open_placement(&mut self) {
        let (Some(uid), Some(detail)) = (self.active, self.detail.as_ref()) else {
            return;
        };
        let Some(cipher) = detail.id else {
            return;
        };
        if !self.remote()
            || !detail.edit
            || !detail.view_password
            || detail.deleted_date.is_some()
            || self.editor.is_some()
            || self.detail_save.is_some()
            || self.attachment_busy.is_some()
            || self.sync.get(&uid).is_some_and(|s| s.busy)
            || self.manager.has_local_edit(&uid, cipher)
            || self.manager.has_placement_intent(&uid)
        {
            return;
        }
        self.placement_editor = Some(model::PlacementEditor {
            id: uuid::Uuid::new_v4(),
            cipher,
            organization: detail.organization_id,
            original_organization: detail.organization_id,
            collections: detail.collection_ids.clone(),
            acknowledged: false,
            busy: false,
            error: None,
        });
    }
    pub fn submit_placement(&mut self) {
        let Some(editor) = self.placement_editor.as_ref().filter(|e| !e.busy) else {
            return;
        };
        let Some(org) = editor.organization else {
            return;
        };
        if editor.collections.is_empty()
            || (editor.original_organization.is_none() && !editor.acknowledged)
        {
            return;
        }
        self.request(Request::Placement {
            cipher: editor.cipher,
            organization: org,
            collections: editor.collections.clone(),
        });
    }
    pub(super) fn execute_placement(
        &mut self,
        scope: Scope,
        client: SessionClient,
        cipher: CipherId,
        organization: OrganizationId,
        collections: Vec<CollectionId>,
    ) {
        let Some(store) = self.manager.profiles() else {
            return;
        };
        let operation = uuid::Uuid::new_v4();
        let uid = scope.uid;
        if let Some(editor) = self.placement_editor.as_mut() {
            editor.busy = true;
            editor.error = None;
        }
        self.sync.insert(
            uid,
            SyncState {
                request: operation,
                error: None,
                busy: true,
            },
        );
        self.events.spawn(placement::change(client,cipher,organization,collections,store.root().to_path_buf(),store),move|core,result|{
            let Some(state)=core.sync.get_mut(&uid).filter(|s|s.request==operation) else{return;};
            state.busy=false;
            match result {
                Ok(outcome) if core.manager.accepts_session(&outcome.report.session)=>{
                    let error=outcome.report.problems.first().cloned();
                    core.sync.get_mut(&uid).unwrap().error=error.clone();
                    core.caches.remove(&uid);
                    if core.active==Some(uid) {
                        core.reload_after_sync();
                        if let Some(error)=error {core.notify(error,true);}
                        else if outcome.applied {core.notify("Vault and collections updated",false);}
                        else {core.notify("The change is awaiting server confirmation. Check the server before retrying.",true);}
                    }
                },
                Ok(_)=>{},
                Err(error)=>{
                    core.sync.get_mut(&uid).unwrap().error=Some(error.clone());
                    if scope.current(core) {
                        if let Some(editor)=core.placement_editor.as_mut(){editor.busy=false;editor.error=Some(error.clone());}
                        core.notify(error,true);
                    }
                },
            }
            core.refresh_agent();
        });
    }
    pub fn dismiss_placement_intent(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        if self.sync.get(&scope.uid).is_some_and(|s| s.busy) {
            return;
        }
        let Some(client) = self.manager.client_for(&scope.uid) else {
            return;
        };
        self.spawn_scoped(scope,client.dismiss_placement_intent(),|core,result|match result {
            Ok(())=>core.notify("Local request record dismissed. This does not undo an organization or collection change.",false),
            Err(e)=>core.notify(e,true),
        });
    }
}
