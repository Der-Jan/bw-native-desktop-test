use super::*;
use crate::services::accounts::login::Factor;
impl Application {
    pub(super) fn login(&mut self, ui: &mut egui::Ui) {
        if self.core.loading {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }
        ui.vertical_centered(|ui| {
            ui.add_space(40.);
            ui.add(
                egui::Image::from_bytes(
                    "bytes://wordmark.svg",
                    crate::assets::PASSWORD_MANAGER_LOGO,
                )
                .tint(if ui.visuals().dark_mode {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::from_rgb(22, 92, 225)
                })
                .fit_to_exact_size(egui::vec2(240., 59.)),
            );
            ui.add_space(24.);
            let accounts = self.core.manager.accounts();
            if !accounts.is_empty() {
                egui::ComboBox::from_id_salt("login-account")
                    .selected_text(
                        accounts
                            .iter()
                            .find(|a| Some(a.user_id) == self.core.active)
                            .map(|a| a.email.as_str())
                            .unwrap_or("Add account"),
                    )
                    .show_ui(ui, |ui| {
                        for account in accounts {
                            if ui
                                .selectable_label(
                                    Some(account.user_id) == self.core.active,
                                    &account.email,
                                )
                                .clicked()
                            {
                                self.core.switch_user(account.user_id);
                            }
                        }
                        if ui.button(super::labels::text("Add account")).clicked() {
                            self.core.add_account();
                        }
                    });
            }
            ui.add_space(15.);
            ui.allocate_ui_with_layout(
                egui::vec2(390., 320.),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let unlocking = self.core.active.is_some();
                    let title = egui::RichText::new(if unlocking {
                        "Unlock vault"
                    } else {
                        "Log in to Bitwarden"
                    });
                    ui.heading(if unlocking { title.size(18.) } else { title });
                    ui.add_space(12.);
                    if let Some(account) = self
                        .core
                        .manager
                        .accounts()
                        .into_iter()
                        .find(|a| Some(a.user_id) == self.core.active)
                    {
                        ui.label(account.email);
                        ui.weak(account.server_url);
                    } else {
                        ui.label("Email address");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.core.login.email)
                                .desired_width(f32::INFINITY),
                        );
                    }
                    let challenge = self.core.login.pending.is_some();
                    let mut submit = false;
                    ui.add_enabled_ui(!self.core.login.busy, |ui| {
                        if challenge {
                            egui::ComboBox::from_id_salt("factor")
                                .selected_text(format!("{:?}", self.core.login.factor))
                                .show_ui(ui, |ui| {
                                    for factor in self.core.login.factors.clone() {
                                        ui.selectable_value(
                                            &mut self.core.login.factor,
                                            factor,
                                            format!("{factor:?}"),
                                        );
                                    }
                                });
                            ui.label("Verification code");
                            let response = secret(ui, &mut self.code, "verification-code", true);
                            if !self.challenge_seen {
                                response.request_focus();
                                self.challenge_seen = true;
                            }
                            submit =
                                response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            if self.core.login.factor == Factor::Email
                                && ui.button("Send email code").clicked()
                            {
                                self.core.send_factor_email();
                            }
                        } else {
                            ui.label("Master password");
                            let response =
                                secret(ui, &mut self.password, "login-password", self.reveal);
                            if self.focus_login {
                                response.request_focus();
                                self.focus_login = false;
                            }
                            submit =
                                response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            ui.checkbox(&mut self.reveal, super::labels::text("Show password"));
                        }
                        if ui
                            .add_sized(
                                [ui.available_width(), 32.],
                                egui::Button::new(if challenge {
                                    "Verify"
                                } else if unlocking {
                                    "Unlock"
                                } else {
                                    "Log in"
                                }),
                            )
                            .clicked()
                        {
                            submit = true;
                        }
                    });
                    if self.core.login.busy {
                        ui.spinner();
                    }
                    if let Some(error) = &self.core.login.error {
                        ui.colored_label(egui::Color32::RED, error);
                    }
                    if submit && !self.core.login.busy {
                        if challenge {
                            self.core.submit_factor(std::mem::take(&mut self.code));
                        } else if unlocking {
                            self.core.unlock(std::mem::take(&mut self.password));
                        } else {
                            self.core.login(std::mem::take(&mut self.password));
                        }
                    }
                    ui.add_space(14.);
                    if !unlocking {
                        ui.weak(
                            self.core
                                .profile
                                .as_ref()
                                .map(|p| p.base.as_str())
                                .unwrap_or("No server selected"),
                        );
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
                                self.core.profiles.as_ref().and_then(|store| store.get(id));
                        }
                    }
                    if challenge && ui.button("Cancel login").clicked() {
                        self.core.cancel_login();
                        self.code = Zeroizing::default();
                    }
                },
            );
        });
    }
    pub(super) fn profile_dialog(&mut self, ctx: &egui::Context) {
        if !self.profile_open {
            return;
        }
        self.core.ensure_profiles();
        let revision = self.core.profile.as_ref().map(|p| (p.id, p.revision));
        if self.profile_loaded != revision {
            self.profile_loaded = revision;
            self.profile_base = self
                .core
                .profile
                .as_ref()
                .map(|p| p.base.clone())
                .unwrap_or_default();
            self.profile_allowed = self
                .core
                .profile
                .as_ref()
                .map(|p| {
                    p.allowed
                        .iter()
                        .skip(1)
                        .map(|o| format!("https://{}:{}", o.host, o.port))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
        }
        let mut open = true;
        egui::Window::new(super::labels::text("Server / mTLS settings"))
            .id(Id::unique("server-settings"))
            .open(&mut open)
            .collapsible(false)
            .default_width(520.)
            .show(ctx, |ui| {
                let bound = self.core.profile.as_ref().is_some_and(|p| p.account_bound);
                if bound {
                    ui.label("Server settings for the selected account");
                    ui.weak("To use a different server, add an account.");
                }
                if let Some(store) = &self.core.profiles {
                    let profiles = store.list();
                    egui::ComboBox::from_id_salt("server-profile")
                        .selected_text(
                            self.core
                                .profile
                                .as_ref()
                                .map(|p| p.name.as_str())
                                .unwrap_or("Select server"),
                        )
                        .show_ui(ui, |ui| {
                            for p in profiles {
                                if !bound
                                    && !p.account_bound
                                    && ui
                                        .selectable_label(
                                            revision.is_some_and(|r| r.0 == p.id),
                                            &p.name,
                                        )
                                        .clicked()
                                {
                                    self.core.select_profile(p.id);
                                }
                            }
                        });
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!bound, egui::Button::new("Bitwarden.com"))
                        .clicked()
                    {
                        self.core.select_cloud(false);
                    }
                    if ui
                        .add_enabled(!bound, egui::Button::new("Bitwarden.eu"))
                        .clicked()
                    {
                        self.core.select_cloud(true);
                    }
                });
                ui.label("Server URL (HTTPS)");
                ui.add_enabled(!bound, egui::TextEdit::singleline(&mut self.profile_base));
                ui.label("Additional allowed HTTPS origins (one per line)");
                ui.add(egui::TextEdit::multiline(&mut self.profile_allowed).desired_rows(2));
                ui.add_enabled_ui(!self.core.profile_busy, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Save server").clicked() {
                            self.core.save_server(
                                self.profile_base.clone(),
                                self.profile_allowed.clone(),
                            );
                        }
                        if ui.button("Test connection").clicked() {
                            self.core.probe_server();
                        }
                    });
                    ui.separator();
                    ui.strong("Client certificate (mTLS)");
                    if let Some(cert) = self
                        .core
                        .profile
                        .as_ref()
                        .and_then(|p| p.certificate.as_ref())
                    {
                        ui.label(&cert.subject);
                        ui.small(format!("SHA-256: {}", cert.fingerprint));
                        if let Some(date) = chrono::DateTime::from_timestamp(cert.not_after, 0) {
                            ui.label(format!("Expires: {}", date.format("%Y-%m-%d %H:%M UTC")));
                        }
                        if let Err(error) = cert.check_time() {
                            ui.colored_label(egui::Color32::RED, error);
                        }
                        if ui.button("Remove client certificate").clicked() {
                            self.core.remove_certificate(false);
                        }
                    } else {
                        ui.weak("No client certificate configured");
                    }
                    ui.label("PKCS#12 password");
                    secret(ui, &mut self.certificate_password, "pkcs12-password", false);
                    if ui.button("Import .p12 / .pfx…").clicked() {
                        self.core
                            .pick_certificate(std::mem::take(&mut self.certificate_password));
                    }
                    ui.separator();
                    ui.label(
                        if self
                            .core
                            .profile
                            .as_ref()
                            .is_some_and(|p| p.server_ca_pem.is_some())
                        {
                            "Private server CA configured"
                        } else {
                            "System server trust"
                        },
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Import private CA…").clicked() {
                            self.core.pick_ca();
                        }
                        if self
                            .core
                            .profile
                            .as_ref()
                            .is_some_and(|p| p.server_ca_pem.is_some())
                            && ui.button("Remove CA").clicked()
                        {
                            self.core.remove_certificate(true);
                        }
                    });
                });
                if self.core.profile_busy {
                    ui.spinner();
                }
                if let Some(error) = &self.core.profile_error {
                    ui.colored_label(egui::Color32::RED, error);
                }
            });
        self.profile_open = open;
    }
}
