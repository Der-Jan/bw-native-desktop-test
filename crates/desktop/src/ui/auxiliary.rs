use super::*;
use crate::{
    core::model::{CopyField, Request},
    services::ssh_agent::{Approval, Decision},
};
impl Application {
    pub(super) fn auxiliary_windows(&mut self, ctx: &egui::Context) {
        if let Some(approval) = self.core.ssh_approval.clone().filter(|a| a.current()) {
            let ssh_id = egui::ViewportId::from_hash_of(("ssh-approval", approval.id));
            let fresh = self.ssh_seen != Some(approval.id);
            self.ssh_seen = Some(approval.id);
            self.ssh_viewport_open = true;
            let mut decision = None;
            ctx.show_viewport_immediate(
                ssh_id,
                egui::ViewportBuilder::default()
                    .with_title("Allow SSH authentication")
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
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Critical,
            ));
    }
    egui::ScrollArea::vertical()
        .max_height(230.)
        .show(ui, |ui| {
            ui.heading(super::labels::text("Allow SSH authentication?"));
            ui.label(&approval.key_name);
            ui.monospace(&approval.fingerprint);
            ui.label(format!(
                "Requested by {} ({})",
                approval.process, approval.username
            ));
            if approval.hosts.is_empty() {
                ui.weak("Unverified destination: allow once only.");
            } else {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn focused_default_enter_and_escape_are_bound_to_current_approval() {
        for (remember, until, expected) in [
            (false, false, Decision::Once),
            (true, false, Decision::Once),
            (true, true, Decision::UntilLock),
        ] {
            let (_manager, approval) =
                crate::services::ssh_agent::test_ui_approval(remember, until).await;
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
}
