use iced::Task;

use crate::{
    app::{App, Message},
    views::settings::{SettingChange, SettingsEvent},
};

impl App {
    pub(crate) fn handle_settings_event(&mut self, event: SettingsEvent) -> Task<Message> {
        match event {
            SettingsEvent::Applied(change) => self.apply_setting_change(change),
        }
    }

    /// Pull the view's working snapshot back into the persisted
    /// [`Settings`][crate::services::settings::Settings] (the view is the source of
    /// truth during the modal's lifetime), then run any runtime side effect
    /// that the change requires — a theme refresh, a language switch,
    /// clipboard-timeout push, tray respawn — or emit the generic
    /// "not supported yet" toast for stubbed fields.
    fn apply_setting_change(&mut self, change: SettingChange) -> Task<Message> {
        if matches!(
            change,
            SettingChange::CopySshSocket | SettingChange::CopySshSetup
        ) {
            return self.copy_and_toast(
                if matches!(change, SettingChange::CopySshSetup) {
                    self.ssh_agent.shell_setup()
                } else {
                    self.ssh_agent.path().display().to_string()
                },
                crate::services::clipboard::Sensitivity::Normal,
                if matches!(change, SettingChange::CopySshSetup) {
                    crate::fl!("ssh-agent-setup-copied")
                } else {
                    crate::fl!("ssh-agent-socket-copied")
                },
            );
        }
        // Keep navigation reconciliations that may have occurred during a background sync.
        self.views.settings.snapshot.settings.vault_filters = self.settings.vault_filters.clone();
        self.views
            .settings
            .snapshot
            .settings
            .collapsed_navigation_groups = self.settings.collapsed_navigation_groups.clone();
        self.views.settings.snapshot.settings.sidebar_collapsed = self.settings.sidebar_collapsed;
        self.views.settings.snapshot.settings.vault_sort = self.settings.vault_sort;
        // Copy every field the view just mutated back into App state. The
        // view's snapshot holds the fresh `Settings` (including the full
        // `user_preferences` map from open time) plus the active user's
        // freshly-edited `UserPreferences`; overwrite the active slot so
        // other users' prefs stay intact.
        self.settings = self.views.settings.snapshot.settings.clone();
        if let Some(uid) = self.active_user {
            self.settings
                .user_preferences
                .insert(uid, self.views.settings.snapshot.prefs);
        }
        self.settings.save();

        // Side effects: live-wired changes need a nudge, stubs get a toast.
        match change {
            SettingChange::Theme(p) => {
                self.theme.preference = p;
                self.theme.current = p.resolve(self.theme.system.as_deref());
            }
            SettingChange::Language(tag) => {
                let did_change = if tag.is_empty() {
                    // Empty = follow OS locale. Re-run the initial selection
                    // so the next `fl!()` call picks up the OS preference.
                    crate::services::i18n::init();
                    true
                } else if let Ok(lang_id) = tag.parse() {
                    crate::services::i18n::set_language(lang_id);
                    true
                } else {
                    tracing::warn!(%tag, "unparseable language tag; ignoring");
                    false
                };
                if did_change {
                    // Menu labels are baked at build time; rebuild so the
                    // custom dropdown + native muda menu pick up the new
                    // translations on the next frame.
                    self.rebuild_menu();
                }
            }
            SettingChange::ClearClipboard(delay) => {
                self.clipboard.set_timeout(delay.as_duration());
            }
            SettingChange::TrayEnabled(_)
            | SettingChange::MinimizeToTray(_)
            | SettingChange::CloseToTray(_) => {
                self.refresh_tray();
                if self.main_window_hidden && self.tray.is_none() {
                    return self.show_main_window();
                }
            }

            // No runtime side-effect needed — the setting is read where it
            // takes effect. `MinimizeOnCopy` is consumed in the vault
            // clipboard handler; `ShowFavicons` is consumed by the item-list
            // renderer;
            // `LockOnSystemLock` is read by the session-event handler in
            // `app/handlers/platform.rs` when an event fires.
            SettingChange::MinimizeOnCopy(_)
            | SettingChange::ShowFavicons(_)
            | SettingChange::LockOnSystemLock(_) => {}

            // Both timeouts are consumed by `services::session_timeout`'s
            // deadline computation — bump it now so the new value takes
            // effect without waiting for the next event.
            SettingChange::LockAfter(_) | SettingChange::LogoutAfter(_) => {
                self.refresh_session_timeout_deadline();
            }

            SettingChange::SshAgent(_)
            | SettingChange::SshPromptBehavior(_)
            | SettingChange::CopySshSocket
            | SettingChange::CopySshSetup => {}
            SettingChange::AllowScreenshots(allow) => {
                return self.apply_allow_screenshots_change(allow);
            }

            // Every remaining variant is currently unwired — the value was
            // persisted above, but the feature doesn't react yet. Let the
            // user know with a toast.
            SettingChange::OpenAtLogin(_)
            | SettingChange::PinUnlock(_)
            | SettingChange::TouchIdUnlock(_)
            | SettingChange::BrowserIntegration(_)
            | SettingChange::BrowserIntegrationFingerprint(_)
            | SettingChange::AutotypeEnabled(_)
            | SettingChange::AlwaysShowDock(_) => {
                self.push_toast(crate::views::settings::not_supported_toast());
            }
        }
        Task::none()
    }

