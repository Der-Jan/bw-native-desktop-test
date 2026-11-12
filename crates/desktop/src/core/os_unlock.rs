use super::*;
use services::sdk::SessionClient;
impl Core {
    pub(crate) fn restore_startup_account(&mut self, uid: UserId) {
        if self.startup_account_restored {
            return;
        }
        self.startup_account_restored = true;
        self.switch_user(uid);
        if self.manager.os_unlock_ref(&uid).is_some() {
            self.unlock_os_store();
        }
    }

    pub(super) fn configure_os_unlock(
        &mut self,
        scope: Scope,
        client: SessionClient,
        enabled: bool,
    ) {
        if self.os_unlock_busy {
            return;
        }
        let Some(store) = self.manager.profiles() else {
            self.notify(
                "OS-store unlock is available for saved server accounts",
                true,
            );
            return;
        };
        self.os_unlock_busy = true;
        let session = scope.session.clone();
        self.events.spawn(
            client.configure_os_unlock(store, enabled),
            move |core, result| {
                if let Ok(reference) = result.as_ref() {
                    core.manager.set_os_unlock_ref(&session, *reference);
                }
                if !scope.current(core) {
                    return;
                }
                core.os_unlock_busy = false;
                match result {
                    Ok(_) => core.notify(
                        if enabled {
                            "OS credential-store unlock enabled for this account"
                        } else {
                            "OS credential-store unlock disabled"
                        },
                        false,
                    ),
                    Err(error) => core.notify(error, true),
                }
            },
        );
    }
    pub fn unlock_os_store(&mut self) {
        let Some(uid) = self.active else {
            return;
        };
        if self.login.busy || self.manager.is_unlocked(&uid) {
            return;
        }
        let (Some(reference), Some(store)) =
            (self.manager.os_unlock_ref(&uid), self.manager.profiles())
        else {
            return;
        };
        let Some(data) = self.manager.unlock_data_for(&uid) else {
            return;
        };
        self.clear_presentation();
        self.login.busy = true;
        self.login.error = None;
        let generation = self.generation;
        self.events.spawn(data.unlock_from_os_store(store,reference),move |core,result| {
            if core.active!=Some(uid) || core.generation!=generation {return;}
            core.login.busy=false;
            match result {
                Ok(())=> {
                    core.screen=Screen::Vault;core.timeout.record_activity(uid);core.refresh_agent();core.load_vault();
                    if core.manager.is_remote(&uid) {core.download();}
                }
                Err(error)=>core.login.error=Some(format!("OS credential-store unlock failed: {error} Use your master password or retry Unlock.")),
            }
            core.update_deadlines();
        });
    }
}
