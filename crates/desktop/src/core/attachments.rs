use super::*;
use bitwarden_vault::CipherId;
use services::sdk::{
    SessionClient,
    attachments::{self, Operation},
};
impl Core {
    pub fn pick_attachment(&mut self, cipher: CipherId, download: Option<(String, String)>) {
        let Some(scope) = self.scope() else {
            return;
        };
        if !self.remote() {
            self.notify("Attachments require a server account.", false);
            return;
        }
        if self.attachment_busy.is_some() || self.sync.get(&scope.uid).is_some_and(|s| s.busy) {
            return;
        }
        let id = uuid::Uuid::new_v4();
        self.attachment_busy = Some(id);
        self.spawn_scoped(
            scope,
            async move {
                match download {
                    None => rfd::AsyncFileDialog::new()
                        .set_title("Attach file")
                        .pick_file()
                        .await
                        .map(|file| Operation::Upload(file.path().into())),
                    Some((id, name)) => {
                        let safe: String = name
                            .chars()
                            .map(|c| {
                                if c.is_control()
                                    || matches!(
                                        c,
                                        '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*'
                                    )
                                {
                                    '_'
                                } else {
                                    c
                                }
                            })
                            .collect();
                        rfd::AsyncFileDialog::new()
                            .set_title("Save attachment")
                            .set_file_name(if safe.trim_matches('.').is_empty() {
                                "attachment"
                            } else {
                                &safe
                            })
                            .save_file()
                            .await
                            .map(|file| Operation::Download {
                                id,
                                path: file.path().into(),
                            })
                    }
                }
            },
            move |core, operation| {
                if core.attachment_busy != Some(id) {
                    return;
                }
                core.attachment_busy = None;
                if let Some(operation) = operation {
                    core.request(Request::Attachment { cipher, operation });
                }
            },
        );
    }
    pub(super) fn execute_attachment(
        &mut self,
        scope: Scope,
        client: SessionClient,
        cipher: CipherId,
        operation: Operation,
    ) {
        let Some(store) = self.manager.profiles() else {
            self.notify("Attachments require a server account", true);
            return;
        };
        let label = match &operation {
            Operation::Upload(_) => "Attachment uploaded",
            Operation::Download { .. } => "Attachment downloaded",
            Operation::Delete(_) => "Attachment deleted",
        };
        let root = store.root().to_path_buf();
        let uid = scope.uid;
        let id = uuid::Uuid::new_v4();
        self.attachment_busy = Some(id);
        self.sync.insert(
            uid,
            SyncState {
                request: id,
                error: None,
                busy: true,
            },
        );
        self.events.spawn(
            attachments::perform(client, cipher, operation, root, store),
            move |core, result| {
                if core.sync.get(&uid).is_none_or(|s| s.request != id) {
                    return;
                }
                core.sync.get_mut(&uid).unwrap().busy = false;
                if core.attachment_busy == Some(id) {
                    core.attachment_busy = None;
                }
                match result {
                    Ok(outcome) if core.manager.accepts_session(&outcome.report.session) => {
                        core.caches.remove(&uid);
                        let error = outcome.report.problems.first().cloned();
                        core.sync.get_mut(&uid).unwrap().error = error.clone();
                        if core.active == Some(uid) {
                            core.reload_after_sync();
                            if let Some(error) = error {
                                core.notify(
                                    if outcome.applied {
                                        format!("{label}. {error}")
                                    } else {
                                        error
                                    },
                                    true,
                                );
                            } else {
                                core.notify(label, false);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        core.sync.get_mut(&uid).unwrap().error = Some(error.clone());
                        if core.active == Some(uid) {
                            core.notify(error, true);
                        }
                    }
                }
                core.refresh_agent();
            },
        );
    }
    pub fn dismiss_attachment_intent(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        if self.sync.get(&scope.uid).is_some_and(|s| s.busy) {
            return;
        }
        let Some(client) = self.manager.client_for(&scope.uid) else {
            return;
        };
        self.spawn_scoped(scope,client.dismiss_attachment_intent(),|core,result|match result {Ok(())=>core.notify("Local attachment request record dismissed; check the server before uploading another copy.",false),Err(error)=>core.notify(error,true)});
    }
}
