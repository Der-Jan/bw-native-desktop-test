//! Magnify launcher dispatch — hotkey handling, summon / hide, async copy.
//! Lives in its own window so it can't ride `ViewMessage` like screen views;
//! `Message::Magnify` arrives directly here.

use std::time::Instant;

use iced::Task;

use crate::{
    app::{App, Message},
    services::{clipboard::Sensitivity, cursor_monitor},
    views::magnify::{
        CopyField, MAGNIFY_RESULTS_SCROLL_ID, MAGNIFY_SEARCH_ID, MagnifyMessage, Mode, dims,
    },
};

impl App {
    pub(crate) fn handle_magnify_message(&mut self, msg: MagnifyMessage) -> Task<Message> {
        match msg {
            MagnifyMessage::HotkeyPressed => self.magnify_hotkey(),
            MagnifyMessage::WindowOpened(id) => {
                if id != self.magnify.window || !self.windows.contains_key(&id) {
                    return iced::window::close(id);
                }
                Task::batch([
                    apply_macos_window_fix(id),
                    iced::window::gain_focus(id),
                    iced::widget::operation::focus(MAGNIFY_SEARCH_ID),
                    iced::widget::operation::select_all(MAGNIFY_SEARCH_ID),
                    self.magnify_keep_selected_visible(),
                ])
            }
            MagnifyMessage::QueryChanged(query) => {
                self.magnify.query = query;
                // Fresh search — reset selection + scroll. `recompute` only
                // clamps; it preserves selection for sticky-restore summons.
                self.magnify.selected = 0;
                self.magnify.scroll_offset_y = 0.0;
                self.magnify_recompute();
                Task::batch([
                    self.magnify_resize_task(),
                    iced::widget::operation::scroll_to(
                        MAGNIFY_RESULTS_SCROLL_ID,
                        iced::widget::scrollable::AbsoluteOffset {
                            x: None,
                            y: Some(0.0),
                        },
                    ),
                ])
            }
            MagnifyMessage::NavigateUp => {
                if self.magnify.selected > 0 {
                    self.magnify.selected -= 1;
                }
                self.magnify_keep_selected_visible()
            }
            MagnifyMessage::NavigateDown => {
                let len = self.magnify.results.len();
                if len > 0 && self.magnify.selected + 1 < len {
                    self.magnify.selected += 1;
                }
                self.magnify_keep_selected_visible()
            }
            MagnifyMessage::Scrolled(viewport) => {
                self.magnify.scroll_offset_y = viewport.absolute_offset().y;
                Task::none()
            }
            MagnifyMessage::RowClicked(idx) => {
                if idx < self.magnify.results.len() {
                    self.magnify.selected = idx;
                }
                Task::none()
            }
            MagnifyMessage::CopyFieldRequested(field) => self.magnify_copy_field(field),
            MagnifyMessage::FieldDecryptCompleted(uid, cipher_id, field, result) => {
                // Stale completion: id/field mismatch, active-user changed,
                // or user re-locked mid-decrypt — must NOT copy a freshly-
                // decrypted secret to a now-locked session. Crucially, do
                // NOT clear `pending_decrypt` on a stale completion: a
                // *different* in-flight decrypt's completion would clobber
                // the still-valid pending request.
                if self.magnify.pending_decrypt != Some((cipher_id, field))
                    || self.active_user != Some(uid)
                    || !self.client_manager.is_unlocked(&uid)
                {
                    return Task::none();
                }
                self.magnify.pending_decrypt = None;
                match result {
                    Ok(Some(value)) => {
                        // Password / TOTP / notes are all "sensitive" by
                        // policy: keep them out of clipboard history and
                        // cloud-clipboard sync.
                        self.clipboard.copy(value, Sensitivity::Sensitive);
                    }
                    Ok(None) => {
                        tracing::debug!(%cipher_id, ?field, "magnify: cipher has no value for field");
                    }
                    Err(e) => {
                        // No toast: the main window is likely hidden (the
                        // launcher dismissed itself when the user pressed
                        // the shortcut), so a toast there would never be
                        // seen. Same rationale as the password path.
                        tracing::warn!(error = %e, %cipher_id, ?field, "magnify: field decrypt failed");
                    }
                }
                Task::none()
            }
            MagnifyMessage::CopyUsernameRequested => {
                if let Some(item) = self.magnify.selected_item() {
                    let subtitle = item.subtitle.as_str();
                    if !subtitle.is_empty() {
                        self.clipboard
                            .copy(subtitle.to_string(), Sensitivity::Normal);
                    }
                }
                self.magnify_hide()
            }
            MagnifyMessage::CopyUriRequested => {
                if let Some(uri) = self.magnify.selected_item().and_then(magnify_first_uri) {
                    self.clipboard.copy(uri, Sensitivity::Normal);
                }
                self.magnify_hide()
            }
            MagnifyMessage::Hide => self.magnify_hide(),
            MagnifyMessage::OpenMainWindow => self.magnify_open_main(),
        }
    }

