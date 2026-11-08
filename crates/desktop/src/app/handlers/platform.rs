//! Platform / OS integration handlers — window chrome, system events, menu
//! dispatch, tray actions. Kept together because menu/tray actions frequently
//! manipulate the window and the window-message handler dispatches menu
//! actions on keyboard shortcuts.

use iced::Task;

use crate::{
    app::{
        App, Message, SystemMessage, WindowMessage,
        window::{WindowInfo, WindowKind},
    },
    domain::Screen,
    views::{settings::SettingsSnapshot, title_bar::WindowAction},
};

impl App {
    // ── Window lifecycle ───────────────────────────────────────────────────

    pub(crate) fn handle_window_action(&mut self, action: WindowAction) -> Task<Message> {
        let id = self.main_window_id();
        match action {
            WindowAction::Minimize => {
                if self.settings.minimize_to_tray && self.ensure_tray() {
                    self.hide_main_window()
                } else {
                    iced::window::minimize(id, true)
                }
            }
            WindowAction::Maximize => {
                if let Some(info) = self.windows.get_mut(&id) {
                    info.maximized = !info.maximized;
                }
                iced::window::toggle_maximize(id)
            }
            WindowAction::Close => {
                if self.settings.close_to_tray && self.ensure_tray() {
                    self.hide_main_window()
                } else {
                    self.request_exit()
                }
            }
            WindowAction::Drag => iced::window::drag(id),
            WindowAction::ResizeEdge(dir) => iced::window::drag_resize(id, dir),
        }
    }

