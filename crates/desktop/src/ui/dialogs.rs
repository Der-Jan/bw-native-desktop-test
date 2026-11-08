use super::*;
use crate::{
    core::generator::Generate,
    services::{menu::MenuAction, preferences::*},
};
use bitwarden_generators::{
    PassphraseGeneratorRequest, PasswordGeneratorRequest, UsernameGeneratorRequest,
};
impl Application {
    pub(super) fn any_modal(&self) -> bool {
        self.core.prompt.is_some()
            || self.core.settings_open
            || self.profile_open
            || self.core.generator_open
            || self.core.export_open
            || self.core.import_open
            || self.core.about_open
            || self.core.fingerprint_open
            || self.core.new_folder_open
    }
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        self.profile_dialog(ctx);
        if self.core.settings_open {
            self.settings(ctx);
        }
        if self.core.generator_open {
            self.generator(ctx);
        }
        if self.core.about_open {
            let mut open = true;
            egui::Window::new(super::labels::text("About"))
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.heading("Bitwarden Password Manager");
                    ui.label(format!(
                        "Native desktop {} · egui",
                        env!("CARGO_PKG_VERSION")
                    ));
                    ui.label(&self.font_description);
                    ui.hyperlink_to(
                        "Source code",
                        "https://github.com/Der-Jan/bw-native-desktop-test",
                    );
                });
            self.core.about_open = open;
        }
        if self.core.fingerprint_open {
            let mut open = true;
            egui::Window::new(super::labels::text("Fingerprint phrase"))
                .open(&mut open)
                .show(ctx, |ui| {
                    if let Some(phrase) = self
                        .core
                        .active
                        .and_then(|uid| self.core.manager.user_fingerprint(&uid))
                    {
                        ui.label(phrase);
                    }
                });
            self.core.fingerprint_open = open;
        }
        if self.core.new_folder_open {
            let mut open = true;
            let mut create = false;
            egui::Window::new(super::labels::text("New folder"))
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("Folder name (use / for nested folders)");
                    let response = ui.text_edit_singleline(&mut self.folder_name);
                    create = ui.button("Create folder").clicked()
                        || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
                });
            self.core.new_folder_open = open;
            if create {
                self.core
                    .create_folder(std::mem::take(&mut self.folder_name));
            }
        }
        if self.core.export_open {
            self.export(ctx);
        }
        if self.core.import_open {
            let mut open = true;
            egui::Window::new(super::labels::text("Import vault")).open(&mut open).show(ctx,|ui|{ui.label("Import is not implemented in the current backend. Use your server’s web vault to import, then synchronize.");if ui.button("Open web vault").clicked(){self.core.menu_action(MenuAction::OpenWebVault(Some("#/tools/import")));}});
            self.core.import_open = open;
        }
        if let Some(prompt) = self.core.prompt.as_ref() {
            let id = prompt.id;
            let busy = prompt.busy;
            let error = prompt.error;
            let focus = self.prompt_seen != Some(id);
            self.prompt_seen = Some(id);
            let response =
                egui::Modal::new(Id::unique("master-password-confirmation")).show(ctx, |ui| {
                    ui.set_width(350.);
                    ui.heading("Confirm master password");
                    ui.label("This action requires your master password.");
                    let input = secret(ui, &mut self.password, ("prompt", id), false);
                    if focus {
                        input.request_focus();
                    }
                    if error {
                        ui.colored_label(
                            egui::Color32::RED,
                            "Incorrect master password. Try again.",
                        );
                    }
                    let mut confirm = false;
                    let mut cancel = false;
                    ui.add_enabled_ui(!busy, |ui| {
                        ui.horizontal(|ui| {
                            confirm = ui.button("Confirm").clicked()
                                || (input.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
                            cancel = ui.button(super::labels::text("Cancel")).clicked();
                        });
                    });
                    if busy {
                        ui.spinner();
                    }
                    (confirm, cancel)
                });
            if response.inner.0 {
                self.core
                    .confirm_password(std::mem::take(&mut self.password));
            }
            if response.inner.1 || response.should_close() {
                self.core.cancel_prompt();
                self.password = Zeroizing::default();
                self.prompt_seen = None;
            }
        } else {
            self.prompt_seen = None;
        }
        if self.any_modal() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
        {
            if self.core.prompt.is_some() {
                self.core.cancel_prompt();
                self.password = Zeroizing::default();
            } else if self.profile_open {
                self.profile_open = false;
            } else if self.core.settings_open {
                self.core.settings_open = false;
            } else if self.core.generator_open {
                self.core.generator_open = false;
                self.core.generator.clear();
            } else {
                self.core.about_open = false;
                self.core.export_open = false;
                self.core.export_request = None;
                self.core.import_open = false;
                self.core.fingerprint_open = false;
                self.core.new_folder_open = false;
            }
        }
    }
    fn settings(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let mut changed = false;
        egui::Window::new(super::labels::text("Settings"))
            .id(Id::unique("settings"))
            .open(&mut open)
            .default_width(550.)
            .collapsible(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(600.)
                    .show(ui, |ui| {
                        ui.heading(super::labels::text("Appearance"));
                        let previous_language = self.core.settings.language.clone();
                        egui::ComboBox::from_id_salt("language")
                            .selected_text(if self.core.settings.language.is_empty() {
                                "System language".into()
                            } else {
                                services::i18n::language_label(&self.core.settings.language)
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.core.settings.language,
                                    String::new(),
                                    "System language",
                                );
                                for language in services::i18n::available_languages() {
                                    let tag = language.to_string();
                                    ui.selectable_value(
                                        &mut self.core.settings.language,
                                        tag.clone(),
                                        services::i18n::language_label(&tag),
                                    );
                                }
                            });
                        if previous_language != self.core.settings.language {
                            if self.core.settings.language.is_empty() {
                                services::i18n::init();
                            } else if let Ok(tag) = self.core.settings.language.parse() {
                                services::i18n::set_language(tag);
                            }
                            changed = true;
                        }
                        #[cfg(any(target_os = "windows", target_os = "macos"))]
                        {
                            changed |= ui
                                .checkbox(
                                    &mut self.core.settings.allow_screenshots,
                                    "Allow screenshots",
                                )
                                .changed();
                        }
                        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
                        {
                            ui.weak("Screen capture exclusion is unavailable on Linux.");
                        }

                        ui.label(crate::fl!("settings-appearance-theme-heading"));
                        egui::ComboBox::from_id_salt("theme")
                            .selected_text(super::labels::text(
                                &self.core.settings.theme.to_string(),
                            ))
                            .show_ui(ui, |ui| {
                                for theme in crate::theme::ThemePreference::ALL {
                                    changed |= ui
                                        .selectable_value(
                                            &mut self.core.settings.theme,
                                            theme,
                                            super::labels::text(&theme.to_string()),
                                        )
                                        .changed();
                                }
                            });
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "Zoom: {:.0}%",
                                self.core.settings.zoom_factor.scale() * 100.
                            ));
                            if ui.button("−").clicked() {
                                changed |= self.core.settings.zoom_factor.step_out();
                            }
                            if ui.button("+").clicked() {
                                changed |= self.core.settings.zoom_factor.step_in();
                            }
                            if ui.button("Reset").clicked() {
                                changed |= self.core.settings.zoom_factor.reset();
                            }
                        });
                        changed |= ui
                            .checkbox(
                                &mut self.core.settings.show_favicons,
                                super::labels::text("Show website icons"),
                            )
                            .changed();
                        ui.separator();
                        ui.heading("Desktop");
                        changed |= ui
                            .checkbox(
                                &mut self.core.settings.show_tray_icon,
                                super::labels::text("Show tray icon"),
                            )
                            .changed();
                        changed |= ui
                            .checkbox(
                                &mut self.core.settings.minimize_to_tray,
                                super::labels::text("Minimize to tray"),
                            )
                            .changed();
                        changed |= ui
                            .checkbox(
                                &mut self.core.settings.close_to_tray,
                                super::labels::text("Close to tray"),
                            )
                            .changed();
                        ui.add_enabled(
                            false,
                            egui::Checkbox::new(
                                &mut self.core.settings.open_at_login,
                                "Open at device login (not implemented)",
                            ),
                        );
                        ui.separator();
                        ui.heading(super::labels::text("Security"));
                        if let Some(uid) = self.core.active {
                            let mut prefs = self.core.settings.preferences_for(&uid);
                            let previous = prefs;
                            duration(ui, "Lock after", &mut prefs.lock_after, LOCK_AFTER_PRESETS);
                            duration(
                                ui,
                                "Log out after",
                                &mut prefs.logout_after,
                                LOGOUT_AFTER_PRESETS,
                            );
                            duration(
                                ui,
                                "Clear clipboard after",
                                &mut prefs.clear_clipboard,
                                CLEAR_CLIPBOARD_PRESETS,
                            );
                            ui.checkbox(
                                &mut prefs.lock_on_system_lock,
                                "Lock when system is locked",
                            );
                            ui.checkbox(
                                &mut prefs.minimize_on_copy,
                                super::labels::text("Hide to tray after copy"),
                            );
                            ui.add_enabled(
                                false,
                                egui::Checkbox::new(
                                    &mut prefs.pin_unlock,
                                    "Unlock with PIN (not implemented)",
                                ),
                            );
                            ui.add_enabled(
                                false,
                                egui::Checkbox::new(
                                    &mut prefs.touch_id_unlock,
                                    "Biometric unlock (concept only)",
                                ),
                            );
                            egui::ComboBox::from_id_salt("ssh-prompt")
                                .selected_text(prefs.ssh_prompt_behavior.label())
                                .show_ui(ui, |ui| {
                                    for mode in SshPromptBehavior::ALL {
                                        ui.selectable_value(
                                            &mut prefs.ssh_prompt_behavior,
                                            *mode,
                                            mode.label(),
                                        );
                                    }
                                });
                            if prefs != previous {
                                self.core.settings.user_preferences.insert(uid, prefs);
                                changed = true;
                            }
                        }
                        ui.separator();
                        ui.heading(super::labels::text("SSH agent"));
                        changed |= ui
                            .checkbox(
                                &mut self.core.settings.ssh_agent,
                                super::labels::text("Enable SSH agent"),
                            )
                            .changed();
                        ui.label(self.core.ssh_agent.status());
                        ui.monospace(self.core.ssh_agent.path().display().to_string());
                        ui.horizontal(|ui| {
                            ui.monospace(self.core.ssh_agent.shell_setup());
                            if ui.button("Copy setup").clicked() {
                                self.core.clipboard.copy(
                                    self.core.ssh_agent.shell_setup(),
                                    services::clipboard::Sensitivity::Normal,
                                );
                            }
                        });
                        if self.core.remote()
                            && ui
                                .button("Test account connection (without sync)")
                                .clicked()
                        {
                            self.core.probe_account();
                        }
                        if ui.button("Server / mTLS settings").clicked() {
                            self.profile_open = true;
                            self.core.ensure_profiles();
                            if let Some(id) = self
                                .core
                                .active
                                .and_then(|uid| self.core.manager.profile_for(&uid))
                            {
                                self.core.profile =
                                    self.core.profiles.as_ref().and_then(|s| s.get(id));
                            }
                        }
                    });
            });
        self.core.settings_open = open;
        if changed {
            self.core.save_settings();
        }
    }
    fn generator(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new(super::labels::text("Generator"))
            .id(Id::unique("generator"))
            .open(&mut open)
            .default_width(500.)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (i, label) in ["Password", "Passphrase", "Username"].iter().enumerate() {
                        ui.selectable_value(&mut self.generator_mode, i, *label);
                    }
                });
                if self.generator_mode == 0 {
                    ui.add(egui::Slider::new(&mut self.generator_length, 5..=128).text("Length"));
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.generator_lower, super::labels::text("a–z"));
                        ui.checkbox(&mut self.generator_upper, super::labels::text("A–Z"));
                        ui.checkbox(&mut self.generator_numbers, super::labels::text("0–9"));
                        ui.checkbox(&mut self.generator_special, super::labels::text("!@#$"));
                    });
                    ui.checkbox(&mut self.generator_avoid_ambiguous,super::labels::text("Avoid ambiguous characters"));
                    ui.add(egui::Slider::new(&mut self.generator_min_numbers,0..=9).text("Minimum numbers"));
                    ui.add(egui::Slider::new(&mut self.generator_min_special,0..=9).text("Minimum special characters"));
                } else if self.generator_mode == 1 {
                    ui.add(egui::Slider::new(&mut self.generator_words, 3..=20).text("Words"));
                    ui.label("Separator");
                    ui.text_edit_singleline(&mut self.generator_separator);
                    ui.checkbox(&mut self.generator_upper, super::labels::text("Capitalize"));
                    ui.checkbox(&mut self.generator_numbers, super::labels::text("Include number"));
                } else {
                    egui::ComboBox::from_id_salt("username-kind").selected_text(["Word","Subaddress","Catch-all"][self.username_mode]).show_ui(ui,|ui|{for(i,label)in["Word","Subaddress","Catch-all"].iter().enumerate(){ui.selectable_value(&mut self.username_mode,i,*label);}});
                    if self.username_mode==1 {ui.label("Email");ui.text_edit_singleline(&mut self.username_email);}
                    if self.username_mode==2 {ui.label("Domain");ui.text_edit_singleline(&mut self.username_domain);}
                    ui.checkbox(&mut self.generator_upper, super::labels::text("Capitalize"));
                    ui.checkbox(&mut self.generator_numbers, super::labels::text("Include number"));
                }
                if ui
                    .add_enabled(!self.core.generator.busy, egui::Button::new(super::labels::text("Generate")))
                    .clicked()
                {
                    let request = match self.generator_mode {
                        1 => Generate::Passphrase(PassphraseGeneratorRequest {
                            num_words: self.generator_words.clamp(3, 20),
                            word_separator: self.generator_separator.clone(),
                            capitalize: self.generator_upper,
                            include_number: self.generator_numbers,
                        }),
                        2 => Generate::Username(match self.username_mode {
                            1=>serde_json::from_value(serde_json::json!({"subaddress":{"type":"random","email":self.username_email}})).expect("SDK subaddress schema"),
                            2=>serde_json::from_value(serde_json::json!({"catchall":{"type":"random","domain":self.username_domain}})).expect("SDK catch-all schema"),
                            _=>UsernameGeneratorRequest::Word{capitalize:self.generator_upper,include_number:self.generator_numbers}
                        }),
                        _ => Generate::Password(PasswordGeneratorRequest {
                            length: self.generator_length.clamp(5, 128),
                            lowercase: self.generator_lower,
                            uppercase: self.generator_upper,
                            numbers: self.generator_numbers,
                            special: self.generator_special,
                            avoid_ambiguous:self.generator_avoid_ambiguous,
                            min_number:self.generator_numbers.then_some(self.generator_min_numbers),
                            min_special:self.generator_special.then_some(self.generator_min_special),
                            ..Default::default()
                        }),
                    };
                    self.core.generate(request);
                }
                if self.core.generator.busy {
                    ui.spinner();
                }
                ui.separator();
                ui.label(
                    egui::RichText::new(self.core.generator.output.as_str())
                        .monospace()
                        .size(19.),
                );
                if ui.button(super::labels::text("Copy")).clicked() {
                    self.core.clipboard.copy(
                        self.core.generator.output.to_string(),
                        services::clipboard::Sensitivity::Sensitive,
                    );
                }
                ui.collapsing("History", |ui| {
                    if let Some(uid) = self.core.active {
                        if ui.button(super::labels::text("Clear history")).clicked() {
                            self.core.manager.clear_password_history(&uid);
                        }
                        egui::ScrollArea::vertical()
                            .max_height(220.)
                            .show(ui, |ui| {
                                for entry in self.core.manager.password_history(&uid) {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            entry
                                                .created
                                                .with_timezone(&chrono::Local)
                                                .format("%H:%M")
                                                .to_string(),
                                        );
                                        ui.label(&entry.value);
                                        if ui.small_button(super::labels::text("Copy")).clicked() {
                                            self.core.clipboard.copy(
                                                entry.value,
                                                services::clipboard::Sensitivity::Sensitive,
                                            );
                                        }
                                    });
                                }
                            });
                    }
                });
            });
        if !open {
            self.core.generator.clear();
        }
        self.core.generator_open = open;
    }
    fn export(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new(super::labels::text("Export vault"))
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                let organizations = self
                    .core
                    .active
                    .map(|uid| self.core.manager.list_organizations(&uid))
                    .unwrap_or_default();
                egui::ComboBox::from_id_salt("export-vault")
                    .selected_text(
                        self.export_organization
                            .and_then(|id| organizations.iter().find(|o| o.id == id))
                            .map(|o| o.name.as_str())
                            .unwrap_or("My vault"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.export_organization,
                            None,
                            super::labels::text("My vault"),
                        );
                        for org in &organizations {
                            ui.selectable_value(
                                &mut self.export_organization,
                                Some(org.id),
                                &org.name,
                            );
                        }
                    });
                egui::ComboBox::from_id_salt("export-format")
                    .selected_text(["JSON", "CSV", "Encrypted JSON"][self.export_format])
                    .show_ui(ui, |ui| {
                        for (i, label) in ["JSON", "CSV", "Encrypted JSON"].iter().enumerate() {
                            ui.selectable_value(&mut self.export_format, i, *label);
                        }
                    });
                if self.export_format == 2 {
                    ui.label("Export password");
                    secret(ui, &mut self.export_password, "export-password", false);
                    ui.label("Confirm export password");
                    secret(ui, &mut self.export_confirm, "export-confirm", false);
                } else {
                    ui.weak("JSON and CSV exports are not encrypted.");
                }
                let valid = self.export_format != 2
                    || (!self.export_password.is_empty()
                        && self.export_password == self.export_confirm);
                if ui
                    .add_enabled(
                        valid,
                        egui::Button::new(super::labels::text("Choose destination…")),
                    )
                    .clicked()
                {
                    use bitwarden_exporters::ExportFormat;
                    let format = match self.export_format {
                        1 => ExportFormat::Csv,
                        2 => ExportFormat::EncryptedJson {
                            password: std::mem::take(&mut self.export_password).to_string(),
                        },
                        _ => ExportFormat::Json,
                    };
                    self.export_confirm = Zeroizing::default();
                    self.core.pick_export_path(self.export_organization, format);
                }
            });
        self.core.export_open = open;
        if !open {
            self.core.export_request = None;
            self.export_password = Zeroizing::default();
            self.export_confirm = Zeroizing::default();
        }
    }
}
fn duration(ui: &mut egui::Ui, label: &str, value: &mut DurationSecs, presets: &[DurationSecs]) {
    ui.horizontal(|ui| {
        ui.label(super::labels::text(label));
        egui::ComboBox::from_id_salt(label)
            .selected_text(value.label())
            .show_ui(ui, |ui| {
                for preset in presets {
                    ui.selectable_value(value, *preset, preset.label());
                }
            });
    });
}
