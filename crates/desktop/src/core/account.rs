use super::*;
use services::{
    accounts::{
        login::{PendingLogin, Step},
        transport::Transport,
    },
    sdk::{production, vault_sync},
};
impl Core {
    pub fn switch_user(&mut self, uid: UserId) {
        if !self.manager.user_ids().contains(&uid) {
            return;
        }
        self.login.cancel();
        self.clear_presentation();
        self.active = Some(uid);
        self.query.clear();
        self.filter = self
            .settings
            .vault_filters
            .get(&uid)
            .copied()
            .unwrap_or_default();
        let prefs = self.settings.preferences_for(&uid);
        self.clipboard
            .set_timeout(prefs.clear_clipboard.as_duration());
        self.timeout.record_activity(uid);
        if self.manager.is_unlocked(&uid) {
            self.screen = Screen::Vault;
            self.load_vault();
        } else {
            self.screen = Screen::Login;
        }
        self.refresh_agent();
        self.update_deadlines();
    }
    pub fn unlock(&mut self, password: Zeroizing<String>) {
        let Some(uid) = self.active else {
            return;
        };
        if self.login.busy || self.manager.is_unlocked(&uid) {
            return;
        }
        let Some(data) = self.manager.unlock_data_for(&uid) else {
            return;
        };
        self.clear_presentation();
        self.login.busy = true;
        self.login.error = None;
        let generation = self.generation;
        self.events
            .spawn(data.unlock(password.to_string()), move |core, result| {
                if core.active != Some(uid) || core.generation != generation {
                    return;
                }
                core.login.busy = false;
                match result {
                    Ok(()) => {
                        core.screen = Screen::Vault;
                        core.timeout.record_activity(uid);
                        core.refresh_agent();
                        core.load_vault();
                        if core.manager.is_remote(&uid) {
                            core.download();
                        }
                    }
                    Err(_) => {
                        core.login.error = Some(
                            "Master password is incorrect or vault could not be unlocked.".into(),
                        )
                    }
                }
                core.update_deadlines();
            });
    }
    pub fn lock_user(&mut self, uid: UserId) {
        let Some(operation) = self.manager.lock(&uid) else {
            return;
        };
        self.caches.remove(&uid);
        self.sync.remove(&uid);
        self.locking.insert(uid);
        if self.active == Some(uid) {
            self.login.cancel();
            self.clear_presentation();
            self.screen = Screen::Login;
        }
        self.refresh_agent();
        self.events.spawn(operation.finish(), move |core, _| {
            core.locking.remove(&uid);
            core.update_deadlines();
        });
    }
    pub fn lock_all(&mut self) {
        for uid in self.manager.user_ids() {
            self.lock_user(uid);
        }
    }
    pub fn logout(&mut self, uid: UserId) {
        let Some(operation) = self.manager.log_out(&uid) else {
            return;
        };
        self.caches.remove(&uid);
        self.sync.remove(&uid);
        self.timeout.unenroll(&uid);
        self.removing.insert(uid);
        if self.active == Some(uid) {
            self.login.cancel();
            self.clear_presentation();
            self.active = None;
            self.screen = Screen::Login;
        }
        self.refresh_agent();
        self.events.spawn(operation.finish(), move |core, result| {
            core.removing.remove(&uid);
            if let Err(error) = result {
                core.notify(format!("Logout cleanup: {error:?}"), true);
            }
            if core.active.is_none()
                && let Some(next) = core.manager.user_ids().first().copied()
            {
                core.switch_user(next);
            }
            core.update_deadlines();
        });
    }
    pub fn add_account(&mut self) {
        self.login.cancel();
        self.clear_presentation();
        self.active = None;
        self.screen = Screen::Login;
        self.login = Login::default();
        if let Some(store) = self.profiles.clone() {
            self.restore_profile(&store);
        }
        self.refresh_agent();
    }
    pub fn cancel_login(&mut self) {
        if let Some(uid) = self.active
            && self.manager.is_unlocking(&uid)
        {
            self.lock_user(uid);
        }
        self.login.cancel();
        self.clear_epoch += 1;
    }
    pub fn login(&mut self, password: Zeroizing<String>) {
        if self.login.busy || self.profile_busy {
            return;
        }
        let (Some(store), Some(profile)) = (self.profiles.clone(), self.profile.clone()) else {
            self.login.error = Some("Choose a server profile first".into());
            return;
        };
        self.login.cancel();
        let attempt = Attempt::new();
        let id = attempt.id;
        self.login.attempt = Some(attempt.clone());
        self.login.busy = true;
        let email = self.login.email.clone();
        let root = crate::paths::data_dir();
        self.events.spawn(
            async move {
                let transport_store = store.clone();
                let device_root = root.clone();
                let (transport, device) = tokio::task::spawn_blocking(move || {
                    Ok::<_, String>((
                        Transport::new(&transport_store, profile)?,
                        services::accounts::device::id(&device_root)?,
                    ))
                })
                .await
                .map_err(|_| "Could not prepare login")??;
                let login =
                    PendingLogin::prepare(attempt.clone(), transport, email, password, device)
                        .await?;
                prepare_step(login.submit(None).await?, root, store, attempt).await
            },
            move |core, result| core.finish_login_step(id, result),
        );
    }
    pub fn submit_factor(&mut self, code: Zeroizing<String>) {
        if self.login.busy {
            return;
        }
        let Some(pending) = self.login.pending.take() else {
            return;
        };
        let Some(store) = self.profiles.clone() else {
            return;
        };
        let attempt = pending.attempt.clone();
        let id = attempt.id;
        let factor = self.login.factor;
        let root = crate::paths::data_dir();
        self.login.busy = true;
        self.login.error = None;
        self.events.spawn(
            async move {
                prepare_step(
                    pending.submit(Some((factor, code))).await?,
                    root,
                    store,
                    attempt,
                )
                .await
            },
            move |core, result| core.finish_login_step(id, result),
        );
    }
    fn finish_login_step(&mut self, id: uuid::Uuid, result: Result<LoginStep, String>) {
        if self
            .login
            .attempt
            .as_ref()
            .is_none_or(|a| a.id != id || a.check().is_err())
        {
            return;
        }
        self.login.busy = false;
        match result {
            Ok(LoginStep::Challenge(pending, factors)) => {
                self.login.factor = *factors.first().unwrap_or(&Factor::Authenticator);
                self.login.factors = factors;
                self.login.pending = Some(pending);
            }
            Ok(LoginStep::Ready(candidate)) => {
                let Some(attempt) = self.login.attempt.take() else {
                    return;
                };
                match self.manager.accept_login(*candidate, &attempt) {
                    Ok(uid) => {
                        self.switch_user(uid);
                        self.download();
                    }
                    Err(e) => self.login.error = Some(e),
                }
            }
            Err(e) => {
                self.login.error = Some(e);
                self.login.pending = None;
            }
        }
    }
    pub fn send_factor_email(&mut self) {
        if self.login.busy {
            return;
        }
        let Some(pending) = self.login.pending.take() else {
            return;
        };
        let id = pending.attempt.id;
        self.login.busy = true;
        self.events.spawn(
            async move {
                let result = pending.send_email().await;
                (pending, result)
            },
            move |core, (pending, result)| {
                if core
                    .login
                    .attempt
                    .as_ref()
                    .is_some_and(|a| a.id == id && a.check().is_ok())
                {
                    core.login.pending = Some(pending);
                    core.login.busy = false;
                    if let Err(e) = result {
                        core.login.error = Some(e);
                    } else {
                        core.notify("Verification email sent", false);
                    }
                }
            },
        );
    }
    pub fn load_vault(&mut self) {
        self.cancel_duplicate_scan();
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(client) = self.manager.client_for(&scope.uid) else {
            return;
        };
        let uid = scope.uid;
        self.spawn_scoped(
            scope,
            client.list_vault(),
            move |core, result| match result {
                Ok((items, folders)) => {
                    let all: Vec<_> = items.into_iter().map(Arc::new).collect();
                    let mut cache = VaultCache {
                        sort_keys: services::vault_sort::prepare_keys(&all),
                        all,
                        folders: folders
                            .into_iter()
                            .filter_map(|f| {
                                Some(model::Folder {
                                    id: f.id?,
                                    name: f.name,
                                    revision: f.revision_date,
                                })
                            })
                            .collect(),
                        collections: core.manager.list_collections(&uid),
                        ..Default::default()
                    };
                    cache.search_keys = cache
                        .all
                        .iter()
                        .filter_map(|item| {
                            item.id.map(|id| {
                                (
                                    id,
                                    format!(
                                        "{} {} {}",
                                        item.name,
                                        item.subtitle,
                                        match &item.r#type {
                                            bitwarden_vault::CipherListViewType::Login(l) => l
                                                .uris
                                                .as_ref()
                                                .map(|u| u
                                                    .iter()
                                                    .filter_map(|u| u.uri.as_deref())
                                                    .collect::<Vec<_>>()
                                                    .join(" "))
                                                .unwrap_or_default(),
                                            _ => String::new(),
                                        }
                                    )
                                    .to_lowercase(),
                                )
                            })
                        })
                        .collect();
                    core.caches.insert(uid, cache);
                    core.refilter();
                    if core.detail_open && core.detail.is_none() && core.detail_request.is_none() {
                        let readable = core.selected.filter(|id| {
                            core.current_cache().is_some_and(|cache| {
                                cache.all.iter().any(|item| {
                                    item.id == Some(*id)
                                        && item.view_password
                                        && item.reprompt
                                            == bitwarden_vault::CipherRepromptType::None
                                })
                            })
                        });
                        if let Some(id) = readable {
                            core.request(Request::Read(id));
                        } else {
                            core.detail_open = false;
                        }
                    }
                }
                Err(e) => core.notify(e, true),
            },
        );
    }
    pub fn synchronize(&mut self) {
        if let Some(uid) = self.active {
            self.sync_account(uid, true);
        }
    }
    pub fn check_folder_outcome(&mut self) {
        self.download();
    }
    fn download(&mut self) {
        if let Some(uid) = self.active {
            self.sync_account(uid, false);
        }
    }
    pub(super) fn flush_saved_uploads(&mut self) {
        for uid in std::mem::take(&mut self.saved_uploads) {
            if !self.manager.is_unlocked(&uid) || !self.manager.is_remote(&uid) {
                continue;
            }
            if self.sync.get(&uid).is_some_and(|s| s.busy)
                || (self.active == Some(uid)
                    && (self.editor.is_some()
                        || self.detail_save.is_some()
                        || self.folder_editor.is_some()))
            {
                self.saved_uploads.insert(uid);
                continue;
            }
            self.sync_account(uid, true);
        }
    }
    pub(super) fn reload_after_sync(&mut self) {
        let query = std::mem::take(&mut self.query);
        let selected = self.selected;
        let details = self.detail_open;
        self.clear_presentation_for_refresh();
        self.query = query;
        self.selected = selected.map(|id| {
            self.active
                .map_or(id, |uid| self.manager.resolve_cipher_id(&uid, id))
        });
        self.detail_open = details;
        self.load_vault();
    }
    fn reload_incoming(&mut self) {
        if self.generator_open {
            let mut generator = std::mem::take(&mut self.generator);
            self.reload_after_sync();
            generator.busy = false;
            generator.sequence = generator.sequence.wrapping_add(1);
            self.generator = generator;
            self.generator_open = true;
        } else {
            self.reload_after_sync();
        }
    }
    pub(super) fn sync_account_background(&mut self, uid: UserId) {
        self.sync_account_inner(uid, false, true);
    }
    fn sync_account(&mut self, uid: UserId, upload: bool) {
        self.sync_account_inner(uid, upload, false);
    }
    fn sync_account_inner(&mut self, uid: UserId, upload: bool, quiet: bool) {
        if self.active == Some(uid) && self.detail_save.is_some() {
            if !quiet {
                self.notify(
                    "Wait for the item save to finish before synchronizing.",
                    false,
                );
            }
            return;
        }
        if quiet
            && self.active == Some(uid)
            && (self.prompt.is_some()
                || self.detail_save.is_some()
                || self.settings_open
                || self.attachment_busy.is_some()
                || self.profile_busy)
        {
            return;
        }

        if self.active == Some(uid) && (self.editor.is_some() || self.folder_editor.is_some()) {
            if !quiet {
                self.notify("Save or cancel the open edit before synchronizing.", false);
            }
            return;
        }
        if !self.manager.is_unlocked(&uid) {
            return;
        }
        if !self.manager.is_remote(&uid) {
            self.load_vault();
            self.notify("Local vault refreshed", false);
            return;
        }
        if self.sync.get(&uid).is_some_and(|s| s.busy) {
            return;
        }
        let (Some(client), Some(store)) = (self.manager.client_for(&uid), self.manager.profiles())
        else {
            return;
        };
        if upload {
            self.saved_uploads.remove(&uid);
        }
        let root = store.root().to_path_buf();
        let id = uuid::Uuid::new_v4();
        self.sync.insert(
            uid,
            SyncState {
                request: id,
                error: None,
                busy: true,
            },
        );
        self.events.spawn(
            async move {
                if upload { vault_sync::synchronize(client,root,store).await }
                else { vault_sync::download(client,root,store).await }
            },
            move |core, result| {
                let Some(state) = core.sync.get_mut(&uid) else {
                    return;
                };
                if state.request != id {
                    return;
                }
                state.busy = false;
                match result {
                    Ok(report)
                        if core.manager.accepts_session(&report.session)
                            && core.manager.is_unlocked(&uid) =>
                    {
                        state.error = report.problems.first().cloned();
                        core.caches.remove(&uid);
                        if core.active == Some(uid) {
                            core.screen = Screen::Vault;
                            if quiet {core.reload_incoming();} else {core.reload_after_sync();}
                            if !quiet {core.notify(format!("{} entries synchronized; {} changes uploaded; {} drafts need review", report.ciphers, report.uploaded, report.conflicts), report.conflicts > 0 || !report.problems.is_empty());}
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        state.error = Some(error.clone());
                        if core.active == Some(uid) {
                            if quiet {core.reload_incoming();} else {core.reload_after_sync();}
                            if !quiet {core.notify(error, true);}
                        }
                    }
                }
                core.refresh_agent();
                core.update_deadlines();
            },
        );
    }
}
enum LoginStep {
    Challenge(Box<PendingLogin>, Vec<Factor>),
    Ready(Box<production::Candidate>),
}
async fn prepare_step(
    step: Step,
    root: std::path::PathBuf,
    store: ProfileStore,
    attempt: Attempt,
) -> Result<LoginStep, String> {
    match step {
        Step::Challenge(p, f) => Ok(LoginStep::Challenge(p, f)),
        Step::Authenticated(success) => production::prepare(root, store, attempt, *success)
            .await
            .map(|candidate| LoginStep::Ready(Box::new(candidate))),
    }
}
impl Core {
    pub fn probe_account(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        if !self.remote() {
            return;
        }
        let (Some(client), Some(store)) =
            (self.manager.client_for(&scope.uid), self.manager.profiles())
        else {
            return;
        };
        self.spawn_scoped(
            scope,
            services::sdk::refresh::probe(client, crate::paths::data_dir(), store),
            |core, result| match result {
                Ok(()) => core.notify("Account connection successful (no sync)", false),
                Err(e) => core.notify(e, true),
            },
        );
    }
}