    pub(crate) fn handle_window_message(&mut self, msg: WindowMessage) -> Task<Message> {
        match msg {
            WindowMessage::Opened(id) => {
                if self
                    .ssh_approval_window
                    .is_some_and(|(window, _)| window == id)
                {
                    return self.focus_ssh_approval(id);
                }
                // Cancellation can beat the asynchronous native open. Never
                // leave an empty always-on-top prompt behind in that race.
                if self
                    .windows
                    .get(&id)
                    .is_some_and(|w| w.kind == WindowKind::SshApproval)
                {
                    return iced::window::close(id);
                }
                if id == self.main_window_id() {
                    Task::batch([
                        iced::window::raw_id::<Message>(id)
                            .map(move |raw| Message::Window(WindowMessage::GotRawId(id, raw))),
                        self.views.vault.restore_surface_scroll(),
                        self.views.send.restore_surface_scroll(),
                    ])
                } else {
                    Task::none()
                }
            }
            WindowMessage::GotRawId(id, raw_id) => {
                if id != self.main_window_id() || self.main_window_suspended {
                    return Task::none();
                }
                self.main_window_raw_id = Some(raw_id);
                self.native_menu = crate::services::menu::attach_menu(raw_id, &self.menu);
                // Populate dynamic submenus immediately so File → Lock vault
                // / Log out reflect any accounts already loaded from disk.
                let state = self.menu_state();
                if let Some(ref mut handle) = self.native_menu {
                    handle.sync_dynamic(&self.cache.accounts);
                    handle.sync_enabled(&state);
                }
                Task::none()
            }
            WindowMessage::HideBackendResolved { id, wayland } => {
                // Restore may have won the race with the native-handle query.
                if id != self.main_window_id()
                    || !self.main_window_hidden
                    || self.main_window_suspended
                {
                    return Task::none();
                }
                if wayland {
                    // winit's Wayland set_visible is a no-op. Destroy only the
                    // surface; the daemon, SDK sessions and view models remain.
                    self.main_window_suspended = true;
                    self.native_menu = None;
                    self.main_window_raw_id = None;
                    iced::window::close(id)
                } else {
                    iced::window::set_mode(id, iced::window::Mode::Hidden)
                }
            }
            WindowMessage::CloseRequested(id) => {
                if let Some((window, request)) = self.ssh_approval_window
                    && window == id
                {
                    return self
                        .decide_ssh_approval(request, crate::services::ssh_agent::Decision::Deny);
                }
                // Route OS-native close to the same handler as the custom
                // title-bar X so close-to-tray applies uniformly. Other
                // windows (About) close normally.
                if id == self.main_window_id() {
                    self.handle_window_action(WindowAction::Close)
                } else {
                    iced::window::close(id)
                }
            }
            WindowMessage::Closed(id) => {
                if let Some((window, request)) = self.ssh_approval_window
                    && window == id
                {
                    self.windows.remove(&id);
                    return self
                        .decide_ssh_approval(request, crate::services::ssh_agent::Decision::Deny);
                }
                let was_main = id == self.main_window_id();
                if was_main && self.main_window_suspended {
                    // Retain the main window's geometry for restoration.
                    return Task::none();
                }
                self.windows.remove(&id);
                if was_main {
                    self.request_exit()
                } else {
                    Task::none()
                }
            }
            WindowMessage::Resized(id, size) => {
                if let Some(info) = self.windows.get_mut(&id) {
                    info.size = size;
                }
                Task::none()
            }
            WindowMessage::Focused(id) => {
                if id == self.main_window_id() {
                    self.main_window_focused = true;
                    // The active user just became eligible for the grace
                    // exemption from `lock_after`; recompute drops their
                    // `lock_after` deadline from the watch.
                    self.refresh_session_timeout_deadline();
                }
                Task::none()
            }
            WindowMessage::Unfocused(id) => {
                if id == self.magnify.window {
                    return self
                        .handle_magnify_message(crate::views::magnify::MagnifyMessage::Hide);
                }
                if id == self.main_window_id() {
                    self.main_window_focused = false;
                    self.refresh_session_timeout_deadline();
                }
                Task::none()
            }
            WindowMessage::MouseInput(id) => {
                // Only main-window clicks count toward the active user's
                // session timer; the Magnify launcher dismisses on
                // focus-loss, so its clicks shouldn't extend the timer.
                if id == self.main_window_id()
                    || self
                        .ssh_approval_window
                        .is_some_and(|(window, _)| window == id)
                {
                    self.record_session_activity();
                }
                Task::none()
            }
            WindowMessage::KeyPressed(id, ev, status) => {
                if let Some((window, request)) = self.ssh_approval_window
                    && window == id
                {
                    self.record_session_activity();
                    if matches!(
                        &ev,
                        iced::keyboard::Event::KeyPressed {
                            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                            repeat: false,
                            ..
                        }
                    ) {
                        return self.decide_ssh_approval(
                            request,
                            crate::services::ssh_agent::Decision::Deny,
                        );
                    }
                    // Enter/Space/Tab are handled by the focused approval
                    // button, whose message contains this request's UUID.
                    return Task::none();
                }
                // Magnify gets first crack at keys for its own window.
                if id == self.magnify.window
                    && let Some(task) = self.handle_magnify_key(ev.clone())
                {
                    return task;
                }
                // Shortcuts only fire on the main window — Ctrl+F in the
                // About window must not trigger vault search.
                if id != self.main_window_id() {
                    return Task::none();
                }
                // Recorded before the shortcut dispatch so e.g. Ctrl+L
                // (lock vault) pushes back any logout deadline before the
                // lock takes effect.
                self.record_session_activity();
                let iced::keyboard::Event::KeyPressed { key, modifiers, .. } = ev else {
                    return Task::none();
                };
                if matches!(
                    key,
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
                ) && self.ssh_approval.is_some()
                {
                    return self.cancel_ssh_approval();
                }
                if self.ssh_approval.is_some() {
                    return Task::none();
                }
                // Escape closes the settings modal before any menu shortcut
                // lookup, otherwise it would fall through to widgets behind
                // the modal.
                if matches!(
                    key,
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
                ) && self.views.settings.is_open()
                {
                    self.views.settings.close();
                    return Task::none();
                }
                if status == iced::event::Status::Ignored
                    && modifiers.is_empty()
                    && self.screen == Screen::Vault
                    && self.open_overlay.is_none()
                    && self.views.vault.list_keyboard_enabled()
                    && self.collect_overlays(&self.render_ctx(id)).is_empty()
                {
                    use iced::keyboard::{Key, key::Named};
                    match key {
                        Key::Named(Named::PageUp) => {
                            return self.views.vault.scroll_page_task(false);
                        }
                        Key::Named(Named::PageDown) => {
                            return self.views.vault.scroll_page_task(true);
                        }
                        _ => {}
                    }
                }
                let state = self.menu_state();
                let Some(action) = self.menu.find_shortcut_action(&key, modifiers, &state) else {
                    return Task::none();
                };
                self.open_overlay = None;
                self.handle_menu_action(action)
            }
        }
    }

