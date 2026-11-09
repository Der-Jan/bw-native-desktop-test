use super::*;
use futures::StreamExt;
use services::{
    menu::MenuAction,
    session_timeout::{self, UserSnapshot},
    ssh_agent::{Decision, UiEvent},
    tray::TrayAction,
};
impl Core {
    pub(super) fn install_desktop_events(&mut self) {
        self.listen(
            services::broadcast_stream::from_receiver(
                Some(self.browser.events().subscribe()),
                "browser-integration",
            ),
            |core, event| {
                use services::browser_integration::{Decision, Event};
                match event {
                    Event::Approval(approval) => {
                        if !approval.current()
                            || core.browser_approval.is_some()
                            || core.ssh_approval.is_some()
                            || core.prompt.is_some()
                        {
                            approval.respond(Decision::Deny);
                        } else {
                            core.browser_approval = Some(approval);
                        }
                    }
                    Event::Dismiss(id) => {
                        if core.browser_approval.as_ref().is_some_and(|a| a.id == id) {
                            core.browser_approval = None;
                        }
                    }
                    Event::Changed => {}
                }
            },
        );
        self.listen(services::desktop_appearance::updates(), |core, color| {
            core.appearance = color
        });
        services::menu::install_event_handler();
        services::tray::install_event_handler();
        services::global_hotkey::install_event_handler();
        if self.settings.wants_tray() {
            self.start_tray();
        }
        self.listen(
            services::broadcast_stream::from_receiver(
                Some(self.favicon.event_sender().subscribe()),
                "favicon",
            ),
            |_, _| {},
        );
        self.listen(services::tray::click_stream(), |core, event| match event {
            TrayAction::ToggleShowHide => core.toggle_tray(),
            TrayAction::LockVault => core.lock_all(),
            TrayAction::Exit => core.quit(),
            TrayAction::Unavailable => {
                core.tray = None;
                if core.hidden {
                    core.hidden = false;
                    core.restore_requested = true;
                    core.update_deadlines();
                }
            }
        });
        self.listen(services::menu::muda_event_stream(), |core, event| {
            if let Some(action) = core.native_menu.as_ref().and_then(|m| m.resolve(&event.id)) {
                core.window_action = Some(action);
            }
        });
        self.listen(services::global_hotkey::event_stream(), |core, _| {
            core.magnify_requested = !core.magnify_requested;
        });
        self.listen(services::instance_lock::wake_stream(), |core, _| {
            core.hidden = false;
            core.restore_requested = true;
        });
        self.listen(session_events::event_stream(), |core, event| match event {
            session_events::SessionEvent::Suspended => {
                core.timeout.note_suspend();
                core.system_lock();
            }
            session_events::SessionEvent::Locked => core.system_lock(),
            session_events::SessionEvent::Resumed => {
                core.timeout.note_resume();
                core.check_timeouts();
            }
            session_events::SessionEvent::Unlocked => {}
        });
        self.listen(
            services::broadcast_stream::from_receiver(
                Some(self.timeout.tick_sender().subscribe()),
                "session-timeout",
            ),
            |core, _| core.check_timeouts(),
        );
        self.listen(
            services::broadcast_stream::from_receiver(
                Some(self.ssh_agent.events().subscribe()),
                "ssh-agent",
            ),
            |core, event| match event {
                UiEvent::Approval(approval) => {
                    if !approval.current()
                        || core.ssh_approval.is_some()
                        || core.browser_approval.is_some()
                    {
                        approval.respond(Decision::Deny);
                    } else {
                        core.cancel_prompt();
                        core.ssh_approval = Some(approval);
                    }
                }
                UiEvent::Dismiss(id) => {
                    if core.ssh_approval.as_ref().is_some_and(|a| a.id == id) {
                        core.ssh_approval = None;
                    }
                }
            },
        );
    }
    fn listen<T: Send + 'static>(
        &self,
        stream: impl futures::Stream<Item = T> + Send + 'static,
        apply: fn(&mut Core, T),
    ) {
        let events = self.events.clone();
        self.events.runtime.spawn(async move {
            let mut stream = Box::pin(stream);
            while let Some(event) = stream.next().await {
                events.post(move |core| apply(core, event));
            }
        });
    }
    pub fn refresh_agent(&mut self) {
        if let Some(uid) = self.active {
            self.favicon.set_source(
                uid,
                if self.settings.show_favicons && !self.quit_requested {
                    self.manager.favicon_source_for(&uid)
                } else {
                    None
                },
            );
        }
        self.browser
            .configure(self.settings.browser_integration && !self.quit_requested);
        self.browser.set_session(
            self.active
                .and_then(|uid| self.manager.browser_session_for(&uid)),
        );
        if self.browser_approval.as_ref().is_some_and(|a| !a.current())
            && let Some(a) = self.browser_approval.take()
        {
            a.respond(services::browser_integration::Decision::Deny);
        }
        self.ssh_agent
            .configure(self.settings.ssh_agent && !self.quit_requested);
        let session = self
            .active
            .and_then(|uid| self.manager.agent_session_for(&uid));
        let mode = self
            .active
            .map(|uid| self.settings.preferences_for(&uid).ssh_prompt_behavior)
            .unwrap_or(services::preferences::SshPromptBehavior::Always);
        self.ssh_agent.set_context(session, mode);
        if self.ssh_approval.as_ref().is_some_and(|a| !a.current())
            && let Some(a) = self.ssh_approval.take()
        {
            a.respond(Decision::Deny);
        }
    }
    pub fn decide_browser(
        &mut self,
        id: uuid::Uuid,
        decision: services::browser_integration::Decision,
    ) {
        if self.browser_approval.as_ref().is_some_and(|a| a.id == id)
            && let Some(approval) = self.browser_approval.take()
        {
            approval.respond(decision);
        }
    }
    pub fn register_browser(&mut self, browser: services::browser_integration::Browser) {
        if self.browser_registration_busy || !self.settings.browser_integration {
            return;
        }
        self.browser_registration_busy = true;
        let path = self.browser.path().to_path_buf();
        self.events.spawn(
            tokio::task::spawn_blocking(move || {
                services::browser_integration::register(browser, &path)
            }),
            |core, result| {
                core.browser_registration_busy = false;
                match result {
                    Ok(Ok(message)) => core.notify(message, false),
                    Ok(Err(error)) => core.notify(error, true),
                    Err(_) => core.notify("Browser registration failed", true),
                }
            },
        );
    }
    pub fn restore_browser_registration(&mut self) {
        if self.browser_registration_busy {
            return;
        }
        self.settings.browser_integration = false;
        self.save_settings();
        self.browser_registration_busy = true;
        self.events.spawn(
            tokio::task::spawn_blocking(services::browser_integration::restore_registration),
            |core, result| {
                core.browser_registration_busy = false;
                match result {
                    Ok(Ok(message)) => core.notify(message, false),
                    Ok(Err(error)) => core.notify(error, true),
                    Err(_) => core.notify("Browser registration restore failed", true),
                }
            },
        );
    }
    pub fn revoke_browser_approvals(&mut self) {
        if self.browser_registration_busy {
            return;
        }
        self.browser_registration_busy = true;
        self.events
            .spawn(self.browser.revoke_permanent(), |core, result| {
                core.browser_registration_busy = false;
                match result {
                    Ok(()) => core.notify("Saved browser approvals removed", false),
                    Err(error) => core.notify(error, true),
                }
                core.refresh_agent();
            });
    }
    pub fn decide_ssh(&mut self, id: uuid::Uuid, decision: Decision) {
        if self.ssh_approval.as_ref().is_some_and(|a| a.id == id)
            && let Some(a) = self.ssh_approval.take()
        {
            a.respond(if decision == Decision::UntilLock && !a.can_remember {
                Decision::Deny
            } else {
                decision
            });
        }
    }
    pub fn record_activity(&mut self) {
        if let Some(uid) = self.active
            && self.timeout.record_activity(uid)
        {
            self.update_deadlines();
        }
    }
    fn snapshots(&self) -> Vec<UserSnapshot> {
        self.manager
            .user_ids()
            .into_iter()
            .map(|uid| UserSnapshot {
                uid,
                prefs: self.settings.preferences_for(&uid),
                is_unlocked: self.manager.is_unlocked(&uid),
            })
            .collect()
    }
    pub fn update_deadlines(&self) {
        self.timeout.recompute_and_push_deadline(
            &self.snapshots(),
            self.active.as_ref(),
            self.focused && !self.hidden,
        );
    }
    fn check_timeouts(&mut self) {
        for snapshot in self.snapshots() {
            match self.timeout.expired(
                &snapshot,
                self.active.as_ref(),
                self.focused && !self.hidden,
            ) {
                Some(session_timeout::Action::Lock) => self.lock_user(snapshot.uid),
                Some(session_timeout::Action::Logout) => self.logout(snapshot.uid),
                None => {}
            }
        }
        self.update_deadlines();
    }
    fn system_lock(&mut self) {
        for uid in self.manager.user_ids() {
            if self.settings.preferences_for(&uid).lock_on_system_lock {
                self.lock_user(uid);
            }
        }
    }
    pub fn save_settings(&mut self) {
        self.settings.save();
        if self.settings.wants_tray() && self.tray.is_none() {
            self.start_tray();
        }
        if !self.settings.wants_tray() && !self.hidden {
            self.tray = None;
        }
        if let Some(uid) = self.active {
            self.clipboard.set_timeout(
                self.settings
                    .preferences_for(&uid)
                    .clear_clipboard
                    .as_duration(),
            );
        }
        self.refresh_agent();
        self.update_deadlines();
    }
    fn start_tray(&mut self) {
        if self.tray.is_some() || self.tray_starting {
            return;
        }
        #[cfg(target_os = "linux")]
        {
            self.tray_starting = true;
            self.events.spawn(
                tokio::task::spawn_blocking(services::tray::build),
                |core, result| {
                    core.tray_starting = false;
                    if !core.quit_requested && core.settings.wants_tray() {
                        core.tray = result.ok().flatten();
                    }
                },
            );
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.tray = services::tray::build();
        }
    }
    pub fn toggle_tray(&mut self) {
        if self.tray_starting {
            self.notify("Desktop tray is still initializing", false);
            return;
        }
        if self.tray.is_none() {
            self.tray = services::tray::build();
        }
        if self.tray.is_none() {
            self.notify("Desktop tray is unavailable", true);
            return;
        }
        self.hidden = !self.hidden;
        if !self.hidden {
            self.restore_requested = true;
        }
        self.update_deadlines();
    }
    pub fn quit(&mut self) {
        self.quit_requested = true;
        self.login.cancel();
        self.lock_all();
        self.refresh_agent();
    }
    pub fn ready_to_quit(&self) -> bool {
        self.quit_requested && self.locking.is_empty() && self.removing.is_empty()
    }
    pub fn menu_state(&self) -> services::menu::MenuState {
        services::menu::MenuState {
            is_locked: !self.unlocked(),
            has_accounts: self.manager.has_users(),
            has_lockable_accounts: self.manager.has_unlocked_users(),
        }
    }
    pub fn menu_action(&mut self, action: MenuAction) {
        use MenuAction::*;
        match action {
            Quit => self.quit(),
            LockAllVaults => self.lock_all(),
            LockAccount(uid) => self.lock_user(uid),
            LogOutAccount(uid) => self.logout(uid),
            SyncNow => self.synchronize(),
            SearchVault => {
                self.screen = Screen::Vault;
                self.set_query(String::new());
            }
            HideToTray => self.toggle_tray(),
            Settings => {
                self.cancel_prompt();
                self.settings_open = true;
            }
            Generator | GeneratorHistory => {
                self.cancel_prompt();
                self.generator_open = true;
            }
            About => self.about_open = true,
            FingerprintPhrase => self.fingerprint_open = true,
            Import => {}
            Export => {
                if !self.remote() {
                    self.export_open = true;
                }
            }
            NewFolder => self.open_folder(None, false),
            NewItem(kind) => self.new_item(kind),
            CopyUsername => {
                if let Some(id) = self.selected {
                    self.request(Request::Copy(id, model::CopyField::Username));
                }
            }
            CopyPassword => {
                if let Some(id) = self.selected {
                    self.request(Request::Copy(id, model::CopyField::Password));
                }
            }
            CopyTotp => {
                if let Some(id) = self.selected {
                    self.request(Request::Copy(id, model::CopyField::Totp));
                }
            }
            ZoomIn => {
                self.settings.zoom_factor.step_in();
                self.save_settings();
            }
            ZoomOut => {
                self.settings.zoom_factor.step_out();
                self.save_settings();
            }
            ZoomReset => {
                self.settings.zoom_factor.reset();
                self.save_settings();
            }
            OpenStaticUrl(url) => {
                let _ = open::that_detached(url);
            }
            OpenWebVault(path) => {
                if let Some(account) = self
                    .manager
                    .accounts()
                    .into_iter()
                    .find(|a| Some(a.user_id) == self.active)
                {
                    let url = format!(
                        "{}{}",
                        account.server_url.trim_end_matches('/'),
                        path.unwrap_or("")
                    );
                    let _ = open::that_detached(url);
                }
            }
            Minimize | Close | ToggleAlwaysOnTop => {}
        }
    }
}