    /// Routes a magnify-window key press to the appropriate `MagnifyMessage`.
    /// Called from `WindowMessage::KeyPressed` when `id == magnify.window`.
    pub(crate) fn handle_magnify_key(
        &mut self,
        ev: iced::keyboard::Event,
    ) -> Option<Task<Message>> {
        use iced::keyboard::{Event::KeyPressed, Key, key::Named};

        let KeyPressed { key, modifiers, .. } = ev else {
            return None;
        };

        let first_char = match &key {
            Key::Character(s) => s.chars().next(),
            _ => None,
        };
        let cmd = modifiers.command();
        let shift = modifiers.shift();
        let is_char = |c: char| first_char.is_some_and(|k| k.eq_ignore_ascii_case(&c));

        let msg = match (&key, cmd, shift) {
            (Key::Named(Named::Escape), _, _) => MagnifyMessage::Hide,
            // In `Mode::Unlocked` Enter is reserved for a future autotype
            // hand-off and stays a no-op here.
            (Key::Named(Named::Enter), _, _) if matches!(self.magnify.mode, Mode::Locked) => {
                MagnifyMessage::OpenMainWindow
            }
            (Key::Named(Named::ArrowUp), _, _) => MagnifyMessage::NavigateUp,
            (Key::Named(Named::ArrowDown), _, _) => MagnifyMessage::NavigateDown,
            // Cmd/Ctrl+Shift+C — copy username (sync, no decrypt).
            _ if cmd && shift && is_char('c') => MagnifyMessage::CopyUsernameRequested,
            // Cmd/Ctrl+Shift+N — copy notes (async decrypt).
            _ if cmd && shift && is_char('n') => {
                MagnifyMessage::CopyFieldRequested(CopyField::Notes)
            }
            // Cmd/Ctrl+C — copy password (async decrypt).
            _ if cmd && !shift && is_char('c') => {
                MagnifyMessage::CopyFieldRequested(CopyField::Password)
            }
            // Cmd/Ctrl+T — copy TOTP code (async decrypt + generator).
            _ if cmd && !shift && is_char('t') => {
                MagnifyMessage::CopyFieldRequested(CopyField::Totp)
            }
            // Cmd/Ctrl+U — copy URI (sync, the URI is on the list view).
            _ if cmd && !shift && is_char('u') => MagnifyMessage::CopyUriRequested,
            _ => return None,
        };
        Some(self.handle_magnify_message(msg))
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    fn magnify_hotkey(&mut self) -> Task<Message> {
        if self.windows.contains_key(&self.magnify.window) {
            return self.magnify_hide();
        }
        let now = Instant::now();
        let active = self.active_user;
        let unlocked = active.is_some_and(|uid| self.client_manager.is_unlocked(&uid));
        self.magnify.mode = if unlocked {
            Mode::Unlocked
        } else {
            Mode::Locked
        };

        // Reset sticky state when the user changed, locked, or the 5-minute
        // TTL expired; otherwise restore the previous query and selection.
        let user_changed = self.magnify.anchored_user != active;
        let stale = !self.magnify.sticky_state_fresh(now);
        if user_changed || !unlocked || stale {
            self.magnify.reset_search();
        }
        self.magnify.anchored_user = active;

        self.magnify_recompute();

        let height = dims::height_for(self.magnify.results.len());
        let (id, size, open) = open_magnify_window(height);
        self.magnify.window = id;
        self.windows.insert(
            id,
            crate::app::window::WindowInfo::new(crate::app::window::WindowKind::Magnify, size),
        );
        // A new widget tree starts at offset zero; restore selection on mount.
        self.magnify.scroll_offset_y = 0.0;
        open
    }

    fn magnify_hide(&mut self) -> Task<Message> {
        self.magnify.touch();
        if self.windows.remove(&self.magnify.window).is_some() {
            iced::window::close(self.magnify.window)
        } else {
            Task::none()
        }
    }

    fn magnify_open_main(&mut self) -> Task<Message> {
        let hide = self.magnify_hide();
        let show = self.show_main_window();
        let route = self.show_login_for_active();
        Task::batch([hide, show, route])
    }

    pub(crate) fn magnify_copy_field(&mut self, field: CopyField) -> Task<Message> {
        let Some(cipher_id) = self.magnify.selected_item().and_then(|item| item.id) else {
            return Task::none();
        };
        if let (Some(item), Some(uid)) = (self.magnify.selected_item(), self.active_user) {
            let target = crate::services::sdk::Authorization {
                cipher: Some(cipher_id),
                revision: Some(item.revision_date),
                action: crate::services::sdk::Action::Copy,
            };
            if item.reprompt == bitwarden_vault::CipherRepromptType::Password
                && !self.client_manager.permitted(&uid, &target, false)
            {
                self.set_screen(crate::domain::Screen::Vault);
                let show = self.show_main_window();
                let authorize = self.update(
                    crate::views::vault::VaultMessage::MagnifyCopy(cipher_id, field).into(),
                );
                return Task::batch([self.magnify_hide(), show, authorize]);
            }
        }
        self.magnify.pending_decrypt = Some((cipher_id, field));
        let decrypt = self.perform_with_active_client(
            move |client| async move {
                client
                    .full_cipher_for(cipher_id, crate::services::sdk::Action::Copy)
                    .await
                    .map(|view| extract_field(view, field))
            },
            move |uid, result| {
                Message::Magnify(MagnifyMessage::FieldDecryptCompleted(
                    uid, cipher_id, field, result,
                ))
            },
        );
        Task::batch([decrypt, self.magnify_hide()])
    }

    fn magnify_recompute(&mut self) {
        let items = self
            .active_user
            .as_ref()
            .and_then(|uid| self.views.vault.all_items_for(uid));
        match items {
            Some(items) => self.magnify.recompute(items),
            None => {
                self.magnify.results.clear();
                self.magnify.selected = 0;
                self.magnify.scroll_offset_y = 0.0;
            }
        }
    }

    fn magnify_resize_task(&self) -> Task<Message> {
        let height = dims::height_for(self.magnify.results.len());
        iced::window::resize(self.magnify.window, iced::Size::new(dims::WIDTH, height))
    }

    /// Scroll the selected row into view, or no-op if it's already visible
    /// (so navigation within the visible window doesn't jump). Callers reset
    /// `scroll_offset_y` to 0 first if the scrollable was freshly mounted.
    fn magnify_keep_selected_visible(&mut self) -> Task<Message> {
        if self.magnify.results.is_empty() {
            return Task::none();
        }
        let row_h = dims::ROW_HEIGHT;
        let viewport_h = dims::MAX_VISIBLE_ROWS as f32 * row_h;
        let selected_top = self.magnify.selected as f32 * row_h;
        let selected_bottom = selected_top + row_h;
        let visible_top = self.magnify.scroll_offset_y;
        let visible_bottom = visible_top + viewport_h;

        let new_offset_y = if selected_top < visible_top {
            Some(selected_top)
        } else if selected_bottom > visible_bottom {
            Some(selected_bottom - viewport_h)
        } else {
            None
        };

        match new_offset_y {
            Some(y) => {
                // Update the cached offset eagerly so a second arrow press
                // before the `on_scroll` callback round-trips sees it.
                self.magnify.scroll_offset_y = y;
                iced::widget::operation::scroll_to(
                    MAGNIFY_RESULTS_SCROLL_ID,
                    iced::widget::scrollable::AbsoluteOffset {
                        x: None,
                        y: Some(y),
                    },
                )
            }
            None => Task::none(),
        }
    }

    /// Called from lock / log-out / user-switch handlers so the next summon
    /// doesn't restore another user's query.
    pub(crate) fn magnify_reset_sticky(&mut self) {
        self.magnify.reset_search();
        self.magnify.anchored_user = self.active_user;
        self.magnify.last_used = None;
    }
}

/// Create a visible launcher only when explicitly summoned. Closing destroys
/// the surface while keeping sticky search state in the application model.
pub(crate) fn open_magnify_window(height: f32) -> (iced::window::Id, iced::Size, Task<Message>) {
    let size = iced::Size::new(dims::WIDTH, height);
    let (id, open_task) = iced::window::open(iced::window::Settings {
        size,
        min_size: Some(iced::Size::new(dims::WIDTH, dims::COLLAPSED_HEIGHT)),
        max_size: Some(iced::Size::new(
            dims::WIDTH,
            dims::height_for(dims::MAX_VISIBLE_ROWS),
        )),
        position: cursor_monitor::cursor_monitor_logical_center()
            .map(|(cx, cy)| {
                iced::window::Position::Specific(iced::Point::new(
                    cx - dims::WIDTH / 2.0,
                    cy - height / 2.0,
                ))
            })
            .unwrap_or(iced::window::Position::Centered),
        resizable: false,
        decorations: false,
        transparent: true,
        level: iced::window::Level::AlwaysOnTop,
        visible: true,
        exit_on_close_request: false,
        platform_specific: magnify_platform_specific(),
        ..Default::default()
    });
    (
        id,
        size,
        open_task.map(|id| Message::Magnify(MagnifyMessage::WindowOpened(id))),
    )
}

/// Apply macOS magnify-window transparency fixes. See
/// [`super::magnify_macos_fix`] for context. No-op on other platforms.
#[cfg(target_os = "macos")]
fn apply_macos_window_fix(id: iced::window::Id) -> Task<Message> {
    use crate::theme::RADIUS_XL;
    iced::window::run(id, |w| {
        super::magnify_macos_fix::apply(w, RADIUS_XL);
    })
    .discard()
}

#[cfg(not(target_os = "macos"))]
fn apply_macos_window_fix(_id: iced::window::Id) -> Task<Message> {
    Task::none()
}

/// Disable Windows' native rounded corners so the iced-side rounded
/// container isn't doubled (OS rounding clips the transparent edges
/// differently than our container's border radius).
fn magnify_platform_specific() -> iced::window::settings::PlatformSpecific {
    #[cfg(target_os = "windows")]
    {
        use iced::window::settings::PlatformSpecific;
        PlatformSpecific {
            undecorated_shadow: false,
            corner_preference: iced::window::settings::platform::CornerPreference::DoNotRound,
            skip_taskbar: true,
            ..Default::default()
        }
    }

    #[cfg(target_os = "macos")]
    {
        iced::window::settings::PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true,
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        iced::window::settings::PlatformSpecific::default()
    }
}

/// Pulls the requested field out of a freshly-decrypted `CipherView`.
/// Password and notes are direct field reads; TOTP runs the SDK's TOTP
/// generator at copy time so the clipboard always holds a still-valid
/// code. `None` covers both "field is empty" and "wrong cipher type"
/// (e.g. password on a SecureNote).
fn extract_field(view: bitwarden_vault::CipherView, field: CopyField) -> Option<String> {
    match field {
        CopyField::Password => match view.r#type {
            bitwarden_vault::CipherType::Login => view.login.and_then(|l| l.password),
            _ => None,
        },
        CopyField::Totp => match view.r#type {
            bitwarden_vault::CipherType::Login => view
                .login
                .and_then(|l| l.totp)
                .filter(|s| !s.is_empty())
                .and_then(|secret| bitwarden_vault::generate_totp(secret, None).ok())
                .map(|resp| resp.code),
            _ => None,
        },
        CopyField::Notes => view.notes.filter(|s| !s.is_empty()),
    }
}