    // ── System signals ─────────────────────────────────────────────────────

    pub(crate) fn handle_system_message(&mut self, msg: SystemMessage) -> Task<Message> {
        match msg {
            SystemMessage::MudaEvent(event) => {
                // Native menu matches win; tray menu is the fallback (both
                // share one global muda receiver).
                if let Some(handle) = &self.native_menu
                    && let Some(action) = handle.resolve(&event.id)
                {
                    return self.handle_menu_action(action);
                }
                if let Some(handle) = &self.tray
                    && let Some(action) = handle.resolve(&event.id)
                {
                    return self.handle_tray_action(action);
                }
                Task::none()
            }
            SystemMessage::TrayClick(action) => self.handle_tray_action(action),
            SystemMessage::ThemeChanged => {
                self.theme.refresh();
                Task::none()
            }
            SystemMessage::CloseToast(idx) => {
                if idx < self.toasts.len() {
                    self.toasts.remove(idx);
                }
                Task::none()
            }
            SystemMessage::ClientManagerLoaded(slot) => {
                // Recover the inner Option even if the load future panicked
                // before sending — the value (if any) is still valid.
                let Some(mgr) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() else {
                    tracing::error!("ClientManager slot was already drained");
                    return Task::none();
                };
                self.client_manager = mgr;
                self.storage_warning = self.client_manager.take_startup_warning();
                self.active_user = self.client_manager.user_ids().into_iter().next();
                self.restore_active_filter();
                self.show_login_for_active()
            }
            SystemMessage::InstanceWakeRequested => self.show_main_window(),
            SystemMessage::LogoutCompleted { operation, result } => {
                if !self.pending_logouts.remove(&operation) {
                    return Task::none();
                }
                if let Err(error) = result {
                    self.storage_warning = Some(match error {
                        crate::services::sdk::LogoutFailure::NotCommitted => {
                            // Keep the window open so a failed durable logout cannot
                            // silently turn into a completed-looking application exit.
                            self.exit_after_logout = false;
                            crate::services::sdk::StorageWarning::LogoutNotStored
                        }
                        crate::services::sdk::LogoutFailure::CleanupPending => {
                            crate::services::sdk::StorageWarning::RemovalIncomplete
                        }
                    });
                }
                if self.pending_logouts.is_empty() && self.exit_after_logout {
                    return iced::exit();
                }
                Task::none()
            }
            SystemMessage::SessionQuiesced(session) => {
                if self.client_manager.accepts_session(&session) {
                    self.refresh_accounts_cache();
                }
                Task::none()
            }
            SystemMessage::SessionEvent(ev) => {
                use session_events::SessionEvent;
                match ev {
                    SessionEvent::Suspended => {
                        // Snapshot wall + monotonic so we can compute the
                        // suspend gap on resume. Users with the
                        // `lock_on_system_lock` preference are locked
                        // immediately; the rest stay enrolled so their
                        // `last_activity` can be back-dated on `Resumed`.
                        self.session_timeout.note_suspend();
                        self.lock_system_lock_users()
                    }
                    SessionEvent::Locked => self.lock_system_lock_users(),
                    SessionEvent::Resumed => {
                        // Reconcile last_activity for the time CLOCK_MONOTONIC
                        // missed during suspend, then re-check timeouts.
                        self.session_timeout.note_resume();
                        self.run_session_timeout_check()
                    }
                    SessionEvent::Unlocked => Task::none(),
                }
            }
            SystemMessage::SessionTimeoutCheck => self.run_session_timeout_check(),
            SystemMessage::SyncCompleted {
                uid,
                operation,
                result,
            } => self.finish_vault_sync(uid, operation, result),
        }
    }

    // ── Menu + tray dispatch ───────────────────────────────────────────────

