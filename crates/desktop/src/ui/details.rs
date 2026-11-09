use super::*;
use crate::core::model::{CopyField, Request};
use serde_json::{Value, json};
/// Human-readable SDK field schemas, shared by read and edit presentation.
type FieldSchema = (&'static str, &'static str, bool);
type SectionSchema = (&'static str, &'static str, &'static [FieldSchema]);
const SECTIONS: &[SectionSchema] = &[
    (
        "login",
        "Login",
        &[
            ("username", "Username", false),
            ("password", "Password", true),
            ("totp", "Authenticator key (TOTP)", true),
        ],
    ),
    (
        "card",
        "Card",
        &[
            ("cardholderName", "Cardholder name", false),
            ("brand", "Brand", false),
            ("number", "Number", true),
            ("expMonth", "Expiry month", false),
            ("expYear", "Expiry year", false),
            ("code", "Security code", true),
        ],
    ),
    (
        "identity",
        "Identity",
        &[
            ("title", "Title", false),
            ("firstName", "First name", false),
            ("middleName", "Middle name", false),
            ("lastName", "Last name", false),
            ("username", "Username", false),
            ("company", "Company", false),
            ("ssn", "Social security number", true),
            ("passportNumber", "Passport number", true),
            ("licenseNumber", "License number", true),
            ("email", "Email", false),
            ("phone", "Phone", false),
            ("address1", "Address", false),
            ("address2", "Address line 2", false),
            ("address3", "Address line 3", false),
            ("city", "City", false),
            ("state", "State", false),
            ("postalCode", "Postal code", false),
            ("country", "Country", false),
        ],
    ),
    (
        "sshKey",
        "SSH key",
        &[
            ("privateKey", "Private key", true),
            ("publicKey", "Public key", false),
            ("fingerprint", "Fingerprint", false),
        ],
    ),
    (
        "bankAccount",
        "Bank account",
        &[
            ("bankName", "Bank name", false),
            ("nameOnAccount", "Name on account", false),
            ("accountType", "Account type", false),
            ("accountNumber", "Account number", true),
            ("routingNumber", "Routing number", true),
            ("branchNumber", "Branch number", false),
            ("pin", "PIN", true),
            ("swiftCode", "SWIFT code", false),
            ("iban", "IBAN", true),
            ("bankContactPhone", "Bank contact phone", false),
        ],
    ),
    (
        "driversLicense",
        "Driver’s license",
        &[
            ("firstName", "First name", false),
            ("middleName", "Middle name", false),
            ("lastName", "Last name", false),
            ("dateOfBirth", "Date of birth (YYYY-MM-DD)", false),
            ("licenseNumber", "License number", true),
            ("issuingCountry", "Issuing country", false),
            ("issuingState", "Issuing state", false),
            ("issueDate", "Issue date (YYYY-MM-DD)", false),
            ("expirationDate", "Expiration date (YYYY-MM-DD)", false),
            ("issuingAuthority", "Issuing authority", false),
            ("licenseClass", "License class", false),
        ],
    ),
    (
        "passport",
        "Passport",
        &[
            ("surname", "Surname", false),
            ("givenName", "Given name", false),
            ("dateOfBirth", "Date of birth (YYYY-MM-DD)", false),
            ("sex", "Sex", false),
            ("birthPlace", "Place of birth", false),
            ("nationality", "Nationality", false),
            ("issuingCountry", "Issuing country", false),
            ("passportNumber", "Passport number", true),
            ("passportType", "Passport type", false),
            (
                "nationalIdentificationNumber",
                "National identification number",
                true,
            ),
            ("issuingAuthority", "Issuing authority", false),
            ("issueDate", "Issue date (YYYY-MM-DD)", false),
            ("expirationDate", "Expiration date (YYYY-MM-DD)", false),
        ],
    ),
];
impl Application {
    pub(super) fn details_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("item-details")
            .frame(egui::Frame::side_top_panel(ui.style()).fill(ui.visuals().extreme_bg_color))
            .default_size(390.)
            .min_size(280.)
            .resizable(true)
            .show(ui, |ui| {
                ui.style_mut().visuals.override_text_color =
                    Some(super::accent::view_text(ui.ctx(), self.core.appearance));
                let title = self.core.editor.as_ref()
                    .map(|editor| editor.data["name"].as_str().filter(|name| !name.is_empty()).unwrap_or("New item").to_owned())
                    .or_else(|| self.core.detail.as_ref().map(|detail| detail.name.clone()))
                    .unwrap_or_default();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    // Lay out the button first so the title uses the actual
                    // remaining space, including theme padding and item spacing.
                    if ui.button("×").on_hover_text("Close (Escape)").clicked() {
                        self.core.close_details();
                        self.list_focus = true;
                    }
                    let title_width = ui.available_width();
                    ui.allocate_ui_with_layout(egui::vec2(title_width, 0.), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_min_width(title_width);
                        ui.add(egui::Label::new(egui::RichText::new(title).size(18.)).wrap());
                    });
                });
                ui.separator();
                if self.core.editor.is_some() {
                    self.editor(ui);
                    return;
                }
                let Some(detail) = &self.core.detail else {
                    ui.spinner();
                    return;
                };
                let id = detail.id;
                let vault_name = detail.organization_id.map_or_else(
                    || "My vault".to_owned(),
                    |id| self.core.active.and_then(|uid| self.core.manager.list_organizations(&uid).into_iter().find(|org| org.id == id))
                        .map_or_else(|| "Unknown organization".to_owned(), |org| org.name),
                );
                let collection_names: Vec<_> = detail.collection_ids.iter().map(|id| {
                    self.core.current_cache().and_then(|cache| cache.collections.iter().find(|collection| collection.id == *id && Some(collection.organization_id) == detail.organization_id))
                        .map_or_else(|| "Unknown collection".to_owned(), |collection| collection.name.clone())
                }).collect();
                let folder_name = detail.folder_id.map_or_else(
                    || "No folder".to_owned(),
                    |id| {
                        self.core
                            .current_cache()
                            .and_then(|cache| cache.folders.iter().find(|folder| folder.id == id))
                            .map_or_else(|| "Unknown folder".to_owned(), |folder| folder.name.clone())
                    },
                );
                let data = serde_json::to_value(detail).unwrap_or(Value::Null);
                let mut copy = None;
                let mut edit = false;
                let mut delete = false;
                let mut archive = false;
                let mut discard = false;
                let mut review = false;
                let mut upload=false;let mut download=None;let mut remove_attachment=None;
                let local_conflict = id.is_some_and(|id|self.core.active.is_some_and(|uid|self.core.manager.local_conflicts(&uid).contains(&id)));
                let local_edit = id.is_some_and(|id| self.core.active.is_some_and(|uid| self.core.manager.has_local_edit(&uid, id)));
                egui::ScrollArea::vertical()
                    .id_salt(("details", id))
                    .show(ui, |ui| {
                        ui.style_mut().interaction.selectable_labels = false;
                        ui.horizontal_wrapped(|ui| {
                            ui.weak("Vault:");
                            ui.label(&vault_name);
                        });
                        if detail.organization_id.is_some() || !collection_names.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.weak("Collections:");
                                ui.label(if collection_names.is_empty() { "None".to_owned() } else { collection_names.join(", ") });
                            });
                        }
                        ui.horizontal_wrapped(|ui| {
                            ui.weak("Folder:");
                            ui.label(&folder_name);
                        });
                        ui.horizontal_wrapped(|ui| {
                            edit = ui.add_enabled(detail.edit && detail.view_password && self.core.active.is_some_and(|uid|self.core.manager.policy_rules_for(&uid).allows_write(detail.r#type,detail.organization_id)), egui::Button::new(super::labels::text("Edit"))).clicked();
                            ui.add_enabled_ui(!self.core.remote(), |ui| {
                                delete = ui.button("Move to trash").clicked();
                            });
                            archive = ui.add_enabled(self.core.remote() && detail.deleted_date.is_none() && !local_edit && !self.core.active.is_some_and(|uid| self.core.sync.get(&uid).is_some_and(|s|s.busy) || self.core.manager.has_archive_intent(&uid)), egui::Button::new(if detail.archived_date.is_some() {"Unarchive"} else {"Archive"})).on_hover_text("Change this entry's archive state on the server").clicked();
                            ui.checkbox(&mut self.reveal, super::labels::text("Reveal secrets"));
                        });
                        if local_edit {
                            if local_conflict {
                                ui.strong(if id.is_some_and(|id| self.core.active.is_some_and(|uid| self.core.manager.is_local_creation(&uid,id))) { "New entry — server outcome needs review" } else { "Server version — a local draft needs review" });
                                review = ui.add_enabled(detail.edit && detail.view_password,egui::Button::new("Review local draft…")).clicked();
                            } else {
                                ui.strong("Local changes — awaiting synchronization");
                            }
                            ui.weak("Locking and restarting preserve these edits. Logging out removes local account data, including these changes.");
                            ui.collapsing("Discard local changes…", |ui| {
                                ui.label("Restore this entry from the last downloaded server snapshot? Your local edits will be lost.");
                                discard = ui.button("Confirm discard").clicked();
                            });
                        }
                        for (section, title, fields) in SECTIONS {
                            if data.get(*section).is_none_or(Value::is_null) {
                                continue;
                            }
                            ui.add_space(10.);
                            ui.strong(super::labels::text(title));
                            for (key, label, sensitive) in *fields {
                                if let Some(text) =
                                    data[*section][*key].as_str().filter(|s| !s.is_empty())
                                    && read_field(ui, label, text, *sensitive && !self.reveal)
                                {
                                    copy = Some(match (*section, *key) {
                                        ("login", "username") => CopyField::Username,
                                        ("login", "password") => CopyField::Password,
                                        ("sshKey", "privateKey") => CopyField::SshPrivate,
                                        ("sshKey", "publicKey") => CopyField::SshPublic,
                                        ("sshKey", "fingerprint") => CopyField::SshFingerprint,
                                        _ => CopyField::Json(format!("/{section}/{key}")),
                                    });
                                }
                            }
                        }
                        if let Some(uris) = data["login"]["uris"].as_array() {
                            for (i, uri) in uris.iter().enumerate() {
                                if let Some(text) = uri["uri"].as_str() {
                                    if read_field(ui, "Website", text, false) {
                                        copy = Some(CopyField::Url(i));
                                    }
                                    if safe_url(text) && ui.small_button("Open website").clicked() {
                                        let _ = open::that_detached(text);
                                    }
                                }
                            }
                        }
                        if data["login"]["totp"]
                            .as_str()
                            .is_some_and(|s| !s.is_empty())
                            && ui.button("Copy current verification code").clicked()
                        {
                            copy = Some(CopyField::Totp);
                        }
                        if let Some(notes) = data["notes"].as_str().filter(|s| !s.is_empty()) {
                            ui.separator();
                            if read_field(ui, "Notes", notes, false) {
                                copy = Some(CopyField::Json("/notes".into()));
                            }
                        }
                        if let Some(fields) = data["fields"].as_array() {
                            ui.separator();
                            ui.strong(super::labels::text("Custom fields"));
                            for (i, field) in fields.iter().enumerate() {
                                if let Some(text) = field["value"].as_str()
                                    && read_field(
                                        ui,
                                        field["name"].as_str().unwrap_or("Field"),
                                        text,
                                        field["type"] == 1 && !self.reveal,
                                    )
                                {
                                    copy = Some(CopyField::Custom(i));
                                }
                            }
                        }
                        ui.separator();ui.strong("Attachments");
                        let ready=self.core.attachment_busy.is_none() && !self.core.active.is_some_and(|uid|self.core.sync.get(&uid).is_some_and(|s|s.busy));
                        if let Some(attachments)=data["attachments"].as_array() {
                            for a in attachments {
                                let name=a["fileName"].as_str().unwrap_or("Attachment");
                                if let Some(aid)=a["id"].as_str() {
                                    ui.push_id(aid,|ui|ui.horizontal_wrapped(|ui| {
                                        ui.label(name);if let Some(size)=a["sizeName"].as_str(){ui.weak(size);}
                                        if ui.add_enabled(ready,egui::Button::new("Download…")).clicked() {download=Some((aid.to_owned(),name.to_owned()));}
                                        if detail.edit && ready {ui.menu_button("Delete…",|ui| {
                                            ui.label("Permanently delete this attachment from the server?");
                                            if ui.button("Confirm delete attachment").clicked(){remove_attachment=Some(aid.to_owned());ui.close();}
                                        });}
                                    }));
                                }
                            }
                        }
                        if self.core.remote() {upload=ui.add_enabled(ready&&detail.edit&&!local_edit,egui::Button::new("Attach file…")).clicked();}
                        if !ready {ui.weak("Attachment transfer or synchronization in progress…");}
                        if let Some(history) =
                            data["passwordHistory"].as_array().filter(|a| !a.is_empty())
                        {
                            ui.collapsing("Password history", |ui| {
                                for (i, entry) in history.iter().enumerate() {
                                    if let Some(text) = entry["password"].as_str()
                                        && read_field(
                                            ui,
                                            entry["lastUsedDate"]
                                                .as_str()
                                                .unwrap_or("Previous password"),
                                            text,
                                            !self.reveal,
                                        )
                                    {
                                        copy = Some(CopyField::Json(format!(
                                            "/passwordHistory/{i}/password"
                                        )));
                                    }
                                }
                            });
                        }
                        ui.separator();
                        ui.small(format!(
                            "Created: {}",
                            detail.creation_date.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M")
                        ));
                        ui.small(format!(
                            "Last changed: {}",
                            detail.revision_date.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M")
                        ));
                    });
                if let Some(id) = id {
                    if let Some(field) = copy {
                        self.core.request(Request::Copy(id, field));
                    }
                    if edit {
                        self.core.request(Request::Edit(id));
                    }
                    if delete {
                        self.core.request(Request::Delete(id));
                    }
                    if discard {
                        self.core.request(Request::DiscardLocal(id));
                    }
                    if review { self.core.request(Request::ReviewLocal(id)); }
                    if archive {self.core.request(Request::Archive{cipher:id,archived:data["archivedDate"].is_null()});}
                    if upload {self.core.pick_attachment(id,None);}
                    if let Some(download)=download {self.core.pick_attachment(id,Some(download));}
                    if let Some(attachment)=remove_attachment {self.core.request(Request::Attachment{cipher:id,operation:crate::services::sdk::attachments::Operation::Delete(attachment)});}

                }
            });
    }
    fn editor(&mut self, ui: &mut egui::Ui) {
        let local_only = self.core.remote();
        if local_only {
            ui.weak("Save synchronizes changes automatically. If synchronization fails, an encrypted local draft is retained.");
        }
        let folders = self
            .core
            .current_cache()
            .map(|c| {
                c.folders
                    .iter()
                    .map(|f| (f.id.to_string(), f.name.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let organizations = self
            .core
            .active
            .map(|uid| self.core.manager.list_organizations(&uid))
            .unwrap_or_default();
        let collections = self
            .core
            .current_cache()
            .map(|c| c.collections.clone())
            .unwrap_or_default();
        let mut save = false;
        let mut cancel = false;
        let mut generate = false;
        let uri_policy_default = self
            .core
            .active
            .and_then(|uid| self.core.manager.policy_rules_for(&uid).uri_default);
        if let Some(editor) = self.core.editor.as_mut() {
            ui.add_enabled_ui(!editor.busy, |ui| {
                ui.horizontal(|ui| {
                    save = ui.button(super::labels::text("Save")).clicked();
                    cancel = ui.button(super::labels::text("Cancel")).clicked();
                });
                egui::ScrollArea::vertical()
                    .id_salt("edit-fields")
                    .show(ui, |ui| {
                        edit_field(ui, &mut editor.data, "name", "Name", false);
                        let mut favorite = editor.data["favorite"].as_bool().unwrap_or(false);
                        if ui
                            .checkbox(&mut favorite, super::labels::text("Favorite"))
                            .changed()
                        {
                            editor.data["favorite"] = json!(favorite);
                        }
                        let mut reprompt = editor.data["reprompt"] == 1;
                        if ui
                            .checkbox(
                                &mut reprompt,
                                super::labels::text("Require master password"),
                            )
                            .changed()
                        {
                            editor.data["reprompt"] = json!(if reprompt { 1 } else { 0 });
                        }
                        ui.add_enabled_ui(!local_only || editor.source.is_none(), |ui| {
                            let mut org = editor.data["organizationId"].as_str().map(str::to_owned);
                            let previous_org = org.clone();
                            egui::ComboBox::from_id_salt("item-vault")
                                .selected_text(
                                    org.as_ref()
                                        .and_then(|id| {
                                            organizations.iter().find(|o| o.id.to_string() == *id)
                                        })
                                        .map(|o| o.name.as_str())
                                        .unwrap_or("My vault"),
                                )
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut org,
                                        None,
                                        super::labels::text("My vault"),
                                    );
                                    for o in &organizations {
                                        ui.selectable_value(
                                            &mut org,
                                            Some(o.id.to_string()),
                                            &o.name,
                                        );
                                    }
                                });
                            if org != previous_org {
                                editor.data["organizationId"] = json!(org);
                                editor.data["collectionIds"] = json!([]);
                            }
                            if let Some(org) = org {
                                ui.collapsing("Collections", |ui| {
                                    let mut ids = editor.data["collectionIds"]
                                        .as_array()
                                        .cloned()
                                        .unwrap_or_default();
                                    for collection in collections
                                        .iter()
                                        .filter(|c| c.organization_id.to_string() == org)
                                    {
                                        let value = json!(collection.id);
                                        let mut selected = ids.contains(&value);
                                        if ui.checkbox(&mut selected, &collection.name).changed() {
                                            if selected {
                                                ids.push(value);
                                            } else {
                                                ids.retain(|id| id != &value);
                                            }
                                        }
                                    }
                                    editor.data["collectionIds"] = json!(ids);
                                });
                            }
                        });
                        let mut folder = editor.data["folderId"].as_str().map(str::to_owned);
                        egui::ComboBox::from_id_salt("item-folder")
                            .selected_text(
                                folder
                                    .as_ref()
                                    .and_then(|id| folders.iter().find(|(f, _)| f == id))
                                    .map(|(_, s)| s.as_str())
                                    .unwrap_or("No folder"),
                            )
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut folder,
                                    None,
                                    super::labels::text("No folder"),
                                );
                                for (id, label) in &folders {
                                    ui.selectable_value(&mut folder, Some(id.clone()), label);
                                }
                            });
                        editor.data["folderId"] = json!(folder);
                        for (section, title, fields) in SECTIONS {
                            if editor.data.get(*section).is_none_or(Value::is_null) {
                                continue;
                            }
                            ui.separator();
                            ui.strong(super::labels::text(title));
                            for (key, label, sensitive) in *fields {
                                edit_field(ui, &mut editor.data[*section], key, label, *sensitive);
                            }
                            if *section == "sshKey" {
                                generate = ui.button("Generate Ed25519 key").clicked();
                            }
                        }
                        if !editor.data["login"].is_null() {
                            ui.separator();
                            ui.strong("Websites");
                            if editor.data["login"]["uris"].is_null() {
                                editor.data["login"]["uris"] = json!([]);
                            }
                            if let Some(uris) = editor.data["login"]["uris"].as_array_mut() {
                                let mut remove = None;
                                for (i, uri) in uris.iter_mut().enumerate() {
                                    ui.push_id(i, |ui| {
                                        edit_field(ui, uri, "uri", "URL", false);
                                        let labels = [
                                            "Base domain",
                                            "Host",
                                            "Starts with",
                                            "Exact",
                                            "Regular expression",
                                            "Never",
                                        ];
                                        let default = uri_policy_default
                                            .map(|v| {
                                                format!(
                                                    "Default: {} (organization)",
                                                    labels[v as usize]
                                                )
                                            })
                                            .unwrap_or_else(|| "Default".into());
                                        let mut matching =
                                            uri["match"].as_u64().filter(|v| *v <= 5);
                                        egui::ComboBox::from_id_salt("uri-matching")
                                            .selected_text(
                                                matching
                                                    .map(|v| labels[v as usize])
                                                    .unwrap_or(default.as_str()),
                                            )
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(
                                                    &mut matching,
                                                    None,
                                                    default.as_str(),
                                                );
                                                for (value, label) in labels.iter().enumerate() {
                                                    ui.selectable_value(
                                                        &mut matching,
                                                        Some(value as u64),
                                                        *label,
                                                    );
                                                }
                                            });
                                        uri["match"] =
                                            matching.map(Value::from).unwrap_or(Value::Null);
                                        if ui.small_button("Remove URL").clicked() {
                                            remove = Some(i);
                                        }
                                    });
                                }
                                if let Some(i) = remove {
                                    uris.remove(i);
                                }
                                if ui.button("Add URL").clicked() {
                                    uris.push(json!({"uri":"","match":null,"uriChecksum":null}));
                                }
                            }
                        }
                        ui.separator();
                        edit_field(ui, &mut editor.data, "notes", "Notes", false);
                        ui.separator();
                        ui.strong(super::labels::text("Custom fields"));
                        if editor.data["fields"].is_null() {
                            editor.data["fields"] = json!([]);
                        }
                        if let Some(fields) = editor.data["fields"].as_array_mut() {
                            let mut remove = None;
                            for (i, field) in fields.iter_mut().enumerate() {
                                ui.push_id(("custom", i), |ui| {
                                    edit_field(ui, field, "name", "Field name", false);
                                    let hidden = field["type"] == 1;
                                    edit_field(ui, field, "value", "Value", hidden);
                                    let mut ty = field["type"].as_u64().unwrap_or(0);
                                    egui::ComboBox::from_id_salt("field-type")
                                        .selected_text(if ty == 1 {
                                            "Hidden"
                                        } else if ty == 2 {
                                            "Boolean"
                                        } else if ty == 3 {
                                            "Linked"
                                        } else {
                                            "Text"
                                        })
                                        .show_ui(ui, |ui| {
                                            for (value, label) in
                                                [(0, "Text"), (1, "Hidden"), (2, "Boolean")]
                                            {
                                                ui.selectable_value(&mut ty, value, label);
                                            }
                                        });
                                    field["type"] = json!(ty);
                                    if ui.small_button("Remove field").clicked() {
                                        remove = Some(i);
                                    }
                                });
                            }
                            if let Some(i) = remove {
                                fields.remove(i);
                            }
                            if ui.button("Add field").clicked() {
                                fields.push(json!({"name":"","value":"","type":0,"linkedId":null}));
                            }
                        }
                    });
            });
        }
        if save {
            self.core.save_editor();
        }
        if cancel {
            self.core.editor = None;
            if self.core.detail.is_none() {
                self.core.close_details();
            }
        }
        if generate {
            self.core.generate_ssh_key();
        }
    }
    pub(super) fn copy_widget_text(
        &mut self,
        text: String,
        source: Option<bitwarden_vault::CipherId>,
    ) {
        if let Some(id) = source {
            self.core
                .request(Request::CopyText(id, Zeroizing::new(text)));
            return;
        }
        self.core
            .clipboard
            .copy(text, services::clipboard::Sensitivity::Sensitive);
    }
}
fn read_field(ui: &mut egui::Ui, label: &str, text: &str, hidden: bool) -> bool {
    ui.add_space(5.);
    ui.weak(super::labels::text(label));
    let mut copy = false;
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::Label::new(if hidden {
                "••••••••"
            } else {
                text
            })
            .wrap()
            .selectable(false),
        );
        copy = ui.small_button(super::labels::text("Copy")).clicked();
    });
    copy
}
pub(super) fn edit_field(
    ui: &mut egui::Ui,
    data: &mut Value,
    key: &str,
    label: &str,
    sensitive: bool,
) {
    ui.push_id(key, |ui| {
        ui.label(super::labels::text(label));
        let mut text = data[key].as_str().unwrap_or("").to_owned();
        let response = if sensitive {
            secret(ui, &mut text, "value", false)
        } else if key == "notes" {
            ui.add(
                egui::TextEdit::multiline(&mut text)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY),
            )
        } else {
            ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY))
        };
        if response.changed() {
            data[key] = if text.is_empty()
                && !matches!(key, "name" | "privateKey" | "publicKey" | "fingerprint")
            {
                Value::Null
            } else {
                json!(text)
            };
        }
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
            state.clear_undoer();
            state.store(ui.ctx(), response.id);
        }
    });
}
fn safe_url(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|u| {
        matches!(u.scheme(), "http" | "https")
            && u.host_str().is_some()
            && u.username().is_empty()
            && u.password().is_none()
    })
}
