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
        let mut changed = false;
        let open =
            self.native_tool_window(ctx, "settings", "Settings", [640., 760.], |this, ui| {
                let app = this;
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        ui.heading(super::labels::text("Appearance"));
                        let previous_language = app.core.settings.language.clone();
                        egui::ComboBox::from_id_salt("language")
                            .selected_text(if app.core.settings.language.is_empty() {
                                "System language".into()
                            } else {
                                services::i18n::language_label(&app.core.settings.language)
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut app.core.settings.language,
                                    String::new(),
                                    "System language",
                                );
                                for language in services::i18n::available_languages() {
                                    let tag = language.to_string();
                                    ui.selectable_value(
                                        &mut app.core.settings.language,
                                        tag.clone(),
                                        services::i18n::language_label(&tag),
                                    );
                                }
                            });
                        if previous_language != app.core.settings.language {
                            if app.core.settings.language.is_empty() {
                                services::i18n::init();
                            } else if let Ok(tag) = app.core.settings.language.parse() {
                                services::i18n::set_language(tag);
                            }
                            changed = true;
                        }
                        #[cfg(any(target_os = "windows", target_os = "macos"))]
                        {
                            changed |= ui
                                .checkbox(
                                    &mut app.core.settings.allow_screenshots,
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
                                &app.core.settings.theme.to_string(),
                            ))
                            .show_ui(ui, |ui| {
                                for theme in crate::theme::ThemePreference::ALL {
                                    changed |= ui
                                        .selectable_value(
                                            &mut app.core.settings.theme,
                                            theme,
                                            super::labels::text(&theme.to_string()),
                                        )
                                        .changed();
                                }
                            });
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "Zoom: {:.0}%",
                                app.core.settings.zoom_factor.scale() * 100.
                            ));
                            if ui.button("−").clicked() {
                                changed |= app.core.settings.zoom_factor.step_out();
                            }
                            if ui.button("+").clicked() {
                                changed |= app.core.settings.zoom_factor.step_in();
                            }
                            if ui.button("Reset").clicked() {
                                changed |= app.core.settings.zoom_factor.reset();
                            }
                        });
                        changed |= ui
                            .checkbox(
                                &mut app.core.settings.show_favicons,
                                super::labels::text("Show website icons"),
                            )
                            .changed();
                        ui.separator();
                        ui.heading("Desktop");
                        changed |= ui
                            .checkbox(
                                &mut app.core.settings.show_tray_icon,
                                super::labels::text("Show tray icon"),
                            )
                            .changed();
                        changed |= ui
                            .checkbox(
                                &mut app.core.settings.minimize_to_tray,
                                super::labels::text("Minimize to tray"),
                            )
                            .changed();
                        changed |= ui
                            .checkbox(
                                &mut app.core.settings.close_to_tray,
                                super::labels::text("Close to tray"),
                            )
                            .changed();
                        ui.add_enabled(
                            false,
                            egui::Checkbox::new(
                                &mut app.core.settings.open_at_login,
                                "Open at device login (not implemented)",
                            ),
                        );
                        ui.separator();
                        ui.heading(super::labels::text("Security"));
                        if let Some(uid) = app.core.active {
                            let mut prefs = app.core.settings.preferences_for(&uid);
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
                                app.core.settings.user_preferences.insert(uid, prefs);
                                changed = true;
                            }
                        }
                        ui.separator();
                        ui.heading(super::labels::text("SSH agent"));
                        changed |= ui
                            .checkbox(
                                &mut app.core.settings.ssh_agent,
                                super::labels::text("Enable SSH agent"),
                            )
                            .changed();
                        ui.label(app.core.ssh_agent.status());
                        ui.monospace(app.core.ssh_agent.path().display().to_string());
                        ui.horizontal(|ui| {
                            ui.monospace(app.core.ssh_agent.shell_setup());
                            if ui.button("Copy setup").clicked() {
                                app.core.clipboard.copy(
                                    app.core.ssh_agent.shell_setup(),
                                    services::clipboard::Sensitivity::Normal,
                                );
                            }
                        });
                        if app.core.remote()
                            && ui
                                .button("Test account connection (without sync)")
                                .clicked()
                        {
                            app.core.probe_account();
                        }
                        if ui.button("Server / mTLS settings").clicked() {
                            app.profile_open = true;
                            app.core.ensure_profiles();
                            if let Some(id) = app
                                .core
                                .active
                                .and_then(|uid| app.core.manager.profile_for(&uid))
                            {
                                app.core.profile =
                                    app.core.profiles.as_ref().and_then(|s| s.get(id));
                            }
                        }
                    });
            });
        self.core.settings_open = open && !self.profile_open;
        if changed {
            self.core.save_settings();
            ctx.request_repaint();
        }
    }
    fn generator(&mut self, ctx: &egui::Context) {
        let open = self.native_tool_window(ctx, "generator", "Generator", [600., 540.], |this, ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let app = this;
                ui.horizontal(|ui| {
                    for (i, label) in ["Password", "Passphrase", "Username", "History"].iter().enumerate() {
                        let selected = app.generator_mode == i;
                        let response = ui.add_sized([120., 34.], egui::Button::new(super::labels::text(label)).selected(selected));
                        if response.clicked() { app.generator_mode = i; }
                        if selected {
                            ui.painter().line_segment([response.rect.left_bottom(), response.rect.right_bottom()], egui::Stroke::new(3., ui.visuals().selection.bg_fill));
                        }
                    }
                });
                ui.separator();
                if app.generator_mode == 3 {
                    app.generator_history(ui);
                    return;
                }
                if app.generator_mode == 0 {
                    ui.add(egui::Slider::new(&mut app.generator_length, 5..=128).text("Length"));
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut app.generator_lower, super::labels::text("a–z"));
                        ui.checkbox(&mut app.generator_upper, super::labels::text("A–Z"));
                        ui.checkbox(&mut app.generator_numbers, super::labels::text("0–9"));
                        ui.checkbox(&mut app.generator_special, super::labels::text("!@#$"));
                    });
                    ui.checkbox(&mut app.generator_avoid_ambiguous,super::labels::text("Avoid ambiguous characters"));
                    ui.add(egui::Slider::new(&mut app.generator_min_numbers,0..=9).text("Minimum numbers"));
                    ui.add(egui::Slider::new(&mut app.generator_min_special,0..=9).text("Minimum special characters"));
                } else if app.generator_mode == 1 {
                    ui.add(egui::Slider::new(&mut app.generator_words, 3..=20).text("Words"));
                    ui.label("Separator");
                    ui.text_edit_singleline(&mut app.generator_separator);
                    ui.checkbox(&mut app.generator_upper, super::labels::text("Capitalize"));
                    ui.checkbox(&mut app.generator_numbers, super::labels::text("Include number"));
                } else {
                    egui::ComboBox::from_id_salt("username-kind").selected_text(["Word","Subaddress","Catch-all"][app.username_mode]).show_ui(ui,|ui|{for(i,label)in["Word","Subaddress","Catch-all"].iter().enumerate(){ui.selectable_value(&mut app.username_mode,i,*label);}});
                    if app.username_mode==1 {ui.label("Email");ui.text_edit_singleline(&mut app.username_email);}
                    if app.username_mode==2 {ui.label("Domain");ui.text_edit_singleline(&mut app.username_domain);}
                    ui.checkbox(&mut app.generator_upper, super::labels::text("Capitalize"));
                    ui.checkbox(&mut app.generator_numbers, super::labels::text("Include number"));
                }
                if ui
                    .add_enabled(!app.core.generator.busy, egui::Button::new(super::labels::text("Generate")))
                    .clicked()
                {
                    let request = match app.generator_mode {
                        1 => Generate::Passphrase(PassphraseGeneratorRequest {
                            num_words: app.generator_words.clamp(3, 20),
                            word_separator: app.generator_separator.clone(),
                            capitalize: app.generator_upper,
                            include_number: app.generator_numbers,
                        }),
                        2 => Generate::Username(match app.username_mode {
                            1=>serde_json::from_value(serde_json::json!({"subaddress":{"type":"random","email":app.username_email}})).expect("SDK subaddress schema"),
                            2=>serde_json::from_value(serde_json::json!({"catchall":{"type":"random","domain":app.username_domain}})).expect("SDK catch-all schema"),
                            _=>UsernameGeneratorRequest::Word{capitalize:app.generator_upper,include_number:app.generator_numbers}
                        }),
                        _ => Generate::Password(PasswordGeneratorRequest {
                            length: app.generator_length.clamp(5, 128),
                            lowercase: app.generator_lower,
                            uppercase: app.generator_upper,
                            numbers: app.generator_numbers,
                            special: app.generator_special,
                            avoid_ambiguous:app.generator_avoid_ambiguous,
                            min_number:app.generator_numbers.then_some(app.generator_min_numbers),
                            min_special:app.generator_special.then_some(app.generator_min_special),
                            ..Default::default()
                        }),
                    };
                    app.core.generate(request);
                }
                if app.core.generator.busy {
                    ui.spinner();
                }
                ui.separator();
                ui.label(
                    egui::RichText::new(app.core.generator.output.as_str())
                        .monospace()
                        .size(19.),
                );
                if ui.button(super::labels::text("Copy")).clicked() {
                    app.core.clipboard.copy(
                        app.core.generator.output.to_string(),
                        services::clipboard::Sensitivity::Sensitive,
                    );
                }
            });
        });
        if !open {
            self.core.generator.clear();
        }
        self.core.generator_open = open;
    }
    fn generator_history(&mut self, ui: &mut egui::Ui) {
        let Some(uid) = self.core.active else {
            return;
        };
        if ui.button(super::labels::text("Clear history")).clicked() {
            self.core.manager.clear_password_history(&uid);
        }
        let entries = self.core.manager.password_history(&uid);
        if entries.is_empty() {
            ui.weak(crate::fl!("generator-history-empty"));
        }
        for entry in entries {
            ui.horizontal(|ui| {
                ui.label(
                    entry
                        .created
                        .with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M")
                        .to_string(),
                );
                if ui.small_button(super::labels::text("Copy")).clicked() {
                    self.core.clipboard.copy(
                        entry.value.clone(),
                        services::clipboard::Sensitivity::Sensitive,
                    );
                }
                ui.add(egui::Label::new(egui::RichText::new(&entry.value).monospace()).wrap());
            });
            ui.separator();
        }
    }
    fn native_tool_window(
        &mut self,
        ctx: &egui::Context,
        id: &'static str,
        title: &str,
        size: [f32; 2],
        mut content: impl FnMut(&mut Self, &mut egui::Ui),
    ) -> bool {
        let mut open = true;
        let scope = self.core.scope();
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of(("tool-window", id)),
            egui::ViewportBuilder::default()
                .with_title(format!("{} – Bitwarden", super::labels::text(title)))
                .with_app_id(crate::assets::DESKTOP_APP_ID)
                .with_icon(crate::assets::window_icon().unwrap_or_default())
                .with_inner_size(size)
                .with_min_inner_size([560., 350.]),
            |ui, class| {
                let child = ui.ctx().clone();
                ui.style_mut().interaction.selectable_labels = false;
                ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
                ui.style_mut().spacing.scroll.bar_width = 12.;
                ui.style_mut().spacing.scroll.foreground_color = true;
                if !child.input(|i| i.events.is_empty()) {
                    self.core.record_activity();
                }
                child.send_viewport_cmd(egui::ViewportCommand::ContentProtected(
                    !self.core.settings.allow_screenshots,
                ));
                if child.input(|i| i.viewport().close_requested())
                    || child.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
                {
                    open = false;
                } else if class == egui::ViewportClass::EmbeddedWindow {
                    content(self, ui);
                } else {
                    egui::CentralPanel::default().show(ui, |ui| content(self, ui));
                }
                self.drain_clipboard(&child, scope.clone(), None);
            },
        );
        if !open {
            ctx.send_viewport_cmd_to(
                egui::ViewportId::from_hash_of(("tool-window", id)),
                egui::ViewportCommand::Close,
            );
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
        open
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