    pub(crate) fn handle_menu_action(
        &mut self,
        action: crate::services::menu::MenuAction,
    ) -> Task<Message> {
        use crate::services::menu::MenuAction;
        if self
            .active_user
            .is_some_and(|uid| self.client_manager.is_remote(&uid))
            && matches!(
                action,
                MenuAction::Import
                    | MenuAction::Export
                    | MenuAction::NewFolder
                    | MenuAction::NewItem(_)
            )
        {
            self.push_toast(crate::components::toast::Toast::warning(
                "Dieser synchronisierte Tresor ist derzeit schreibgeschützt; Serveränderungen und Export folgen später.",
                None,
            ));
            return Task::none();
        }
        match action {
            MenuAction::Quit => {
                return self.request_exit();
            }
            MenuAction::LockAllVaults => {
                let uids = self.client_manager.user_ids();
                let tasks: Vec<_> = uids.iter().map(|uid| self.lock_user(uid)).collect();
                return Task::batch(tasks);
            }
            MenuAction::LockAccount(uid) => {
                return self.lock_user(&uid);
            }
            MenuAction::LogOutAccount(uid) => {
                return self.handle_log_out_for(uid);
            }
            MenuAction::Minimize => {
                return iced::window::minimize(self.main_window_id(), true);
            }
            MenuAction::Close => {
                return iced::window::close(self.main_window_id());
            }
            MenuAction::SearchVault => {
                if self.screen == Screen::Vault {
                    return self.views.vault.focus_search_task().map(Into::into);
                }
            }
            MenuAction::SyncNow => return self.start_vault_sync(),
            MenuAction::HideToTray => {
                if self.ensure_tray() {
                    return self.hide_main_window();
                }
            }
            MenuAction::ToggleAlwaysOnTop => {
                let id = self.main_window_id();
                if let Some(info) = self.windows.get_mut(&id) {
                    info.always_on_top = !info.always_on_top;
                    let level = if info.always_on_top {
                        iced::window::Level::AlwaysOnTop
                    } else {
                        iced::window::Level::Normal
                    };
                    return iced::window::set_level(id, level);
                }
            }
            MenuAction::Settings => {
                if let Some(uid) = self.active_user {
                    self.open_overlay = None;

                    let snap = SettingsSnapshot {
                        settings: self.settings.clone(),
                        prefs: self.settings.preferences_for(&uid),
                    };
                    self.views.settings.open_with(snap);
                }
            }
            MenuAction::Generator => {
                return self.open_generator_modal();
            }
            MenuAction::GeneratorHistory => {
                return self.open_generator_history();
            }
            MenuAction::ZoomIn => {
                if self.settings.zoom_factor.step_in() {
                    self.settings.save();
                }
            }
            MenuAction::ZoomOut => {
                if self.settings.zoom_factor.step_out() {
                    self.settings.save();
                }
            }
            MenuAction::ZoomReset => {
                if self.settings.zoom_factor.reset() {
                    self.settings.save();
                }
            }
            MenuAction::Import => {
                return self.open_import_modal();
            }
            MenuAction::Export => {
                return self.open_export_modal();
            }
            MenuAction::NewFolder => {
                return self.open_new_folder_modal();
            }
            MenuAction::NewItem(t) => {
                if self.screen != Screen::Vault {
                    self.set_screen(Screen::Vault);
                }
                self.open_overlay = None;
                return Task::done(crate::views::vault::VaultMessage::NewItem(t).into());
            }
            MenuAction::CopyUsername => {
                return self.copy_from_active_selection(
                    crate::views::vault::widgets::cipher_detail::CipherDetailMessage::CopyUsername,
                );
            }
            MenuAction::CopyPassword => {
                return self.copy_from_active_selection(
                    crate::views::vault::widgets::cipher_detail::CipherDetailMessage::CopyPassword,
                );
            }
            MenuAction::CopyTotp => {
                return self.copy_from_active_selection(
                    crate::views::vault::widgets::cipher_detail::CipherDetailMessage::CopyTotp,
                );
            }
            MenuAction::FingerprintPhrase => {
                let Some(uid) = self.active_user else {
                    return Task::none();
                };
                let Some(phrase) = self.client_manager.user_fingerprint(&uid) else {
                    return Task::none();
                };
                self.open_overlay = None;
                self.views.fingerprint.open_with(phrase);
            }
            MenuAction::OpenStaticUrl(url) => {
                crate::services::clipboard::launch_url(url);
            }
            MenuAction::OpenWebVault(path) => {
                let Some(base) = self
                    .active_account_entry()
                    .map(|a| a.server_url.as_str())
                    .filter(|s| !s.is_empty())
                else {
                    return Task::none();
                };
                let url = match path {
                    Some(p) => format!("{}/{}", base.trim_end_matches('/'), p),
                    None => base.to_string(),
                };
                crate::services::clipboard::launch_url(&url);
            }
            MenuAction::About => {
                if let Some(id) = self.about_window_id() {
                    return iced::window::gain_focus(id);
                }

                let about_size = iced::Size::new(400.0, 280.0);
                let (about_id, open_task) = iced::window::open(iced::window::Settings {
                    size: about_size,
                    min_size: Some(iced::Size::new(360.0, 260.0)),
                    position: iced::window::Position::Centered,
                    resizable: false,
                    decorations: true,
                    icon: crate::assets::window_icon(),
                    ..Default::default()
                });

                self.windows
                    .insert(about_id, WindowInfo::new(WindowKind::About, about_size));

                return open_task.map(|id| Message::Window(WindowMessage::Opened(id)));
            }
        }
        Task::none()
    }

