use super::*;
use bitwarden_vault::CipherId;
use services::sdk::{SessionClient, archive};
impl Core {
    pub(super) fn execute_archive(
        &mut self,
        scope: Scope,
        client: SessionClient,
        cipher: CipherId,
        archived: bool,
    ) {
        self.execute_item_change(scope, client, cipher, archive::Change::Archive(archived));
    }
    pub(super) fn execute_trash(
        &mut self,
        scope: Scope,
        client: SessionClient,
        cipher: CipherId,
        trashed: bool,
    ) {
        self.execute_item_change(scope, client, cipher, archive::Change::Trash(trashed));
    }
    fn execute_item_change(
        &mut self,
        scope: Scope,
        client: SessionClient,
        cipher: CipherId,
        change: archive::Change,
    ) {
        let Some(store) = self.manager.profiles() else {
            self.notify("This action requires a server account", true);
            return;
        };
        let uid = scope.uid;
        let id = uuid::Uuid::new_v4();
        let root = store.root().to_path_buf();
        self.sync.insert(
            uid,
            SyncState {
                request: id,
                error: None,
                busy: true,
            },
        );
        self.events.spawn(archive::change_state(client,cipher,change,root,store),move |core,result| {
            let Some(state)=core.sync.get_mut(&uid) else {return;};
            if state.request!=id {return;}
            state.busy=false;
            match result {
                Ok(outcome) if core.manager.accepts_session(&outcome.report.session)=> {
                    core.caches.remove(&uid);
                    let error=outcome.report.problems.first().cloned();
                    core.sync.get_mut(&uid).unwrap().error=error.clone();
                    if core.active==Some(uid) {
                        core.reload_after_sync();
                        if let Some(error)=error {core.notify(error,true);}
                        else if outcome.applied {core.notify(change.success(),false);}
                        else {core.notify("Item change is awaiting confirmation. Check the server before retrying.",true);}
                    }
                }
                Ok(_)=>{},
                Err(error)=> {
                    core.sync.get_mut(&uid).unwrap().error=Some(error.clone());
                    if core.active==Some(uid) {core.notify(error,true);}
                }
            }
            core.refresh_agent();
        });
    }
    pub fn dismiss_archive_intent(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        if self.sync.get(&scope.uid).is_some_and(|s| s.busy) {
            return;
        }
        let Some(client) = self.manager.client_for(&scope.uid) else {
            return;
        };
        self.spawn_scoped(
            scope,
            client.dismiss_archive_intent(),
            |core, result| match result {
                Ok(()) => core.notify(
                    "Local request record dismissed. Check the server before another change.",
                    false,
                ),
                Err(error) => core.notify(error, true),
            },
        );
    }
}