    /// Toggle screen-capture protection on the main window. When enabling
    /// (`allow == false`), open the "confirm window still visible" dialog
    /// and schedule a 5 s timeout that auto-reverts if the user doesn't
    /// click OK — protects against the case where they're on a remote-
    /// desktop session and the window has just become invisible to them.
    /// Disabling is unconditional: clear protection, close the dialog if
    /// it happened to be open.
    fn apply_allow_screenshots_change(&mut self, allow: bool) -> Task<Message> {
        let protect = !allow;
        let apply_task =
            crate::services::screenshot_protection::apply(self.main_window, protect).discard();

        if !protect {
            // User just allowed screenshots — clear protection and any
            // in-flight confirm dialog (timeout will see a stale version
            // and no-op).
            self.views.screenshot_confirm.close();
            return apply_task;
        }

        // Linux: protection didn't actually do anything; show a one-shot
        // toast and skip the confirm dialog.
        if cfg!(not(any(target_os = "windows", target_os = "macos"))) {
            self.push_toast(crate::components::toast::Toast::warning(
                crate::fl!("settings-allow-screenshots-unsupported"),
                None,
            ));
            return apply_task;
        }

        // Windows / macOS: open the dialog and schedule the auto-revert.
        // `abortable` returns a handle whose `abort_on_drop` will cancel
        // the pending sleep when the user clicks OK (or re-opens the
        // modal), so the modal owns the lifetime of its own timeout.
        let revert_after = crate::views::screenshot_confirm::REVERT_AFTER;
        let timeout_task = Task::perform(tokio::time::sleep(revert_after), |_| {
            crate::views::screenshot_confirm::ScreenshotConfirmMessage::Timeout.into()
        });
        let (timeout_task, handle) = timeout_task.abortable();
        self.views.screenshot_confirm.open(handle);
        Task::batch([apply_task, timeout_task])
    }

    /// Ensure the tray reflects `self.settings.wants_tray()` — create one on
    /// enable, drop it on full disable. Called after any tray-related
    /// setting change.
    fn refresh_tray(&mut self) {
        let wants = self.settings.wants_tray();
        match (wants, self.tray.is_some()) {
            (true, false) => {
                self.tray = crate::services::tray::build();
                if self.tray.is_none() {
                    tracing::warn!("tray requested via settings but failed to initialise");
                }
            }
            (false, true) => {
                self.tray = None;
            }
            _ => {}
        }
    }
}