    fn handle_tray_action(&mut self, action: crate::services::tray::TrayAction) -> Task<Message> {
        use crate::services::tray::TrayAction;
        match action {
            TrayAction::ToggleShowHide => self.toggle_main_window_visibility(),
            TrayAction::LockVault => {
                self.handle_menu_action(crate::services::menu::MenuAction::LockAllVaults)
            }
            TrayAction::Exit => self.request_exit(),
            TrayAction::Unavailable => {
                if self.main_window_hidden {
                    return self.show_main_window();
                }
                Task::none()
            }
        }
    }

    // ── Window visibility helpers (tray + IPC wake) ────────────────────────

    pub(crate) fn hide_main_window(&mut self) -> Task<Message> {
        self.main_window_hidden = true;
        self.main_window_focused = false;
        self.refresh_session_timeout_deadline();
        let cancel = self.cancel_ssh_approval();
        if self.main_window_suspended {
            return cancel;
        }
        let id = self.main_window_id();
        Task::batch([
            cancel,
            iced::window::run(id, |window| {
                use iced::window::raw_window_handle::RawWindowHandle;
                window
                    .window_handle()
                    .is_ok_and(|h| matches!(h.as_raw(), RawWindowHandle::Wayland(_)))
            })
            .map(move |wayland| {
                Message::Window(WindowMessage::HideBackendResolved { id, wayland })
            }),
        ])
    }

    pub(crate) fn show_main_window(&mut self) -> Task<Message> {
        self.main_window_hidden = false;
        if self.main_window_suspended {
            self.main_window_suspended = false;
            let old_id = self.main_window_id();
            let info = self
                .windows
                .remove(&old_id)
                .unwrap_or_else(|| WindowInfo::new(WindowKind::Main, crate::app::MAIN_WINDOW_SIZE));
            let (id, open) = iced::window::open(iced::window::Settings {
                size: info.size,
                min_size: Some(iced::Size::new(800.0, 750.0)),
                maximized: info.maximized,
                level: if info.always_on_top {
                    iced::window::Level::AlwaysOnTop
                } else {
                    iced::window::Level::Normal
                },
                decorations: crate::services::menu::should_use_native_title_bar(),
                exit_on_close_request: false,
                platform_specific: crate::app::helpers::main_window_platform_specific(),
                icon: crate::assets::window_icon(),
                ..Default::default()
            });
            self.main_window = id;
            self.windows.insert(id, info);
            return open.map(|id| Message::Window(WindowMessage::Opened(id)));
        }
        let id = self.main_window_id();
        Task::batch([
            iced::window::set_mode(id, iced::window::Mode::Windowed),
            iced::window::minimize(id, false),
            iced::window::gain_focus(id),
        ])
    }

    /// Restore hidden or unfocused windows; hide a focused visible window.
    fn toggle_main_window_visibility(&mut self) -> Task<Message> {
        if self.main_window_hidden || !self.main_window_focused {
            self.show_main_window()
        } else {
            self.hide_main_window()
        }
    }

    /// Lazily build the tray. Callers hiding the window to the tray MUST
    /// check the return: on `false`, fall through to a normal minimize/close,
    /// otherwise the window becomes unrecoverable (hidden, no tray icon).
    fn ensure_tray(&mut self) -> bool {
        if self.tray.is_none() {
            self.tray = crate::services::tray::build();
            if self.tray.is_none() {
                tracing::warn!("tray requested but failed to initialise; continuing without tray");
            }
        }
        let available = self.tray.as_ref().is_some_and(|tray| tray.available());
        if !available {
            self.push_toast(crate::components::toast::Toast::warning(
                "Kein erreichbarer System-Tray. Das Fenster bleibt über die Taskleiste erreichbar.",
                None,
            ));
        }
        available
    }
}

