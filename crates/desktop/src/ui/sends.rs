use super::*;
use bitwarden_send::{SendType, SendView};
use serde_json::{Value, json};
pub struct SendDraft {
    data: Value,
    emails: String,
    limit: String,
}
impl Drop for SendDraft {
    fn drop(&mut self) {
        crate::core::model::zero_value(&mut self.data);
    }
}
impl SendDraft {
    fn new() -> Self {
        let now = chrono::Utc::now();
        Self {
            data: json!({"name":"","type":0,"text":{"text":"","hidden":false},"hasPassword":false,"accessCount":0,"disabled":false,"hideEmail":false,"revisionDate":now,"deletionDate":now+chrono::Duration::days(7),"emails":[],"authType":2}),
            emails: String::new(),
            limit: String::new(),
        }
    }
    fn edit(view: SendView) -> Self {
        let emails = view.emails.join(", ");
        let limit = view
            .max_access_count
            .map(|n| n.to_string())
            .unwrap_or_default();
        Self {
            data: serde_json::to_value(view).expect("send schema"),
            emails,
            limit,
        }
    }
    fn view(&self) -> Result<SendView, String> {
        let mut data = self.data.clone();
        data["emails"] = json!(
            self.emails
                .split([',', ';', '\n'])
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        );
        data["maxAccessCount"] = if self.limit.trim().is_empty() {
            Value::Null
        } else {
            json!(
                self.limit
                    .parse::<u32>()
                    .map_err(|_| "Invalid access limit")?
            )
        };
        serde_json::from_value(data).map_err(|_| "Invalid Send fields".into())
    }
}
impl Application {
    pub(super) fn sends(&mut self, ui: &mut egui::Ui) {
        ui.heading(super::labels::text("Send"));
        if self.core.remote() {
            ui.label("Send is not connected to the server yet.");
            return;
        }
        ui.weak("Local session only — sends are not published to a server.");
        let Some(uid) = self.core.active else {
            return;
        };
        ui.horizontal(|ui| {
            if ui.button("New text Send").clicked() {
                self.send_editor = Some(SendDraft::new());
            }
            ui.selectable_value(&mut self.send_type, None, super::labels::text("All"));
            ui.selectable_value(
                &mut self.send_type,
                Some(SendType::Text),
                super::labels::text("Text"),
            );
            ui.selectable_value(
                &mut self.send_type,
                Some(SendType::File),
                super::labels::text("Files"),
            );
        });
        ui.add(egui::TextEdit::singleline(&mut self.send_query).hint_text("Search Sends"));
        let sends = self.core.manager.list_sends(&uid);
        let query = self.send_query.to_lowercase();
        let mut edit = None;
        let mut delete = None;
        egui::ScrollArea::vertical()
            .id_salt("send-list")
            .max_height(if self.send_editor.is_some() {
                180.
            } else {
                ui.available_height()
            })
            .show(ui, |ui| {
                for send in sends.into_iter().filter(|s| {
                    self.send_type.is_none_or(|t| s.r#type == t)
                        && s.name.to_lowercase().contains(&query)
                }) {
                    ui.horizontal(|ui| {
                        if ui.button(&send.name).clicked() {
                            edit = Some(send.clone());
                        }
                        ui.weak(send.deletion_date.format("Expires %Y-%m-%d").to_string());
                        if let Some(id) = send.id
                            && ui.small_button(super::labels::text("Delete")).clicked()
                        {
                            delete = Some(id);
                        }
                    });
                }
            });
        if let Some(view) = edit {
            self.send_editor = Some(SendDraft::edit(view));
        }
        if let Some(id) = delete {
            self.core.manager.delete_send(&uid, id);
        }
        let mut save = false;
        let mut cancel = false;
        if let Some(draft) = &mut self.send_editor {
            ui.separator();
            ui.horizontal(|ui| {
                save = ui.button("Save locally").clicked();
                cancel = ui.button(super::labels::text("Cancel")).clicked();
            });
            egui::ScrollArea::vertical()
                .id_salt("send-form")
                .show(ui, |ui| {
                    super::details::edit_field(ui, &mut draft.data, "name", "Name", false);
                    if draft.data["type"] == 0 {
                        ui.label(super::labels::text("Text"));
                        let mut text = draft.data["text"]["text"].as_str().unwrap_or("").to_owned();
                        let response = ui.add(
                            egui::TextEdit::multiline(&mut text)
                                .desired_rows(4)
                                .desired_width(f32::INFINITY),
                        );
                        if response.changed() {
                            draft.data["text"]["text"] = json!(text);
                        }
                        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                            state.clear_undoer();
                            state.store(ui.ctx(), response.id);
                        }
                        let mut hidden = draft.data["text"]["hidden"].as_bool().unwrap_or(false);
                        if ui
                            .checkbox(&mut hidden, super::labels::text("Hide text"))
                            .changed()
                        {
                            draft.data["text"]["hidden"] = json!(hidden);
                        }
                    } else {
                        ui.label(draft.data["file"]["fileName"].as_str().unwrap_or("File"));
                        ui.weak("File upload is not implemented.");
                    }
                    egui::ComboBox::from_id_salt("send-deletion")
                        .selected_text("Deletion date")
                        .show_ui(ui, |ui| {
                            for days in [1, 2, 3, 7, 14, 30] {
                                if ui.button(format!("In {days} days")).clicked() {
                                    draft.data["deletionDate"] =
                                        json!(chrono::Utc::now() + chrono::Duration::days(days));
                                    ui.close();
                                }
                            }
                        });
                    ui.small(draft.data["deletionDate"].as_str().unwrap_or(""));
                    let mut auth = draft.data["authType"].as_u64().unwrap_or(2);
                    egui::ComboBox::from_id_salt("send-auth")
                        .selected_text(match auth {
                            0 => "Specific people",
                            1 => "Password",
                            _ => "Anyone with the link",
                        })
                        .show_ui(ui, |ui| {
                            for (value, label) in [
                                (2, "Anyone with the link"),
                                (0, "Specific people"),
                                (1, "Password"),
                            ] {
                                ui.selectable_value(&mut auth, value, label);
                            }
                        });
                    draft.data["authType"] = json!(auth);
                    if auth == 0 {
                        ui.label("Email addresses, separated by commas");
                        ui.text_edit_singleline(&mut draft.emails);
                    } else {
                        draft.emails.clear();
                    }
                    if auth == 1 {
                        super::details::edit_field(
                            ui,
                            &mut draft.data,
                            "newPassword",
                            "Password",
                            true,
                        );
                        if draft.data["newPassword"]
                            .as_str()
                            .is_some_and(|s| !s.is_empty())
                        {
                            draft.data["hasPassword"] = json!(true);
                        }
                    } else {
                        draft.data["hasPassword"] = json!(false);
                        draft.data["newPassword"] = Value::Null;
                    }
                    ui.label("Maximum access count (empty = unlimited)");
                    ui.text_edit_singleline(&mut draft.limit);
                    let mut hide = draft.data["hideEmail"].as_bool().unwrap_or(false);
                    if ui
                        .checkbox(&mut hide, super::labels::text("Hide my email address"))
                        .changed()
                    {
                        draft.data["hideEmail"] = json!(hide);
                    }
                    super::details::edit_field(ui, &mut draft.data, "notes", "Notes", false);
                });
        }
        if save && let Some(draft) = &self.send_editor {
            match draft.view() {
                Ok(view)
                    if !view.name.trim().is_empty()
                        && view
                            .text
                            .as_ref()
                            .and_then(|t| t.text.as_ref())
                            .is_some_and(|t| !t.trim().is_empty()) =>
                {
                    self.core.manager.save_send(uid, view);
                    self.send_editor = None;
                    self.core.notify("Send saved locally", false);
                }
                Ok(_) => self.core.notify("Enter a name and text", true),
                Err(e) => self.core.notify(e, true),
            }
        }
        if cancel {
            self.send_editor = None;
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.send_editor = None;
        }
    }
}
