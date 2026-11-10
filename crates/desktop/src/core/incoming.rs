use super::*;
use services::sdk::incoming;
use services::sdk::notifications;
const RETRY: Duration = Duration::from_secs(30);
#[derive(Default)]
pub(super) struct PollState {
    pub next: Option<Instant>,
    pub busy: bool,
    pub failures: u8,
    checked: bool,
    push: Option<notifications::Subscription>,
    dirty: bool,
    push_after: Option<Instant>,
}
impl Core {
    /// The dispatcher also wakes the tray-only loop; no render loop is required.
    pub(super) fn start_incoming_timer(&self) {
        let events = self.events.clone();
        self.events.runtime.spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(1));
            timer.tick().await;
            loop {
                timer.tick().await;
                (events.wake)();
                match events
                    .sender
                    .try_send(Box::new(|core| core.poll_incoming()))
                {
                    Ok(()) => {}
                    Err(mpsc::TrySendError::Full(_)) => {}
                    Err(mpsc::TrySendError::Disconnected(_)) => break,
                }
                (events.wake)();
            }
        });
    }
    pub fn check_incoming_now(&mut self) {
        if !self.incoming_enabled {
            return;
        }
        for state in self.incoming.values_mut() {
            state.next = None;
            state.checked = false;
        }
        self.poll_incoming();
    }
    #[cfg(test)]
    pub(crate) fn incoming_for_test(&mut self) {
        self.incoming_enabled = true;
        self.check_incoming_now();
    }
    #[cfg(test)]
    pub(crate) fn notifications_for_test(&mut self) {
        self.notifications_enabled = true;
        self.incoming_for_test();
    }
    #[cfg(test)]
    pub(crate) fn expire_incoming_deadlines_for_test(&mut self) {
        for state in self.incoming.values_mut() {
            state.next = None;
        }
    }
    #[cfg(test)]
    pub(crate) fn incoming_idle_for_test(&self) -> bool {
        self.incoming.values().all(|s| !s.busy) && self.sync.values().all(|s| !s.busy)
    }
    pub(super) fn poll_incoming(&mut self) {
        if !self.incoming_enabled {
            return;
        }
        let now = Instant::now();
        self.incoming
            .retain(|uid, _| self.manager.is_remote(uid) && self.manager.is_unlocked(uid));
        for uid in self.manager.user_ids() {
            if !self.manager.is_remote(&uid) || !self.manager.is_unlocked(&uid) {
                self.incoming.remove(&uid);
                continue;
            }
            let state = self.incoming.entry(uid).or_default();
            if self.notifications_enabled && state.push.as_ref().is_none_or(|s| !s.is_active()) {
                state.push = None;
                if let (Some(client), Some(store)) =
                    (self.manager.client_for(&uid), self.manager.profiles())
                {
                    let events = self.events.clone();
                    let wake = Arc::new(move || {
                        (events.wake)();
                        let _ = events
                            .sender
                            .try_send(Box::new(|core| core.poll_incoming()));
                        (events.wake)();
                    });
                    state.push = notifications::subscribe(
                        client,
                        store.root().to_path_buf(),
                        store,
                        &self.events.runtime,
                        wake,
                    )
                    .ok();
                }
            }
            if let Some(push) = &state.push {
                if push.take_revoked() {
                    self.lock_user(uid);
                    continue;
                }
                state.dirty |= push.take_changed();
                if push.take_reconnected() {
                    state.next = None;
                    state.checked = false;
                }
            }
            if self.sync.get(&uid).is_some_and(|s| s.busy) {
                continue;
            }
            if self.active == Some(uid)
                && (self.editor.is_some()
                    || self.attachment_busy.is_some()
                    || self.folder_editor.is_some()
                    || self.prompt.is_some()
                    || self.settings_open
                    || self.profile_busy
                    || self.export_request.is_some())
            {
                continue;
            }
            let state = self.incoming.entry(uid).or_default();
            if state.dirty && !state.busy && state.push_after.is_none_or(|at| at <= now) {
                state.dirty = false;
                state.push_after = Some(now + Duration::from_secs(2));
                self.sync_account_background(uid);
                continue;
            }
            if state.busy || state.checked || state.next.is_some_and(|at| at > now) {
                continue;
            }
            let (Some(client), Some(store)) =
                (self.manager.client_for(&uid), self.manager.profiles())
            else {
                continue;
            };
            let token = client.session();
            let root = store.root().to_path_buf();
            state.busy = true;
            state.checked = true;
            state.next = None;
            self.events.spawn(
                incoming::changed(client, root, store),
                move |core, result| {
                    let Some(state) = core.incoming.get_mut(&uid) else {
                        return;
                    };
                    state.busy = false;
                    if !core.manager.accepts_session(&token) || !core.manager.is_unlocked(&uid) {
                        return;
                    }
                    match result {
                        Ok(changed) => {
                            state.failures = 0;
                            if changed {
                                core.sync_account_background(uid);
                            }
                        }
                        Err(error) => {
                            state.failures = state.failures.saturating_add(1).min(3);
                            state.checked = false;
                            state.next =
                                Some(Instant::now() + RETRY * (1u32 << (state.failures - 1)));
                            if let Some(sync) = core.sync.get_mut(&uid) {
                                sync.error = Some(error);
                            }
                        }
                    }
                },
            );
        }
    }
}