/// First URI string off a `CipherListView`'s login summary — already
/// decrypted on the SDK side so no async hop is needed. `None` for
/// non-login items or items with no URIs.
fn magnify_first_uri(item: &std::sync::Arc<bitwarden_vault::CipherListView>) -> Option<String> {
    match &item.r#type {
        bitwarden_vault::CipherListViewType::Login(login) => login
            .uris
            .as_ref()
            .and_then(|uris| uris.first())
            .and_then(|u| u.uri.clone())
            .filter(|s| !s.is_empty()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn launcher_has_no_startup_surface_and_dismissal_destroys_it() {
        let mut app = App::test();
        assert!(
            !app.windows
                .values()
                .any(|w| w.kind == crate::app::window::WindowKind::Magnify)
        );
        let _ = app.magnify_hotkey();
        let first = app.magnify.window;
        assert!(app.windows.contains_key(&first));
        let _ = app.magnify_hotkey();
        assert!(!app.windows.contains_key(&first));
        let _ = app.magnify_hotkey();
        let second = app.magnify.window;
        assert_ne!(first, second);
        // A delayed open/close event from a dismissed surface cannot remove its replacement.
        let _ = app.handle_magnify_message(MagnifyMessage::WindowOpened(first));
        let _ = app.handle_window_message(crate::app::WindowMessage::Closed(first));
        assert!(app.windows.contains_key(&second));
        let _ = app.magnify_hide();
        assert!(!app.windows.contains_key(&second));
        assert!(!app.shutting_down);
    }
}
