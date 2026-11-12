use super::*;
use crate::{
    core::model::{CopyField, Request},
    services::ssh_agent::{Approval, Decision},
};
impl Application {
    pub(super) fn auxiliary_windows(&mut self, ctx: &egui::Context) {
        if let Some(approval) = self.core.browser_approval.clone().filter(|a| a.current()) {
            let fresh = self.browser_seen != Some(approval.id);
            self.browser_seen = Some(approval.id);
            if let Some(decision) = browser_dialog(ctx, &approval, fresh) {
                self.core.decide_browser(approval.id, decision);
                self.browser_seen = None;
                ctx.request_repaint();
            }
        } else {
            self.browser_seen = None;
        }
        if let Some(approval) = self.core.ssh_approval.clone().filter(|a| a.current()) {
            let ssh_id = egui::ViewportId::from_hash_of(("ssh-approval", approval.id));
            let fresh = self.ssh_seen != Some(approval.id);
            self.ssh_seen = Some(approval.id);
            self.ssh_viewport_open = true;
            let mut decision = None;
            ctx.show_viewport_immediate(
                ssh_id,
                egui::ViewportBuilder::default()
                    .with_title(approval.title())
                    .with_app_id(crate::assets::DESKTOP_APP_ID)
                    .with_icon(crate::assets::window_icon().unwrap_or_default())
                    .with_inner_size([500., 350.])
                    .with_resizable(false)
                    .with_always_on_top()
                    .with_active(fresh),
                |ui, _class| {
                    if fresh {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    egui::CentralPanel::default()
                        .show(ui, |ui| decision = ssh_content(ui, &approval, fresh));
                },
            );
            if let Some(decision) = decision {
                self.core.decide_ssh(approval.id, decision);
                ctx.send_viewport_cmd_to(ssh_id, egui::ViewportCommand::Close);
                self.ssh_viewport_open = false;
            }
        } else if self.ssh_viewport_open {
            if let Some(id) = self.ssh_seen {
                ctx.send_viewport_cmd_to(
                    egui::ViewportId::from_hash_of(("ssh-approval", id)),
                    egui::ViewportCommand::Close,
                );
            }
            self.ssh_viewport_open = false;
            self.ssh_seen = None;
        }
        if self.core.magnify_requested {
            let mut close = false;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("magnify"),
                egui::ViewportBuilder::default()
                    .with_title("Bitwarden quick search")
                    .with_app_id(crate::assets::DESKTOP_APP_ID)
                    .with_icon(crate::assets::window_icon().unwrap_or_default())
                    .with_inner_size([600., 420.])
                    .with_always_on_top(),
                |ui, _| {
                    egui::CentralPanel::default().show(ui, |ui| close = self.magnify(ui));
                },
            );
            if close {
                self.core.magnify_requested = false;
                ctx.send_viewport_cmd_to(
                    egui::ViewportId::from_hash_of("magnify"),
                    egui::ViewportCommand::Close,
                );
            }
        }
    }
    pub(super) fn standalone_auxiliary(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        match self.surface {
            window_loop::Surface::BrowserApproval => {
                if let Some(approval) = self.core.browser_approval.clone().filter(|a| a.current()) {
                    let fresh = self.browser_seen != Some(approval.id);
                    self.browser_seen = Some(approval.id);
                    let mut decision = None;
                    egui::CentralPanel::default()
                        .show(ui, |ui| decision = browser_content(ui, &approval, fresh));
                    if let Some(decision) = decision {
                        self.core.decide_browser(approval.id, decision);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            window_loop::Surface::Approval => {
                if let Some(approval) = self.core.ssh_approval.clone().filter(|a| a.current()) {
                    let fresh = self.ssh_seen != Some(approval.id);
                    self.ssh_seen = Some(approval.id);
                    let mut decision = None;
                    egui::CentralPanel::default()
                        .show(ui, |ui| decision = ssh_content(ui, &approval, fresh));
                    if let Some(decision) = decision {
                        self.core.decide_ssh(approval.id, decision);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            window_loop::Surface::Magnify => {
                let mut close = false;
                egui::CentralPanel::default().show(ui, |ui| close = self.magnify(ui));
                if close || !self.core.hidden {
                    self.core.magnify_requested = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            window_loop::Surface::ProxyAuth => {
                let fresh =
                    self.proxy_seen != self.core.proxy_auth.as_ref().map(|f| f.challenge.id);
                self.proxy_seen = self.core.proxy_auth.as_ref().map(|f| f.challenge.id);
                egui::CentralPanel::default().show(ui, |ui| self.proxy_content(ui, fresh));
            }
            window_loop::Surface::Main => {}
        }
    }
    fn magnify(&mut self, ui: &mut egui::Ui) -> bool {
        let mut close = ui.input(|i| i.viewport().close_requested() || i.key_pressed(Key::Escape));
        ui.heading("Quick search");
        if !self.core.unlocked() {
            ui.label("Unlock your vault in Bitwarden.");
            if ui.button(super::labels::text("Open Bitwarden")).clicked() {
                self.core.hidden = false;
                self.core.restore_requested = true;
                close = true;
            }
            return close;
        }
        let search = ui.add(
            egui::TextEdit::singleline(&mut self.magnify_query)
                .hint_text("Search vault")
                .desired_width(f32::INFINITY),
        );
        if !ui.memory(|m| m.has_focus(search.id)) && self.magnify_query.is_empty() {
            search.request_focus();
        }
        let query = self.magnify_query.to_lowercase();
        let mut request = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(cache) = self.core.current_cache() {
                for item in cache
                    .all
                    .iter()
                    .filter(|v| {
                        v.deleted_date.is_none()
                            && v.archived_date.is_none()
                            && v.name.to_lowercase().contains(&query)
                    })
                    .take(100)
                {
                    if let Some(id) = item.id {
                        ui.horizontal(|ui| {
                            if ui.button(&item.name).clicked() {
                                request = Some(Request::Read(id));
                            }
                            if ui.small_button(super::labels::text("Username")).clicked() {
                                request = Some(Request::Copy(id, CopyField::Username));
                            }
                            if ui.small_button(super::labels::text("Password")).clicked() {
                                request = Some(Request::Copy(id, CopyField::Password));
                            }
                        });
                    }
                }
            }
        });
        if let Some(request) = request {
            let read = matches!(request, Request::Read(_));
            self.core.request(request);
            if read || self.core.prompt.is_some() {
                self.core.hidden = false;
                self.core.restore_requested = true;
            }
            close = true;
        }
        close
    }
}
fn ssh_content(ui: &mut egui::Ui, approval: &Approval, fresh: bool) -> Option<Decision> {
    if fresh {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Title(approval.title().to_owned()));
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Critical,
            ));
    }
    egui::ScrollArea::vertical()
        .max_height(230.)
        .show(ui, |ui| {
            let git = approval.purpose == crate::services::ssh_agent::SigningPurpose::Git;
            ui.heading(super::labels::text(if git { "Allow Git signing?" } else { "Allow SSH authentication?" }));
            ui.label(&approval.key_name);
            ui.monospace(&approval.fingerprint);
            if git {
                ui.label(format!("Requested by {}", approval.process));
                ui.label("Sign Git data with this key (namespace: git).");
                ui.weak("Git-compatible request: the content hash does not identify a repository or show what is being signed.");
                ui.weak("Allow until lock applies only to Git signing with this key.");
            } else {
                ui.label(format!("Requested by {} ({})", approval.process, approval.username));
            }
            if !git && approval.hosts.is_empty() {
                ui.weak("Unverified destination: allow once only.");
            } else if !git {
                ui.label("Verified host binding:");
                for host in &approval.hosts {
                    ui.monospace(host);
                }
            }
        });
    let mut decision = None;
    ui.add_space(12.);
    ui.horizontal(|ui| {
        let default = if approval.can_remember {
            approval.default_decision
        } else {
            Decision::Once
        };
        let allow = ui.button(if default == Decision::UntilLock {
            "Allow until lock"
        } else {
            "Allow once"
        });
        if fresh {
            allow.request_focus();
        }
        if allow.clicked()
            || ((allow.has_focus() || fresh)
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter)))
        {
            decision = Some(default);
        }
        if approval.can_remember
            && ui
                .button(if default == Decision::UntilLock {
                    "Allow once"
                } else {
                    "Allow until lock"
                })
                .clicked()
        {
            decision = Some(if default == Decision::UntilLock {
                Decision::Once
            } else {
                Decision::UntilLock
            });
        }
        if ui.button(super::labels::text("Deny")).clicked() {
            decision = Some(Decision::Deny);
        }
    });
    if ui.input(|i| i.viewport().close_requested())
        || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
    {
        decision = Some(Decision::Deny);
    }
    decision
}

fn browser_shield(ui: &mut egui::Ui) {
    let id = Id::unique("browser-approval-shield");
    let mut texture = ui
        .ctx()
        .data(|data| data.get_temp::<egui::TextureHandle>(id));
    if texture.is_none()
        && let Some(icon) = crate::assets::window_icon()
    {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [icon.width as usize, icon.height as usize],
            &icon.rgba,
        );
        let handle = ui.ctx().load_texture(
            "browser-approval-shield",
            image,
            egui::TextureOptions::LINEAR,
        );
        ui.ctx()
            .data_mut(|data| data.insert_temp(id, handle.clone()));
        texture = Some(handle);
    }
    if let Some(texture) = texture {
        ui.image((texture.id(), egui::vec2(40., 40.)));
    }
}

