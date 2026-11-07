//! `App::view` + the main-window composition (`view_main`).

use iced::Element;

use crate::{
    components::toast,
    domain::Screen,
    theme::AppTheme,
    views::{title_bar, title_bar::TitleBarMessage},
};

use super::{App, Message, Overlay, RenderCtx, SystemMessage, View, ViewExt, window::WindowKind};

impl App {
    pub fn view(&self, window_id: iced::window::Id) -> Element<'_, Message, AppTheme> {
        let element: Element<'_, Message, AppTheme> =
            match self.windows.get(&window_id).map(|w| w.kind) {
                Some(WindowKind::Main) => self.view_main(),
                Some(WindowKind::SshApproval) => match &self.ssh_approval {
                    Some(approval)
                        if self.ssh_approval_window.is_some_and(|(id, request)| {
                            id == window_id && request == approval.id
                        }) =>
                    {
                        crate::components::ssh_approval::view(approval, &self.theme.current.colors)
                            .map(|(id, action)| match action {
                                crate::components::ssh_approval::Action::Decide(decision) => {
                                    Message::SshDecision { id, decision }
                                }
                                crate::components::ssh_approval::Action::Focus(target) => {
                                    Message::SshFocus { id, target }
                                }
                            })
                    }
                    _ => iced::widget::Space::new().into(),
                },
                Some(WindowKind::About) => {
                    let rctx = self.render_ctx(window_id);
                    crate::views::about::view(&rctx).map(Into::into)
                }
                Some(WindowKind::Magnify) => {
                    let rctx = self.render_ctx(window_id);
                    crate::views::magnify::view(&self.magnify, &rctx).map(Into::into)
                }
                // Defensive: all windows are inserted at creation, but a stray
                // unknown id renders as empty space rather than panicking.
                None => iced::widget::Space::new().into(),
            };
        let session = self
            .active_user
            .as_ref()
            .and_then(|uid| self.client_manager.session_for(uid));
        let presentation = self.presentation_generation;
        element.map(move |message| {
            let sensitive = matches!(
                &message,
                Message::View(
                    super::ViewMessage::Login(_)
                        | super::ViewMessage::Vault(_)
                        | super::ViewMessage::Send(_)
                        | super::ViewMessage::Generator(_)
                        | super::ViewMessage::Import(_)
                        | super::ViewMessage::Export(_)
                        | super::ViewMessage::NewFolder(_)
                        | super::ViewMessage::NewItemPicker(_)
                        | super::ViewMessage::Fingerprint(_)
                ) | Message::Magnify(_)
            );
            if sensitive && let Some(session) = &session {
                use super::ctx::SessionScoped;
                message.for_session(session.clone(), presentation)
            } else if sensitive {
                Message::Presentation {
                    generation: presentation,
                    message: Box::new(message),
                }
            } else {
                message
            }
        })
    }

    /// Build a `RenderCtx` keyed to a specific window's width. Shared between
    /// `view_main` and the About / Magnify secondary windows. Also called
    /// from `test_support` to build a render context off an [`App::test`]
    /// instance.
    pub(crate) fn render_ctx(&self, window_id: iced::window::Id) -> RenderCtx<'_> {
        let active = self.active_account_entry();
        let window_width = self
            .windows
            .get(&window_id)
            .map(|info| info.size.width)
            .unwrap_or(1024.0);
        let (open_title_bar_menu, open_title_bar_submenu) = match self.open_overlay {
            Some(Overlay::TitleBarMenu { menu, submenu }) => (Some(menu), submenu),
            _ => (None, None),
        };
        RenderCtx {
            ssh_agent: &self.ssh_agent,
            active_vault_filter: self.sidebar.active_vault_filter,
            colors: &self.theme.current.colors,
            favicon: &self.favicon,
            // Private vault hostnames must not go to the public icon service.
            show_favicons: self.settings.show_favicons
                && !self
                    .active_user
                    .is_some_and(|uid| self.client_manager.is_remote(&uid)),
            remote_vault: self
                .active_user
                .is_some_and(|uid| self.client_manager.is_remote(&uid)),
            last_sync: self
                .active_user
                .and_then(|uid| self.client_manager.last_sync(&uid)),
            syncing: self
                .active_user
                .and_then(|uid| self.sync_status.get(&uid))
                .is_some_and(|s| s.operation.is_some()),
            sync_error: self
                .active_user
                .and_then(|uid| self.sync_status.get(&uid))
                .and_then(|s| s.error.as_deref()),
            window_width,
            active_user: self.active_user.as_ref(),
            active_email: active.map(|a| a.email.as_str()),
            active_server_url: active.map(|a| a.server_url.as_str()).unwrap_or(""),
            accounts: &self.cache.accounts,
            open_overlay: self.open_overlay,
            is_maximized: self.main_window_maximized(),
            menu: &self.menu,
            menu_state: self.menu_state(),
            open_title_bar_menu,
            open_title_bar_submenu,
        }
    }

    fn view_main(&self) -> Element<'_, Message, AppTheme> {
        let rctx = self.render_ctx(self.main_window_id());
        let colors = rctx.colors;

        // Vault / Send return just the right-hand content area — the sidebar
        // is composed below so it persists across screen switches without
        // each view re-rendering it.
        let inner: Element<'_, Message, AppTheme> = match self.screen {
            Screen::Loading => {
                iced::widget::center(crate::components::spinner::spinner(48.0, colors.accent))
                    .into()
            }
            Screen::Login => self.views.login.view(&rctx).map(Into::into),
            Screen::Vault => self.views.vault.view(&rctx).map(Into::into),
            Screen::Send => self.views.send.view(&rctx).map(Into::into),
        };

        let page: Element<'_, Message, AppTheme> =
            if matches!(self.screen, Screen::Vault | Screen::Send) {
                let organizations: &[crate::services::sdk::Organization] = self
                    .active_user
                    .as_ref()
                    .and_then(|uid| self.views.vault.organizations_for(uid))
                    .unwrap_or(&[]);
                let sidebar_el = crate::components::sidebar::view(
                    &self.sidebar,
                    organizations,
                    self.active_user
                        .as_ref()
                        .map(|uid| self.views.vault.folders_for(uid))
                        .unwrap_or(&[]),
                    self.active_user
                        .as_ref()
                        .map(|uid| self.views.vault.collections_for(uid))
                        .unwrap_or(&[]),
                    colors,
                )
                .map(Into::into);
                let main_row = iced::widget::container(
                    iced::widget::row![sidebar_el, inner].height(iced::Fill),
                )
                .width(iced::Fill)
                .height(iced::Fill)
                .style(|theme: &AppTheme| {
                    iced::widget::container::Style::default().background(theme.colors.header_bg)
                });
                iced::widget::container(main_row)
                    .width(iced::Fill)
                    .height(iced::Fill)
                    .style(|theme: &AppTheme| {
                        iced::widget::container::Style::default()
                            .background(theme.colors.background)
                    })
                    .into()
            } else {
                inner
            };

        let close_toast = |idx| Message::System(SystemMessage::CloseToast(idx));

        let mut overlays = self.collect_overlays(&rctx);

        let use_custom_menu_bar = crate::services::menu::should_use_custom_menu_bar();

        let tb: Element<'_, Message, AppTheme> = self.views.title_bar.view(&rctx).map(Into::into);

        let mut column = iced::widget::column![tb];
        if !self.pending_logouts.is_empty() {
            column = column.push(
                iced::widget::container(
                    iced::widget::text(crate::fl!("storage-logout-pending")).size(14),
                )
                .padding([12, 16])
                .width(iced::Fill),
            );
        }
        if let Some(warning) = self.storage_warning {
            let text = match warning {
                crate::services::sdk::StorageWarning::StorageUnavailable => {
                    crate::fl!("storage-accounts-unavailable")
                }
                crate::services::sdk::StorageWarning::RemovalIncomplete => {
                    crate::fl!("storage-logout-incomplete")
                }
                crate::services::sdk::StorageWarning::LogoutNotStored => {
                    crate::fl!("storage-logout-not-stored")
                }
            };
            column = column.push(
                iced::widget::container(iced::widget::text(text).size(14))
                    .padding([12, 16])
                    .width(iced::Fill),
            );
        }
        let main_column: Element<'_, Message, AppTheme> =
            column.push(page).height(iced::Fill).into();

        // Drag-by-titlebar overlay sits on top when any overlay is up. Only
        // meaningful with the custom title bar — macOS native already handles
        // drag.
        if use_custom_menu_bar && !overlays.is_empty() {
            let drag_strip = iced::widget::mouse_area(
                iced::widget::container(iced::widget::Space::new())
                    .width(iced::Fill)
                    .height(iced::Length::Fixed(title_bar::TITLE_BAR_HEIGHT)),
            )
            .on_press(TitleBarMessage::DragStart.into());
            let filler = iced::widget::container(iced::widget::Space::new())
                .width(iced::Fill)
                .height(iced::Fill);
            overlays.push(
                iced::widget::column![drag_strip, filler]
                    .width(iced::Fill)
                    .height(iced::Fill)
                    .into(),
            );
        }

        let stacked: Element<'_, Message, AppTheme> = if overlays.is_empty() {
            main_column
        } else {
            let mut layers = Vec::with_capacity(overlays.len() + 1);
            layers.push(main_column);
            layers.extend(overlays);
            iced::widget::Stack::with_children(layers)
                .width(iced::Fill)
                .height(iced::Fill)
                .into()
        };

        let with_toasts: Element<'_, Message, AppTheme> =
            toast::Manager::new(stacked, &self.toasts, close_toast).into();

        if use_custom_menu_bar {
            title_bar::resize_wrapper(with_toasts, |dir| TitleBarMessage::ResizeEdge(dir).into())
        } else {
            with_toasts
        }
    }

    pub fn title(&self, window_id: iced::window::Id) -> String {
        match self.windows.get(&window_id).map(|w| w.kind) {
            Some(WindowKind::About) => crate::fl!("about-window-title"),
            Some(WindowKind::SshApproval) => crate::fl!("ssh-approval-title"),
            _ => crate::fl!("app-title"),
        }
    }

    pub fn theme(&self, window_id: iced::window::Id) -> AppTheme {
        let theme = self.theme.current.clone();
        // Magnify launcher needs a transparent OS-window background so pixels
        // outside its rounded container stay see-through.
        if matches!(
            self.windows.get(&window_id).map(|w| w.kind),
            Some(WindowKind::Magnify)
        ) {
            theme.with_transparent_background()
        } else {
            theme
        }
    }

    /// Per-window UI scale fed back to iced. Only the main window honours the
    /// user's zoom setting — the About dialog and Magnify launcher have hand-
    /// tuned fixed layouts and stay at 1.0.
    pub fn scale_factor(&self, window_id: iced::window::Id) -> f32 {
        match self.windows.get(&window_id).map(|w| w.kind) {
            Some(WindowKind::Main) => self.settings.zoom_factor.scale(),
            _ => 1.0,
        }
    }

    /// Build the list of overlays (sub-modals, sheets, app-level modals) that
    /// stack above the active screen this frame, in z-order. The active
    /// screen's `view()` is rendered separately as the page content; its
    /// `overlays()` (sheet + sub-modal) come first, then the global modals
    /// (Settings, Generator, etc.) on top, then the small confirmation
    /// modals (fingerprint, screenshot confirm).
    fn collect_overlays<'a>(&'a self, rctx: &RenderCtx<'a>) -> Vec<Element<'a, Message, AppTheme>> {
        if let Some(approval) = &self.ssh_approval {
            return vec![crate::components::modal::view(
                iced::widget::container(
                    iced::widget::text(crate::fl!("ssh-approval-window-open"))
                        .color(rctx.colors.text_primary),
                )
                .padding(24)
                .style(|theme: &AppTheme| {
                    iced::widget::container::Style::default().background(theme.colors.card_bg)
                }),
                Message::SshDecision {
                    id: approval.id,
                    decision: crate::services::ssh_agent::Decision::Deny,
                },
                1.0,
            )];
        }
        let mut out: Vec<Element<'a, Message, AppTheme>> = Vec::new();

        // Active screen's sub-overlays. The screen's own `view()` is rendered
        // separately as page content, so only its `overlays()` go here.
        match self.screen {
            Screen::Vault => self.views.vault.push_overlays(rctx, &mut out),
            Screen::Send => self.views.send.push_overlays(rctx, &mut out),
            Screen::Login => self.views.login.push_overlays(rctx, &mut out),
            Screen::Loading => {}
        }

        // App-level modal views. Each renders as an overlay (their `view()`
        // is the modal dialog) plus any nested overlays they own. `Message`
        // is inferred from `out`; each view's local message converts via
        // `impl From<XxxMessage> for Message`.
        self.views.settings.push_render(rctx, &mut out);
        self.views.generator.push_render(rctx, &mut out);
        self.views.import.push_render(rctx, &mut out);
        self.views.export.push_render(rctx, &mut out);
        self.views.new_folder.push_render(rctx, &mut out);
        self.views.new_item_picker.push_render(rctx, &mut out);
        self.views.fingerprint.push_render(rctx, &mut out);
        self.views.screenshot_confirm.push_render(rctx, &mut out);

        out
    }
}
