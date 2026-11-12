//! Native egui frontend. Account authority and asynchronous work live in Core.
mod accent;
mod account;
mod auxiliary;
mod details;
mod dialogs;
mod labels;
mod merge;
mod sends;
#[cfg(test)]
mod tests;
mod typography;
mod vault;
pub mod window_loop;
use crate::{core::Core, domain::Screen, services};
use eframe::egui::{self, Id, Key};
use std::sync::Arc;
use zeroize::Zeroizing;

pub struct Application {
    pub surface: window_loop::Surface,
    pub core: Core,
    pub password: Zeroizing<String>,
    pub code: Zeroizing<String>,
    pub certificate_password: Zeroizing<String>,
    pub profile_open: bool,
    pub profile_base: String,
    pub profile_allowed: String,
    pub profile_loaded: Option<(uuid::Uuid, u64)>,
    pub clear_epoch: u64,
    list_epoch: u64,
    pub prompt_seen: Option<uuid::Uuid>,
    pub reveal: bool,
    pub focus_search: bool,
    pub focus_login: bool,
    pub challenge_seen: bool,
    pub list_focus: bool,
    pub list_offset: f32,
    pub restore_offset: Option<f32>,
    pub generator_mode: usize,
    pub generator_length: u8,
    pub generator_upper: bool,
    pub generator_lower: bool,
    pub generator_numbers: bool,
    pub generator_special: bool,
    pub generator_separator: String,
    pub generator_words: u8,
    pub generator_avoid_ambiguous: bool,
    pub generator_min_numbers: u8,
    pub generator_min_special: u8,
    pub username_mode: usize,
    pub username_email: String,
    pub username_domain: String,
    pub folder_seen: Option<uuid::Uuid>,
    pub placement_seen: Option<uuid::Uuid>,
    pub proxy_seen: Option<uuid::Uuid>,
    pub export_format: usize,
    pub export_password: Zeroizing<String>,
    pub export_confirm: Zeroizing<String>,
    pub export_organization: Option<bitwarden_core::OrganizationId>,
    pub always_on_top: bool,
    pub browser_seen: Option<uuid::Uuid>,
    pub browser_choice: services::browser_integration::Browser,
    pub ssh_seen: Option<uuid::Uuid>,
    pub ssh_viewport_open: bool,
    pub magnify_query: String,
    pub font_description: String,
    favicon_version: u64,
    pub icon_textures: std::collections::HashMap<String, egui::TextureHandle>,
    pub send_editor: Option<crate::ui::sends::SendDraft>,
    pub send_query: String,
    pub send_type: Option<bitwarden_send::SendType>,
}
impl Application {
    pub fn new_with_wake(
        cc: &eframe::CreationContext<'_>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let font_description = typography::install(&cc.egui_ctx)?;
        let core = Core::new(wake);
        Ok(Self::from_core(core, font_description))
    }
    pub(crate) fn from_core(core: Core, font_description: String) -> Self {
        let list_epoch = core.list_epoch;
        Self {
            surface: window_loop::Surface::Main,
            core,
            password: Zeroizing::default(),
            code: Zeroizing::default(),
            certificate_password: Zeroizing::default(),
            profile_open: false,
            profile_base: String::new(),
            profile_allowed: String::new(),
            profile_loaded: None,
            clear_epoch: 0,
            list_epoch,
            prompt_seen: None,
            reveal: false,
            focus_search: false,
            focus_login: true,
            challenge_seen: false,
            list_focus: false,
            list_offset: 0.,
            restore_offset: None,
            generator_mode: 0,
            generator_length: 20,
            generator_upper: true,
            generator_lower: true,
            generator_numbers: true,
            generator_special: true,
            generator_separator: "-".into(),
            generator_words: 6,
            generator_avoid_ambiguous: false,
            generator_min_numbers: 1,
            generator_min_special: 1,
            username_mode: 0,
            username_email: String::new(),
            username_domain: String::new(),
            folder_seen: None,
            placement_seen: None,
            proxy_seen: None,
            export_format: 0,
            export_password: Zeroizing::default(),
            export_confirm: Zeroizing::default(),
            export_organization: None,
            always_on_top: false,
            browser_seen: None,
            browser_choice: services::browser_integration::Browser::Helium,
            ssh_seen: None,
            ssh_viewport_open: false,
            magnify_query: String::new(),
            font_description,
            favicon_version: 0,
            icon_textures: Default::default(),
            send_editor: None,
            send_query: String::new(),
            send_type: None,
        }
    }
    fn cleanup(&mut self, ctx: &egui::Context) {
        let favicon_version = self.core.favicon.version();
        if self.favicon_version != favicon_version {
            self.favicon_version = favicon_version;
            self.icon_textures.clear();
        }
        let reset_list = self.list_epoch != self.core.list_epoch;
        if self.clear_epoch != self.core.clear_epoch {
            self.clear_epoch = self.core.clear_epoch;
            self.password = Zeroizing::default();
            self.focus_login = true;
            self.challenge_seen = false;
            self.code = Zeroizing::default();
            self.certificate_password = Zeroizing::default();
            self.export_password = Zeroizing::default();
            self.export_confirm = Zeroizing::default();
            self.export_organization = None;

            self.magnify_query.clear();
            self.reveal = false;
            self.send_editor = None;
            self.send_query.clear();
            self.icon_textures.clear();
            self.prompt_seen = None;
            let offset = self.restore_offset.unwrap_or(self.list_offset);
            self.list_offset = if reset_list { 0. } else { offset };
            self.restore_offset = (!reset_list).then_some(offset);
            ctx.memory_mut(|m| {
                let options = m.options.clone();
                *m = egui::Memory::default();
                m.options = options;
            });
            // Layout caches can contain plaintext glyph strings from the previous session.
            ctx.set_font_providers(vec![]);
        }
        if reset_list {
            self.list_epoch = self.core.list_epoch;
            self.list_offset = 0.;
            self.restore_offset = Some(0.);
        }
    }
    fn action(&mut self, ctx: &egui::Context, action: services::menu::MenuAction) {
        use services::menu::MenuAction::*;
        match action {
            Minimize => {
                if self.core.settings.minimize_to_tray {
                    if !self.core.hidden {
                        self.core.toggle_tray();
                    }
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            }
            Close => self.close(),
            ToggleAlwaysOnTop => {
                self.always_on_top = !self.always_on_top;
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.always_on_top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                }));
            }
            SearchVault => {
                self.core.menu_action(action);
                self.focus_search = true;
            }
            GeneratorHistory => {
                self.generator_mode = 3;
                self.core.menu_action(action);
            }
            _ => self.core.menu_action(action),
        }
    }
    fn close(&mut self) {
        if self.core.settings.close_to_tray {
            if !self.core.hidden {
                self.core.toggle_tray();
            }
            if !self.core.hidden {
                self.core.quit();
            }
        } else {
            self.core.quit();
        }
    }
    fn menus(&mut self, ui: &mut egui::Ui) {
        let tree = services::menu::menu_tree();
        let state = self.core.menu_state();
        let accounts = self.core.manager.accounts();
        let mut action = None;
        if let Some(menu) = self.core.native_menu.as_mut() {
            menu.sync_enabled(&state);
            menu.sync_dynamic(&accounts);
        }
        if !services::menu::should_use_custom_menu_bar() {
            return;
        }
        egui::MenuBar::new().ui(ui, |ui| {
            for section in tree.sections {
                ui.menu_button(section.label, |ui| {
                    menu_entries(ui, &section.entries, &state, &accounts, &mut action)
                });
            }
        });
        if let Some(action) = action {
            self.action(ui.ctx(), action);
        }
    }
}
impl eframe::App for Application {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.core.poll();
        self.cleanup(ctx);
        if self.core.settings.minimize_to_tray
            && !self.core.hidden
            && ctx.input(|i| i.viewport().minimized.unwrap_or(false))
        {
            self.core.toggle_tray();
        }
        if self.core.ready_to_quit()
            || (self.surface == window_loop::Surface::Main && self.core.hidden)
            || (self.surface == window_loop::Surface::Approval && self.core.ssh_approval.is_none())
            || (self.surface == window_loop::Surface::ProxyAuth && self.core.proxy_auth.is_none())
            || (self.surface == window_loop::Surface::BrowserApproval
                && self.core.browser_approval.is_none())
            || (self.surface == window_loop::Surface::Magnify && !self.core.magnify_requested)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        services::screenshot_protection::apply(frame, !self.core.settings.allow_screenshots);
        self.show(ui);
    }
}
impl Application {
    fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.core.poll();
        self.cleanup(&ctx);
        accent::apply(&ctx, self.core.appearance, self.core.settings.theme);
        ui.set_style(ctx.global_style());
        if let Some(action) = self.core.window_action.take() {
            self.action(&ctx, action);
        }
        let render_scope = self.core.scope();
        let copy_source = self.core.editor.as_ref().and_then(|e| e.source);
        if self.surface != window_loop::Surface::Main {
            self.standalone_auxiliary(ui);
            self.drain_clipboard(&ctx, render_scope, copy_source);
            return;
        }
        if ctx.input(|i| !i.events.is_empty()) {
            self.core.record_activity();
        }
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if self.core.focused != focused {
            self.core.focused = focused;
            if focused {
                self.core.check_incoming_now();
            }
            self.core.update_deadlines();
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.core.ready_to_quit()
            && !self.core.hidden
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close();
        }
        ctx.set_zoom_factor(self.core.settings.zoom_factor.scale());
        if !self.any_modal() {
            let action = ctx.input(|i| {
                i.events.iter().find_map(|event| {
                    if let egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } = event
                    {
                        services::menu::menu_tree().find_shortcut_action(
                            key,
                            *modifiers,
                            &self.core.menu_state(),
                        )
                    } else {
                        None
                    }
                })
            });
            if let Some(action) = action {
                self.action(&ctx, action);
            }
        }
        egui::Panel::top("application-menu").show(ui, |ui| self.menus(ui));
        if self.core.unlocked() {
            self.sidebar(ui);
            if self.core.detail_open {
                self.details_panel(ui);
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::central_panel(ui.style()).fill(ui.visuals().extreme_bg_color))
                .show(ui, |ui| {
                    ui.style_mut().visuals.override_text_color =
                        Some(accent::view_text(ui.ctx(), self.core.appearance));
                    match self.core.screen {
                        Screen::Send => self.sends(ui),
                        _ => self.vault(ui),
                    }
                });
        } else {
            egui::CentralPanel::default().show(ui, |ui| self.login(ui));
        }
        if let Some(notice) = &self.core.notice {
            let text = notice.text.clone();
            let error = notice.error;
            egui::Area::new(Id::unique("notice"))
                .anchor(egui::Align2::CENTER_BOTTOM, [0., -14.])
                .show(&ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        if error {
                            ui.colored_label(ui.visuals().error_fg_color, text);
                        } else {
                            ui.label(text);
                        }
                    });
                });
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        self.dialogs(&ctx);
        self.auxiliary_windows(&ctx);
        self.drain_clipboard(&ctx, render_scope, copy_source);
        if self.core.restore_requested {
            self.core.restore_requested = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if self.core.hidden {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn drain_clipboard(
        &mut self,
        ctx: &egui::Context,
        scope: Option<crate::core::Scope>,
        source: Option<bitwarden_vault::CipherId>,
    ) {
        let copies = ctx.output_mut(|output| {
            let mut copies = Vec::new();
            for command in std::mem::take(&mut output.commands) {
                match command {
                    egui::OutputCommand::CopyText(text) => copies.push(text),
                    command => output.commands.push(command),
                }
            }
            copies
        });
        if scope
            .as_ref()
            .is_some_and(|scope| scope.current(&self.core))
        {
            for text in copies {
                self.copy_widget_text(text, source);
            }
        }
    }
}
fn menu_entries(
    ui: &mut egui::Ui,
    entries: &[services::menu::MenuEntry],
    state: &services::menu::MenuState,
    accounts: &[services::sdk::AccountEntry],
    action: &mut Option<services::menu::MenuAction>,
) {
    for entry in entries {
        if entry.is_separator() {
            ui.separator();
            continue;
        }
        ui.add_enabled_ui(entry.enabled.check(state), |ui| {
            let children = entry.effective_children(accounts);
            if entry.is_submenu() {
                ui.menu_button(&entry.label, |ui| {
                    menu_entries(ui, children.as_ref(), state, accounts, action)
                });
            } else if ui
                .button(if let Some(shortcut) = entry.shortcut {
                    format!("{}    {}", entry.label, shortcut.display())
                } else {
                    entry.label.clone()
                })
                .clicked()
            {
                *action = entry.action;
                ui.close();
            }
        });
    }
}
/// Secret inputs never retain an undo copy in egui's persistent widget memory.
pub fn secret(
    ui: &mut egui::Ui,
    value: &mut String,
    id: impl std::hash::Hash + std::fmt::Debug,
    reveal: bool,
) -> egui::Response {
    let output = egui::TextEdit::singleline(value)
        .id_salt(id)
        .password(!reveal)
        .desired_width(f32::INFINITY)
        .show(ui);
    let mut state = output.state;
    state.clear_undoer();
    state.store(ui.ctx(), output.response.id);
    output.response.response
}