#[cfg(test)]
mod tray_window_tests {
    use super::*;
    #[tokio::test]
    async fn wayland_hide_suspends_only_the_surface_and_restore_replaces_its_id() {
        let mut app = App::test();
        let old = app.main_window_id();
        let uid = crate::domain::UserId::new_v4();
        app.active_user = Some(uid);
        app.windows.get_mut(&old).unwrap().size = iced::Size::new(1130.0, 810.0);
        let _ = app.hide_main_window();
        let _ = app.handle_window_message(WindowMessage::HideBackendResolved {
            id: old,
            wayland: true,
        });
        assert!(app.main_window_hidden && app.main_window_suspended);
        let _ = app.handle_window_message(WindowMessage::Closed(old));
        assert!(!app.shutting_down);
        assert_eq!(app.active_user, Some(uid));
        let _ = app.show_main_window();
        let new = app.main_window_id();
        assert_ne!(new, old);
        assert!(!app.main_window_hidden && !app.main_window_suspended);
        assert_eq!(app.windows[&new].size, iced::Size::new(1130.0, 810.0));
        let _ = app.handle_window_message(WindowMessage::Closed(old));
        assert!(!app.shutting_down);
        assert_eq!(app.active_user, Some(uid));
    }
    #[tokio::test]
    async fn restore_cancels_pending_hide_and_stale_backend_results() {
        let mut app = App::test();
        let id = app.main_window_id();
        let _ = app.hide_main_window();
        let _ = app.show_main_window();
        let _ = app.handle_window_message(WindowMessage::HideBackendResolved { id, wayland: true });
        assert!(!app.main_window_hidden && !app.main_window_suspended);
        let _ = app.hide_main_window();
        let _ = app.handle_window_message(WindowMessage::HideBackendResolved {
            id: iced::window::Id::unique(),
            wayland: true,
        });
        assert!(!app.main_window_suspended);
        let _ =
            app.handle_window_message(WindowMessage::HideBackendResolved { id, wayland: false });
        assert!(app.main_window_hidden && !app.main_window_suspended);
        assert_eq!(app.main_window_id(), id);
    }
}

#[cfg(test)]
mod page_key_tests {
    use super::*;
    use iced::{
        event::Status,
        keyboard::{Modifiers, key::Named},
    };

    fn page(
        id: iced::window::Id,
        key: Named,
        status: Status,
        modifiers: Modifiers,
    ) -> WindowMessage {
        let iced::Event::Keyboard(mut event) = iced_test::simulator::press_key(key, None) else {
            unreachable!()
        };
        if let iced::keyboard::Event::KeyPressed {
            modifiers: value, ..
        } = &mut event
        {
            *value = modifiers;
        }
        WindowMessage::KeyPressed(id, event, status)
    }
    #[tokio::test]
    async fn page_keys_scroll_only_the_unobstructed_main_vault_and_respect_capture() {
        crate::test_support::init();
        let mut app = App::test();
        let id = app.main_window_id();
        app.screen = Screen::Vault;
        let uid = crate::domain::UserId::new_v4();
        app.active_user = Some(uid);
        app.client_manager = crate::services::sdk::test_support::unlocked_manager(uid);
        for key in [Named::PageDown, Named::PageUp] {
            assert!(
                app.handle_window_message(page(id, key, Status::Ignored, Modifiers::empty()))
                    .units()
                    > 0
            );
            assert_eq!(
                app.handle_window_message(page(id, key, Status::Captured, Modifiers::empty()))
                    .units(),
                0
            );
            assert_eq!(
                app.handle_window_message(page(id, key, Status::Ignored, Modifiers::CTRL))
                    .units(),
                0
            );
            assert_eq!(
                app.handle_window_message(page(
                    iced::window::Id::unique(),
                    key,
                    Status::Ignored,
                    Modifiers::empty()
                ))
                .units(),
                0
            );
        }
        let _ = app.handle_menu_action(crate::services::menu::MenuAction::Settings);
        assert_eq!(
            app.handle_window_message(page(
                id,
                Named::PageDown,
                Status::Ignored,
                Modifiers::empty()
            ))
            .units(),
            0
        );
    }
}