// Keep approval in the existing GL context. eframe's immediate native
// viewport path panics if GLX window creation fails before invoking its callback.
fn browser_dialog(
    ctx: &egui::Context,
    approval: &services::browser_integration::Approval,
    fresh: bool,
) -> Option<services::browser_integration::Decision> {
    use services::browser_integration::Decision;
    if !approval.current() {
        return Some(Decision::Deny);
    }
    let mut decision = None;
    let response =
        egui::Modal::new(Id::unique(("browser-approval", approval.id))).show(ctx, |ui| {
            ui.set_width(540.);
            decision = browser_content(ui, approval, fresh);
        });
    if response.should_close() && decision.is_none() {
        decision = Some(Decision::Deny);
    }
    decision
}

fn browser_content(
    ui: &mut egui::Ui,
    approval: &services::browser_integration::Approval,
    fresh: bool,
) -> Option<services::browser_integration::Decision> {
    use services::browser_integration::Decision;
    ui.style_mut().interaction.selectable_labels = false;
    if fresh {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Critical,
            ));
    }
    let mut decision = None;
    egui::Frame::new().inner_margin(20.).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 12.;
        ui.horizontal(|ui| {
            browser_shield(ui);
            ui.add_space(8.);
            ui.vertical(|ui| {
                ui.heading(format!("Connect {}", approval.browser));
                ui.weak("Bitwarden browser extension");
            });
        });
        egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(8.).inner_margin(14.).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.strong(&approval.email);
            ui.add(egui::Label::new(&approval.server).wrap());
        });
        ui.label("Allow the extension to unlock this account while your desktop vault is unlocked.");
        let choice_id = Id::unique(("browser-permission-duration", approval.id));
        let mut choice = ui.ctx().data(|d| d.get_temp::<Decision>(choice_id)).unwrap_or(Decision::Always);
        ui.horizontal(|ui| {
            ui.label("Permission");
            egui::ComboBox::from_id_salt("browser-permission-duration").width(230.)
                .selected_text(match choice {
                    Decision::Always => "Always allow",
                    Decision::UntilLock => "Until desktop locks",
                    _ => "Just this once",
                }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut choice, Decision::Always, "Always allow");
                    ui.selectable_value(&mut choice, Decision::UntilLock, "Until desktop locks");
                    ui.selectable_value(&mut choice, Decision::Once, "Just this once");
                });
        });
        ui.ctx().data_mut(|d| d.insert_temp(choice_id, choice));
        let details = egui::CollapsingHeader::new("Connection details").id_salt(("browser-details", approval.id)).show(ui, |ui| {
            ui.label("The extension receives this account’s vault key, never your master password. Check that both apps use this account and server.");
            ui.weak("Desktop approval only; no biometric verification.");
            ui.add(egui::Label::new(&approval.extension).wrap());
        });
        if details.header_response.clicked() {
            let id = Id::unique(("browser-details-expanded", approval.id));
            let expanded = !ui.ctx().data(|data| data.get_temp::<bool>(id).unwrap_or(false));
            ui.ctx().data_mut(|data| data.insert_temp(id, expanded));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                ui.ctx().input(|input| input.viewport_rect().width()),
                if expanded { 560. } else { 420. },
            )));
        }
        ui.horizontal(|ui| { ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let text = match choice {
                Decision::Always => "Always allow",
                Decision::UntilLock => "Allow until lock",
                _ => "Allow once",
            };
            let allow = ui.add(egui::Button::new(egui::RichText::new(text).color(ui.visuals().selection.stroke.color))
                .fill(ui.visuals().selection.bg_fill).min_size(egui::vec2(150., 36.)));
            if fresh { allow.request_focus(); }
            if allow.clicked() || ((allow.has_focus() || fresh) && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter))) {
                decision = Some(choice);
            }
            if ui.add(egui::Button::new("Deny").min_size(egui::vec2(80., 36.))).clicked() {
                decision = Some(Decision::Deny);
            }
        }); });
        ui.weak("You can remove saved approvals in Settings.");
    });
    if ui.input(|i| i.viewport().close_requested())
        || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
    {
        decision = Some(Decision::Deny);
    }
    decision
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn focused_default_enter_and_escape_are_bound_to_current_approval() {
        use crate::services::ssh_agent::SigningPurpose::{Authentication, Git};
        for (remember, until, purpose, expected) in [
            (false, false, Authentication, Decision::Once),
            (true, false, Authentication, Decision::Once),
            (true, true, Authentication, Decision::UntilLock),
            (true, true, Git, Decision::Once),
        ] {
            let (_manager, approval) =
                crate::services::ssh_agent::test_ui_approval(remember, until, purpose).await;
            assert!(approval.current());
            let ctx = egui::Context::default();
            let mut fonts = egui::FontDefinitions::empty();
            fonts.font_data.insert(
                "test".into(),
                egui::FontData::from_static(crate::assets::FONT_MEDIUM).into(),
            );
            fonts
                .families
                .insert(egui::FontFamily::Proportional, vec!["test".into()]);
            ctx.set_fonts(fonts);
            let run = |key: Option<Key>, fresh: bool| {
                let mut decision = None;
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        events: key
                            .into_iter()
                            .map(|key| egui::Event::Key {
                                key,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: egui::Modifiers::NONE,
                            })
                            .collect(),
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default()
                            .show(ui, |ui| decision = ssh_content(ui, &approval, fresh));
                    },
                );
                if fresh {
                    assert!(
                        output.viewport_output[&egui::ViewportId::ROOT]
                            .commands
                            .contains(&egui::ViewportCommand::Focus)
                    );
                }
                output.textures_delta.clear();
                decision
            };
            assert_eq!(run(None, true), None);
            assert!(ctx.memory(|m| m.focused()).is_some());
            assert_eq!(run(Some(Key::Enter), false), Some(expected));
            assert_eq!(run(Some(Key::Escape), false), Some(Decision::Deny));
            approval.respond(Decision::Deny);
        }
    }
    #[test]
    fn browser_pairing_defaults_to_always_with_enter_and_escape() {
        use crate::services::browser_integration::{self, Decision as BrowserDecision};
        let (_runtime, _manager, approval, _response) = browser_integration::test_ui_approval();
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::empty();
        fonts.font_data.insert(
            "test".into(),
            egui::FontData::from_static(crate::assets::FONT_MEDIUM).into(),
        );
        fonts
            .families
            .insert(egui::FontFamily::Proportional, vec!["test".into()]);
        ctx.set_fonts(fonts);
        let run = |key: Option<Key>, fresh: bool| {
            let mut decision = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(580., 420.),
                    )),
                    events: key
                        .into_iter()
                        .map(|key| egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        })
                        .collect(),
                    ..Default::default()
                },
                |ui| {
                    decision = browser_dialog(ui.ctx(), &approval, fresh);
                },
            );
            assert_eq!(
                output.viewport_output.len(),
                1,
                "browser approval must not create a native GLX viewport"
            );
            output.textures_delta.clear();
            decision
        };
        assert!(run(None, true).is_none());
        assert!(ctx.memory(|m| m.focused()).is_some());
        assert!(run(Some(Key::Enter), false) == Some(BrowserDecision::Always));
        for choice in [BrowserDecision::Once, BrowserDecision::UntilLock] {
            ctx.data_mut(|d| {
                d.insert_temp(
                    Id::unique(("browser-permission-duration", approval.id)),
                    choice,
                )
            });
            run(None, true);
            assert!(run(Some(Key::Enter), false) == Some(choice));
        }
        assert!(run(Some(Key::Escape), false) == Some(BrowserDecision::Deny));
    }
}
