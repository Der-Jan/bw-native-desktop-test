//! Login coordinator. Secrets and account candidates are never part of Debug output.
use super::*;
use crate::components::buttons::secondary as button;
use crate::services::{
    accounts::{
        login::{Attempt, Factor, PendingLogin, Step},
        profile::{ProfileStore, ServerProfile},
        storage::{atomic_write, read_bounded},
        transport::Transport,
    },
    sdk::production,
};
use iced::widget::{column, pick_list, text, text_input};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use zeroize::Zeroizing;

pub struct Slot<T>(Arc<Mutex<Option<T>>>);
impl<T> Clone for Slot<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> Slot<T> {
    fn new(v: T) -> Self {
        Self(Arc::new(Mutex::new(Some(v))))
    }
    fn take(&self) -> Option<T> {
        self.0.lock().ok()?.take()
    }
}
impl<T> std::fmt::Debug for Slot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<private one-shot result>")
    }
}
pub enum ResultStep {
    Challenge(Box<PendingLogin>, Vec<Factor>),
    Ready(Box<production::Candidate>),
}
#[derive(Clone, Debug)]
pub enum FlowMessage {
    Result(Uuid, Slot<Result<ResultStep, String>>),
    Code(String),
    Factor(Factor),
    Submit,
    SendEmail,
    Cancel,
    CertificatePassword(String),
    PickCertificate,
    CertificatePicked(Option<std::path::PathBuf>),
    ImportCertificate,
    CertificateImported(Uuid, Result<ServerProfile, String>),
    PickCa,
    CaPicked(Uuid, Result<Option<String>, String>),
    Origins(String),
    Settings,
    UseProfile(Uuid),
    RemoveCertificate,
    RemoveCa,
    Probe,
    Sync,
    Probed(Result<(), String>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ProfileChoice {
    id: Uuid,
    label: String,
}
#[derive(Default)]
pub(super) struct Flow {
    attempt: Option<Attempt>,
    pending: Option<Box<PendingLogin>>,
    factors: Vec<Factor>,
    factor: Option<Factor>,
    code: String,
    busy: bool,
    error: Option<String>,
    ready: bool,
    pub(super) certificate_password: String,
    certificate_path: Option<std::path::PathBuf>,
    pub(super) origins: String,
    pub(super) profile: Option<ServerProfile>,
    pub(super) settings_busy: bool,
    settings_error: Option<String>,
    selected_profile: Option<Uuid>,
    saved_profiles: Vec<ProfileChoice>,
    close_after_import: bool,
}
impl LoginView {
    pub fn cancel_login(&mut self) {
        if let Some(a) = self.flow.attempt.take() {
            a.cancel();
        }
        self.flow.pending = None;
        self.flow.code.clear();
        self.flow.busy = false;
        self.flow.ready = false;
        self.flow.error = None;
    }
    pub fn show_ready(&mut self) {
        self.flow.ready = true;
        self.flow.busy = false;
    }
    fn profile(&self, store: &ProfileStore) -> Result<ServerProfile, String> {
        let server = match &self.auth_page {
            AuthPage::LoginEmail {
                selected_server, ..
            }
            | AuthPage::LoginPassword {
                selected_server, ..
            } => selected_server.clone(),
            _ => return Err("Select a server first".into()),
        };
        let draft = match server {
            ServerOption::Bitwarden => ServerProfile::cloud(false),
            ServerOption::BitwardenEu => ServerProfile::cloud(true),
            ServerOption::SelfHosted(url) => ServerProfile::new(&url)?,
        };
        if let Some(id) = self.flow.selected_profile {
            let profile = store.get(id).ok_or(
                "Das ausgewählte Serverprofil fehlt. Bitte in den Einstellungen erneut auswählen.",
            )?;
            if profile.base != draft.base || profile.account_bound {
                return Err("Die Serverauswahl stimmt nicht mit dem gespeicherten Profil überein. Bitte das gewünschte Profil erneut auswählen.".into());
            }
            return Ok(profile);
        }
        let matching: Vec<_> = store
            .list()
            .into_iter()
            .filter(|p| p.base == draft.base && !p.account_bound)
            .collect();
        if matching.is_empty() {
            store.save(draft, None)
        } else {
            ProfileStore::preferred_login_profile(matching)
        }
    }
    fn apply_server(&mut self, profile: &ServerProfile) {
        let option = if profile.base == "https://vault.bitwarden.com/" {
            ServerOption::Bitwarden
        } else if profile.base == "https://vault.bitwarden.eu/" {
            ServerOption::BitwardenEu
        } else {
            ServerOption::SelfHosted(profile.base.clone())
        };
        match &mut self.auth_page {
            AuthPage::LoginEmail {
                selected_server, ..
            } => *selected_server = option,
            AuthPage::LoginPassword {
                selected_server,
                password_input,
                ..
            } => {
                if *selected_server != option {
                    use zeroize::Zeroize;
                    password_input.zeroize();
                }
                *selected_server = option;
            }
            _ => {}
        }
        self.flow.selected_profile = Some(profile.id);
    }
    pub(super) fn restore_server(&mut self, mgr: &ClientManager) {
        self.flow.selected_profile = None;
        self.flow.profile = None;
        if let Some(store) = mgr.profiles() {
            match store.selected_login_profile() {
                Ok(Some(profile)) => {
                    self.apply_server(&profile);
                    self.flow.profile = Some(profile);
                }
                Ok(None) => {}
                Err(error) => self.flow.error = Some(error),
            }
        }
    }
    fn fill_server_settings(&mut self, profile: ServerProfile) {
        self.self_hosted_modal.url_input = profile.base.clone();
        self.flow.origins = profile
            .allowed
            .iter()
            .skip(1)
            .map(|o| format!("https://{}:{}", o.host, o.port))
            .collect::<Vec<_>>()
            .join(" ");
        self.flow.profile = Some(profile);
        self.flow.settings_error = None;
        self.flow.certificate_path = None;
        self.flow.certificate_password.clear();
    }
    fn refresh_profile_choices(&mut self, store: &ProfileStore) {
        self.flow.saved_profiles = store
            .list()
            .into_iter()
            .filter(|p| !p.account_bound)
            .map(|p| {
                let label = format!(
                    "{} · {}",
                    p.base,
                    p.certificate
                        .as_ref()
                        .map(|c| format!("mTLS {}", &c.fingerprint[..c.fingerprint.len().min(12)]))
                        .unwrap_or("ohne Clientzertifikat".into())
                );
                ProfileChoice { id: p.id, label }
            })
            .collect();
    }
    pub(super) fn open_custom_settings(&mut self, mgr: &ClientManager) -> Result<(), String> {
        let store = mgr.profiles().ok_or("Server settings unavailable")?;
        self.refresh_profile_choices(&store);
        let matching: Vec<_> = store
            .list()
            .into_iter()
            .filter(|p| {
                !p.account_bound
                    && p.base != ServerProfile::cloud(false).base
                    && p.base != ServerProfile::cloud(true).base
            })
            .collect();
        let current = self.profile(&store).ok().filter(|p| {
            p.base != ServerProfile::cloud(false).base && p.base != ServerProfile::cloud(true).base
        });
        if let Some(profile) = current {
            self.fill_server_settings(profile);
        } else if !matching.is_empty() && matching.iter().all(|p| p.base == matching[0].base) {
            self.fill_server_settings(ProfileStore::preferred_login_profile(matching)?);
        } else {
            self.flow.profile = None;
            self.flow.origins.clear();
            self.self_hosted_modal.url_input.clear();
            self.flow.settings_error = None;
        }
        self.self_hosted_modal.url_error = false;
        self.self_hosted_modal.fade.open();
        Ok(())
    }
    pub(super) fn save_server_dialog(&mut self, mgr: &mut ClientManager) -> Outcome<Self> {
        if self.flow.settings_busy {
            return Outcome::None;
        }
        match validate_self_hosted_url(&self.self_hosted_modal.url_input) {
            SelfHostedUrl::Empty => return Outcome::None,
            SelfHostedUrl::Invalid => {
                self.self_hosted_modal.url_error = true;
                return Outcome::None;
            }
            SelfHostedUrl::Accepted(_) => {}
        }
        if self.flow.certificate_path.is_some() {
            self.flow.close_after_import = true;
            return self
                .flow_update(
                    &LoginMessage::Flow(FlowMessage::ImportCertificate),
                    mgr,
                    None,
                )
                .unwrap_or(Outcome::None);
        }
        match self.save_server(mgr) {
            Ok(()) => {
                self.self_hosted_modal.fade.close();
                self.flow.certificate_password.clear();
                Outcome::None
            }
            Err(error) => Outcome::toast(Toast::error(error, None)),
        }
    }
    pub(super) fn select_server(
        &mut self,
        server: ServerOption,
        mgr: &ClientManager,
    ) -> Result<(), String> {
        self.flow.selected_profile = None;
        match &mut self.auth_page {
            AuthPage::LoginEmail {
                selected_server, ..
            }
            | AuthPage::LoginPassword {
                selected_server, ..
            } => *selected_server = server,
            _ => return Ok(()),
        }
        let store = mgr.profiles().ok_or("Server settings unavailable")?;
        let profile = self.profile(&store)?;
        store.select_login_profile(profile.id)?;
        self.apply_server(&profile);
        self.flow.profile = Some(profile);
        Ok(())
    }
    pub(super) fn save_server(&mut self, mgr: &ClientManager) -> Result<(), String> {
        let store = mgr
            .profiles()
            .ok_or("Server settings are not available yet")?;
        let mut profile = if let Some(profile) = &self.flow.profile {
            profile.clone()
        } else {
            let draft = ServerProfile::new(&self.self_hosted_modal.url_input)?;
            let matching: Vec<_> = store
                .list()
                .into_iter()
                .filter(|p| p.base == draft.base && !p.account_bound)
                .collect();
            if matching.is_empty() {
                draft
            } else {
                ProfileStore::preferred_login_profile(matching)?
            }
        };
        if profile.account_bound
            && profile.base != ServerProfile::new(&self.self_hosted_modal.url_input)?.base
        {
            return Err("Create a new login to change this account's server".into());
        }
        profile.base = ServerProfile::new(&self.self_hosted_modal.url_input)?.base;
        profile.allowed = vec![crate::services::accounts::profile::Origin::of(
            &url::Url::parse(&profile.base).map_err(|_| "Invalid server")?,
        )?];
        for line in self
            .flow
            .origins
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.trim().is_empty())
        {
            let origin = crate::services::accounts::profile::Origin::parse(line)?;
            if !profile.allowed.contains(&origin) {
                profile.allowed.push(origin)
            }
        }
        let expected = (profile.revision > 0).then_some(profile.revision);
        let saved = store.save(profile, expected)?;
        self.flow.profile = Some(saved.clone());
        if !saved.account_bound {
            store.select_login_profile(saved.id)?;
            self.cancel_login();
            self.apply_server(&saved);
        }
        self.refresh_profile_choices(&store);
        Ok(())
    }
    pub(super) fn start_login(&mut self, mgr: &ClientManager) -> Outcome<Self> {
        if self.flow.busy {
            return Outcome::None;
        }
        if crate::paths::is_demo() {
            return Outcome::toast(Toast::warning(
                "Echte Anmeldung bitte über start-login.sh oder den normalen App-Start öffnen.",
                None,
            ));
        }
        let (email, password) = match &mut self.auth_page {
            AuthPage::LoginPassword {
                email,
                password_input,
                ..
            } => (
                email.clone(),
                Zeroizing::new(std::mem::take(password_input)),
            ),
            _ => return Outcome::None,
        };
        let Some(store) = mgr.profiles() else {
            return Outcome::toast(Toast::error("Server settings unavailable", None));
        };
        let profile = match self.profile(&store) {
            Ok(p) => p,
            Err(e) => return Outcome::toast(Toast::error(e, None)),
        };
        self.cancel_login();
        let attempt = Attempt::new();
        let id = attempt.id;
        self.flow.attempt = Some(attempt.clone());
        self.flow.busy = true;
        Outcome::perform(
            async move {
                let root = crate::paths::data_dir();
                let transport_store = store.clone();
                let (transport, device) = tokio::task::spawn_blocking(move || {
                    Ok::<_, String>((
                        Transport::new(&transport_store, profile)?,
                        device_id(&root)?,
                    ))
                })
                .await
                .map_err(|_| "Could not prepare login")??;
                let pending =
                    PendingLogin::prepare(attempt.clone(), transport, email, password, device)
                        .await?;
                finish(pending.submit(None).await?, store, attempt).await
            },
            move |result| LoginMessage::Flow(FlowMessage::Result(id, Slot::new(result))),
        )
    }
    pub(super) fn flow_update(
        &mut self,
        msg: &LoginMessage,
        mgr: &mut ClientManager,
        active_user: Option<UserId>,
    ) -> Option<Outcome<Self>> {
        let LoginMessage::Flow(msg) = msg else {
            return None;
        };
        let mut outcome = Outcome::None;
        match msg.clone() {
            FlowMessage::Sync => return Some(Outcome::event(LoginEvent::SyncRequested)),
            FlowMessage::Probe => {
                if self.flow.busy {
                    return Some(outcome);
                }
                let uid = active_user?;
                let client = mgr.client_for(&uid)?;
                let store = mgr.profiles()?;
                self.flow.busy = true;
                outcome = Outcome::perform(
                    crate::services::sdk::refresh::probe(client, crate::paths::data_dir(), store),
                    |r| LoginMessage::Flow(FlowMessage::Probed(r)),
                );
            }
            FlowMessage::Probed(result) => {
                self.flow.busy = false;
                outcome = match result {
                    Ok(()) => Outcome::toast(Toast::success(
                        "Verbindung und Anmeldung bestätigt. Keine Synchronisierung ausgeführt.",
                        None,
                    )),
                    Err(e) => Outcome::toast(Toast::error(e, None)),
                };
            }
            FlowMessage::Result(id, slot) => {
                let result = slot.take()?;
                if !self
                    .flow
                    .attempt
                    .as_ref()
                    .is_some_and(|a| a.id == id && a.check().is_ok())
                {
                    return Some(Outcome::None);
                }
                self.flow.busy = false;
                match result {
                    Ok(ResultStep::Challenge(p, factors)) => {
                        self.flow.factor = factors.first().copied();
                        self.flow.factors = factors;
                        self.flow.pending = Some(p);
                        self.flow.error=Some("Enter your verification code. If a previous code was rejected, try a new one.".into());
                        self.flow.code.clear();
                    }
                    Ok(ResultStep::Ready(candidate)) => {
                        match mgr.accept_login(*candidate, self.flow.attempt.as_ref().unwrap()) {
                            Ok(uid) => {
                                self.flow.ready = true;
                                self.flow.attempt = None;
                                outcome = Outcome::event(LoginEvent::LoggedIn { uid });
                            }
                            Err(e) => self.flow.error = Some(e),
                        }
                    }
                    Err(e) => {
                        self.flow.error = Some(e);
                        self.flow.pending = None;
                        self.flow.attempt = None;
                    }
                }
            }
            FlowMessage::Code(code) => self.flow.code = code,
            FlowMessage::Factor(f) => self.flow.factor = Some(f),
            FlowMessage::Submit => {
                if self.flow.busy {
                    return Some(outcome);
                }
                let Some(pending) = self.flow.pending.take() else {
                    return Some(outcome);
                };
                let Some(factor) = self.flow.factor else {
                    return Some(outcome);
                };
                let code = Zeroizing::new(std::mem::take(&mut self.flow.code));
                let attempt = pending.attempt.clone();
                let id = attempt.id;
                let store = mgr.profiles()?;
                self.flow.busy = true;
                self.flow.error = None;
                outcome = Outcome::perform(
                    async move {
                        finish(pending.submit(Some((factor, code))).await?, store, attempt).await
                    },
                    move |r| LoginMessage::Flow(FlowMessage::Result(id, Slot::new(r))),
                );
            }
            FlowMessage::SendEmail => {
                if self.flow.busy {
                    return Some(outcome);
                }
                let pending = self.flow.pending.take()?;
                let id = pending.attempt.id;
                self.flow.busy = true;
                // Return the password-bearing pending flow through the one-shot result, never Clone it.
                outcome = Outcome::perform(
                    async move {
                        let result = pending.send_email().await;
                        (pending, result)
                    },
                    move |(pending, result)| {
                        let factors = vec![Factor::Email];
                        if result.is_err() {
                            LoginMessage::Flow(FlowMessage::Result(
                                id,
                                Slot::new(Err("Could not send email code; restart login".into())),
                            ))
                        } else {
                            LoginMessage::Flow(FlowMessage::Result(
                                id,
                                Slot::new(Ok(ResultStep::Challenge(pending, factors))),
                            ))
                        }
                    },
                );
            }
            FlowMessage::Cancel => self.cancel_login(),
            FlowMessage::CertificatePassword(s) => self.flow.certificate_password = s,
            FlowMessage::Origins(s) => self.flow.origins = s,
            FlowMessage::Settings => {
                let store = mgr.profiles()?;
                let account =
                    if self.flow.ready || matches!(self.auth_page, AuthPage::Unlock { .. }) {
                        active_user
                    } else {
                        None
                    };
                let profile = if let Some(uid) = account.filter(|uid| mgr.is_remote(uid)) {
                    mgr.profile_for(&uid).and_then(|id| store.get(id))
                } else {
                    self.profile(&store).ok()
                };
                self.refresh_profile_choices(&store);
                if let Some(profile) = profile {
                    self.fill_server_settings(profile);
                }
                self.self_hosted_modal.fade.open();
            }
            FlowMessage::UseProfile(id) => {
                if self.flow.settings_busy {
                    return Some(outcome);
                }
                let store = mgr.profiles()?;
                let profile = store.get(id).filter(|p| !p.account_bound)?;
                match store.select_login_profile(id) {
                    Ok(()) => {
                        self.cancel_login();
                        self.apply_server(&profile);
                        self.fill_server_settings(profile);
                    }
                    Err(error) => self.flow.settings_error = Some(error),
                }
            }
            FlowMessage::PickCertificate => {
                outcome = Outcome::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("PKCS#12", &["p12", "pfx"])
                            .pick_file()
                            .await
                            .map(|p| p.path().to_path_buf())
                    },
                    |p| LoginMessage::Flow(FlowMessage::CertificatePicked(p)),
                );
            }
            FlowMessage::CertificatePicked(p) => self.flow.certificate_path = p,
            FlowMessage::ImportCertificate => {
                if self.flow.settings_busy {
                    return Some(outcome);
                }
                if let Err(e) = self.save_server(mgr) {
                    return Some(Outcome::toast(Toast::error(e, None)));
                }
                let store = mgr.profiles()?;
                let profile = self.flow.profile.clone()?;
                let id = profile.id;
                let path = self.flow.certificate_path.clone()?;
                let password = Zeroizing::new(std::mem::take(&mut self.flow.certificate_password));
                self.flow.settings_busy = true;
                outcome = Outcome::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            crate::services::accounts::certificate::import(
                                &store, profile, &path, password,
                            )
                        })
                        .await
                        .unwrap_or_else(|_| Err("Certificate import failed".into()))
                    },
                    move |r| LoginMessage::Flow(FlowMessage::CertificateImported(id, r)),
                );
            }
            FlowMessage::CertificateImported(id, result) => {
                self.flow.settings_busy = false;
                if self.flow.profile.as_ref().map(|p| p.id) != Some(id) {
                    return Some(outcome);
                }
                match result {
                    Ok(p) => {
                        self.flow.profile = Some(p);
                        self.flow.certificate_path = None;
                        self.flow.settings_error = None;
                        self.refresh_profile_choices(&mgr.profiles()?);
                        if self.flow.close_after_import {
                            self.self_hosted_modal.fade.close();
                        }
                    }
                    Err(e) => self.flow.settings_error = Some(e),
                }
                self.flow.close_after_import = false;
            }
            FlowMessage::RemoveCertificate => {
                if self.flow.settings_busy {
                    return Some(outcome);
                }
                let store = mgr.profiles()?;
                let mut p = self.flow.profile.clone()?;
                let revision = p.revision;
                p.certificate = None;
                match store.save(p, Some(revision)) {
                    Ok(p) => self.flow.profile = Some(p),
                    Err(e) => self.flow.settings_error = Some(e),
                }
            }
            FlowMessage::RemoveCa => {
                let store = mgr.profiles()?;
                let mut p = self.flow.profile.clone()?;
                let revision = p.revision;
                p.server_ca_pem = None;
                match store.save(p, Some(revision)) {
                    Ok(p) => self.flow.profile = Some(p),
                    Err(e) => self.flow.settings_error = Some(e),
                }
            }
            FlowMessage::PickCa => {
                if let Err(e) = self.save_server(mgr) {
                    return Some(Outcome::toast(Toast::error(e, None)));
                }
                let store = mgr.profiles()?;
                let profile = self.flow.profile.clone()?;
                let id = profile.id;
                outcome = Outcome::perform(
                    async move {
                        let Some(file) = rfd::AsyncFileDialog::new()
                            .add_filter("PEM CA certificates", &["pem", "crt"])
                            .pick_file()
                            .await
                        else {
                            return Ok(None);
                        };
                        tokio::task::spawn_blocking(move || {
                            let bytes = read_bounded(file.path(), 1024 * 1024)?;
                            let pem = crate::services::accounts::certificate::validate_ca(&bytes)?;
                            let rev = profile.revision;
                            let mut p = profile;
                            p.server_ca_pem = Some(pem);
                            store.save(p, Some(rev))?;
                            Ok(Some("CA saved".into()))
                        })
                        .await
                        .map_err(|_| "CA import failed")?
                    },
                    move |r| LoginMessage::Flow(FlowMessage::CaPicked(id, r)),
                );
            }
            FlowMessage::CaPicked(id, result) => {
                if self.flow.profile.as_ref().map(|p| p.id) != Some(id) {
                    return Some(outcome);
                }
                if let Err(e) = result {
                    self.flow.settings_error = Some(e)
                } else {
                    self.flow.profile = mgr.profiles().and_then(|s| s.get(id));
                }
            }
        }
        Some(outcome)
    }
    pub(super) fn flow_view<'a>(
        &'a self,
        ctx: &RenderCtx<'a>,
    ) -> Option<Element<'a, LoginMessage, AppTheme>> {
        let f = &self.flow;
        if !f.ready && !f.busy && f.pending.is_none() && f.error.is_none() {
            return None;
        }
        let m = |m| LoginMessage::Flow(m);
        let content: Element<'a, LoginMessage, AppTheme> = if f.ready {
            let mut body = column![
                text("Angemeldet").size(28),
                text("Login und lokale Schlüsselprüfung waren erfolgreich."),
                text(if ctx.syncing {
                    "Der verschlüsselte Tresor wird heruntergeladen und geprüft …"
                } else {
                    "Noch kein vollständiger lokaler Tresor vorhanden."
                }),
                button(if ctx.syncing {
                    "Synchronisierung läuft …"
                } else {
                    "Tresor synchronisieren"
                })
                .on_press_maybe((!ctx.syncing).then(|| m(FlowMessage::Sync))),
                button("Verbindung prüfen (ohne Sync)")
                    .on_press_maybe((!f.busy).then(|| m(FlowMessage::Probe))),
                button("Server / mTLS-Einstellungen").on_press(m(FlowMessage::Settings)),
                button("Sperren").on_press(LoginMessage::AccountSwitcher(
                    AccountSwitcherMessage::LockActive
                )),
            ]
            .spacing(16)
            .width(420);
            if let Some(error) = ctx.sync_error {
                body = body.push(text(error).size(14));
            }
            body.into()
        } else {
            let mut body = column![
                text(if f.busy {
                    "Anmeldung läuft …"
                } else {
                    "Anmeldung"
                })
                .size(26)
            ]
            .spacing(16)
            .width(420);
            if let Some(error) = &f.error {
                body = body
                    .push(text(error).size(14))
                    .push(button("Server / mTLS-Einstellungen").on_press(m(FlowMessage::Settings)));
            }
            if f.pending.is_some() {
                body = body
                    .push(
                        pick_list(f.factor, f.factors.clone(), |v| v.to_string())
                            .on_select(move |v| m(FlowMessage::Factor(v))),
                    )
                    .push(
                        text_input("Bestätigungscode", &f.code)
                            .secure(true)
                            .on_input(move |v| m(FlowMessage::Code(v)))
                            .on_submit(m(FlowMessage::Submit)),
                    )
                    .push(
                        button("Bestätigen")
                            .on_press_maybe((!f.busy).then(|| m(FlowMessage::Submit))),
                    );
                if f.factors.contains(&Factor::Email) {
                    body = body.push(
                        button("Code per E-Mail senden")
                            .on_press_maybe((!f.busy).then(|| m(FlowMessage::SendEmail))),
                    );
                }
            }
            body.push(button("Zurück / Abbrechen").on_press(m(FlowMessage::Cancel)))
                .into()
        };
        Some(layout::auth_page_shell(
            content,
            server_selector::simple_status(ctx.active_server_url, ctx.colors),
            ctx.active_email,
            ctx.accounts,
            ctx.open_overlay == Some(Overlay::AccountSwitcher),
            ctx.colors,
        ))
    }
    pub(super) fn certificate_controls<'a>(&'a self) -> Element<'a, LoginMessage, AppTheme> {
        let f = &self.flow;
        let m = |m| LoginMessage::Flow(m);
        let mut body=column![text("mTLS (optional)").size(18),text("PKCS#12 mit genau einem privaten Schlüssel. Die Identität gilt nur für dieses Serverprofil.").size(12)].spacing(10);
        if !f.saved_profiles.is_empty() && !f.profile.as_ref().is_some_and(|p| p.account_bound) {
            let selected = f
                .profile
                .as_ref()
                .and_then(|p| f.saved_profiles.iter().find(|choice| choice.id == p.id))
                .cloned();
            body = body.push(text("Gespeicherte Serverprofile").size(12)).push(
                pick_list(selected, f.saved_profiles.clone(), |choice| {
                    choice.label.clone()
                })
                .on_select(|choice| LoginMessage::Flow(FlowMessage::UseProfile(choice.id))),
            );
        }
        if let Some(c) = f.profile.as_ref().and_then(|p| p.certificate.as_ref()) {
            body = body
                .push(
                    text(format!(
                        "{}\nSHA-256: {}\nGültig bis: {}",
                        c.subject,
                        c.fingerprint,
                        chrono::DateTime::from_timestamp(c.not_after, 0)
                            .map(|d| d.to_rfc3339())
                            .unwrap_or_default()
                    ))
                    .size(12),
                )
                .push(button("Zertifikat entfernen").on_press(m(FlowMessage::RemoveCertificate)));
        }
        body = body.push(button("PKCS#12 auswählen …").on_press(m(FlowMessage::PickCertificate)));
        if let Some(path) = &f.certificate_path {
            body = body
                .push(
                    text(
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                    )
                    .size(12),
                )
                .push(
                    text_input("PKCS#12-Passwort", &f.certificate_password)
                        .secure(true)
                        .on_input(move |v| m(FlowMessage::CertificatePassword(v))),
                )
                .push(
                    button(if f.settings_busy {
                        "Import läuft …"
                    } else {
                        "Zertifikat importieren / ersetzen"
                    })
                    .on_press_maybe((!f.settings_busy).then(|| m(FlowMessage::ImportCertificate))),
                );
        }
        body=body.push(text("Zusätzlich erlaubte HTTPS-Ursprünge (durch Leerzeichen getrennt, einschließlich Port)").size(12))
            .push(text_input("https://identity.example.org",&f.origins).on_input(move|s|m(FlowMessage::Origins(s))))
            .push(button("Private Server-CA importieren (PEM) …").on_press(m(FlowMessage::PickCa)));
        if f.profile
            .as_ref()
            .is_some_and(|p| p.server_ca_pem.is_some())
        {
            body = body
                .push(text("Zusätzliche Server-CA eingerichtet").size(12))
                .push(button("Zusätzliche Server-CA entfernen").on_press(m(FlowMessage::RemoveCa)));
        }
        if let Some(e) = &f.settings_error {
            body = body.push(text(e).size(12));
        }
        body.into()
    }
}
async fn finish(step: Step, store: ProfileStore, attempt: Attempt) -> Result<ResultStep, String> {
    match step {
        Step::Challenge(p, f) => Ok(ResultStep::Challenge(p, f)),
        Step::Authenticated(s) => production::prepare(crate::paths::data_dir(), store, attempt, *s)
            .await
            .map(|candidate| ResultStep::Ready(Box::new(candidate))),
    }
}
fn device_id(root: &std::path::Path) -> Result<Uuid, String> {
    let path = root.join("device-id");
    if path.exists() {
        return std::str::from_utf8(&read_bounded(&path, 100)?)
            .map_err(|_| "Invalid device ID")?
            .trim()
            .parse()
            .map_err(|_| "Invalid device ID".into());
    }
    let id = Uuid::new_v4();
    atomic_write(root, "device-id", id.to_string().as_bytes())
        .map_err(|_| "Could not save device ID")?;
    Ok(id)
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use crate::{
        app::App,
        services::{
            accounts::{
                profile::CertificateInfo,
                storage::{SecretStore, test_support::MemoryKeys},
            },
            sdk::test_support::manager_with_profiles,
        },
    };
    fn setup() -> (std::path::PathBuf, ProfileStore, App) {
        let root = std::env::temp_dir().join(format!("bw-login-settings-test-{}", Uuid::new_v4()));
        let secrets = SecretStore::with_keys(&root, Arc::new(MemoryKeys::default()));
        let store = ProfileStore::open(&root, secrets).unwrap();
        let mut app = App::test();
        app.client_manager = manager_with_profiles(store.clone());
        (root, store, app)
    }
    fn with_certificate(store: &ProfileStore, mut profile: ServerProfile) -> ServerProfile {
        profile.certificate = Some(CertificateInfo {
            fingerprint: "ab".repeat(32),
            subject: "Synthetic identity".into(),
            not_before: 0,
            not_after: chrono::Utc::now().timestamp() + 86400,
            secret: store
                .secrets
                .put(b"synthetic encrypted identity fixture")
                .unwrap(),
        });
        let rev = profile.revision;
        store.save(profile, Some(rev)).unwrap()
    }
    #[tokio::test]
    async fn saving_on_password_page_changes_login_binding_and_restores_certificate_after_restart()
    {
        let (root, store, mut app) = setup();
        let mut view = LoginView::new();
        view.auth_page =
            AuthPage::new_login_password("test@example.invalid".into(), ServerOption::Bitwarden);
        view.flow_update(
            &LoginMessage::Flow(FlowMessage::Settings),
            &mut app.client_manager,
            None,
        );
        view.self_hosted_modal.url_input = "https://custom.example.invalid/vault/".into();
        view.flow.origins.clear();
        assert!(matches!(
            view.update(LoginMessage::SelfHostedSave, app.update_ctx()),
            Outcome::None
        ));
        let saved = view.profile(&store).unwrap();
        assert_eq!(saved.base, "https://custom.example.invalid/vault/");
        assert!(
            matches!(&view.auth_page,AuthPage::LoginPassword{selected_server:ServerOption::SelfHosted(url),..} if url==&saved.base)
        );
        let saved = with_certificate(&store, saved);
        view.flow_update(
            &LoginMessage::Flow(FlowMessage::CertificateImported(
                saved.id,
                Ok(saved.clone()),
            )),
            &mut app.client_manager,
            None,
        );
        view.save_server(&app.client_manager).unwrap();
        // A second URL-matching profile must never replace the selected identity.
        store
            .save(ServerProfile::new(&saved.base).unwrap(), None)
            .unwrap();
        let reopened = ProfileStore::open(&root, store.secrets.clone()).unwrap();
        let mut manager = manager_with_profiles(reopened.clone());
        let mut restored = LoginView::new();
        restored.show_unlock_for(None, &manager);
        let selected = restored.profile(&reopened).unwrap();
        assert_eq!(selected.id, saved.id);
        assert_eq!(
            selected.certificate.as_ref().unwrap().secret,
            saved.certificate.as_ref().unwrap().secret
        );
        restored.flow_update(
            &LoginMessage::Flow(FlowMessage::Settings),
            &mut manager,
            None,
        );
        assert_eq!(restored.flow.profile.as_ref().unwrap().id, saved.id);
        assert!(
            restored
                .flow
                .profile
                .as_ref()
                .unwrap()
                .certificate
                .is_some()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn old_duplicate_profiles_recover_the_unique_certificate_binding_but_explicit_cloud_choice_sticks()
     {
        let (root, store, mut app) = setup();
        store.save(ServerProfile::cloud(false), None).unwrap();
        let plain = store
            .save(
                ServerProfile::new("https://legacy.example.invalid/").unwrap(),
                None,
            )
            .unwrap();
        let cert = store
            .save(ServerProfile::new(&plain.base).unwrap(), None)
            .unwrap();
        let cert = with_certificate(&store, cert);
        let mut view = LoginView::new();
        view.show_unlock_for(None, &app.client_manager);
        assert_eq!(view.profile(&store).unwrap().id, cert.id);
        assert!(root.join("login-profile.json").exists());
        view.select_server(ServerOption::BitwardenEu, &app.client_manager)
            .unwrap();
        let reopen = ProfileStore::open(&root, store.secrets.clone()).unwrap();
        app.client_manager = manager_with_profiles(reopen.clone());
        view.reset_to_email_entry(&app.client_manager);
        assert_eq!(
            view.profile(&reopen).unwrap().base,
            "https://vault.bitwarden.eu/"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn save_with_pending_certificate_import_waits_for_success_and_preserves_import_failure() {
        let (root, _store, mut app) = setup();
        let mut view = LoginView::new();
        view.reset_to_email_entry(&app.client_manager);
        view.self_hosted_modal.url_input = "https://custom.example.invalid/".into();
        view.self_hosted_modal.fade.open();
        view.flow.certificate_path = Some(root.join("not-read-in-this-update-test.p12"));
        view.flow.certificate_password = "synthetic password".into();
        let task = view.update(LoginMessage::SelfHostedSave, app.update_ctx());
        assert!(matches!(task, Outcome::Task(_)));
        assert!(view.flow.settings_busy);
        assert!(view.self_hosted_modal.fade.is_open());
        assert!(view.flow.certificate_password.is_empty());
        // Error delivery keeps the dialog open and tells the user import did not complete.
        let id = view.flow.profile.as_ref().unwrap().id;
        view.update(
            LoginMessage::Flow(FlowMessage::CertificateImported(
                id,
                Err("Synthetic wrong certificate password".into()),
            )),
            app.update_ctx(),
        );
        assert!(view.self_hosted_modal.fade.is_open());
        assert!(!view.flow.settings_busy);
        assert!(
            view.flow
                .settings_error
                .as_ref()
                .unwrap()
                .contains("wrong certificate password")
        );
        drop(task);
        std::fs::remove_dir_all(root).unwrap();
    }
}
