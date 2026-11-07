mod flow;
mod handler;
mod layout;
mod login_email;
mod login_password;
mod self_hosted_modal;
mod server_selector;
mod unlock;

use iced::{Element, Task};

use crate::{
    app::{Outcome, Overlay, RenderCtx, UpdateCtx, View},
    components::{
        FadeInOut,
        account_switcher::{AccountSwitcherEvent, AccountSwitcherMessage},
        toast::Toast,
    },
    domain::{UnlockMethod, UserId},
    services::sdk::ClientManager,
    theme::AppTheme,
    views::login::{
        login_email::LOGIN_EMAIL_FIELD_ID, login_password::LOGIN_PASSWORD_FIELD_ID,
        self_hosted_modal::SELF_HOSTED_URL_FIELD_ID, unlock::UNLOCK_FIELD_ID,
    },
};

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub(in crate::views::login) enum AuthPage {
    Unlock {
        method: UnlockMethod,
        password_input: String,
        pin_input: String,
    },
    LoginEmail {
        email_input: String,
        remember_email: bool,
        selected_server: ServerOption,
    },
    LoginPassword {
        email: String,
        password_input: String,
        selected_server: ServerOption,
    },
}

impl AuthPage {
    pub fn new_unlock(method: UnlockMethod) -> Self {
        Self::Unlock {
            method,
            password_input: String::new(),
            pin_input: String::new(),
        }
    }

    pub fn new_login_email() -> Self {
        Self::LoginEmail {
            email_input: String::new(),
            remember_email: false,
            selected_server: ServerOption::Bitwarden,
        }
    }

    pub fn new_login_password(email: String, selected_server: ServerOption) -> Self {
        Self::LoginPassword {
            email,
            password_input: String::new(),
            selected_server,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerOption {
    Bitwarden,
    BitwardenEu,
    SelfHosted(String),
}

/// Result of validating user input for the self-hosted server URL field.
/// The three states match the modal's UX contract: empty input is a no-op
/// (don't surface an error, don't persist), invalid input toggles the
/// error state, and accepted input is the trimmed URL ready to persist.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::views::login) enum SelfHostedUrl {
    /// User submitted an empty (or whitespace-only) field. No-op.
    Empty,
    /// User submitted a non-empty URL that didn't pass validation. The
    /// modal should flip its error indicator and stay open.
    Invalid,
    /// User submitted an `https://` URL. Persist this exact (trimmed)
    /// string as the active self-hosted server.
    Accepted(String),
}

/// Gate for the self-hosted server URL field. The only requirement enforced
/// here is `https://` — anything `http`, `javascript:`, etc. is rejected
/// because the SDK then issues authenticated requests against this origin.
pub(in crate::views::login) fn validate_self_hosted_url(raw: &str) -> SelfHostedUrl {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return SelfHostedUrl::Empty;
    }
    if !trimmed.starts_with("https://") {
        return SelfHostedUrl::Invalid;
    }
    SelfHostedUrl::Accepted(trimmed.to_string())
}

impl ServerOption {
    pub fn display_name(&self) -> String {
        match self {
            ServerOption::Bitwarden => "bitwarden.com".to_string(),
            ServerOption::BitwardenEu => "bitwarden.eu".to_string(),
            ServerOption::SelfHosted(url) if url.is_empty() => {
                crate::fl!("login-server-self-hosted")
            }
            ServerOption::SelfHosted(url) => url.clone(),
        }
    }
}

#[derive(Clone)]
pub enum LoginMessage {
    Flow(flow::FlowMessage),
    PasswordChanged(String),
    Unlock,
    UnlockCompleted(UserId, Result<(), String>),
    PinChanged(String),
    UnlockWithPin,
    UnlockWithBiometrics,
    SwitchUnlockMethod(UnlockMethod),

    EmailChanged(String),
    ToggleRememberEmail(bool),
    ContinueWithEmail,
    UseSingleSignOn,

    LoginPasswordChanged(String),
    LoginWithPassword,
    // Forward stub: the handler arm exists and is correct, but no caller
    // dispatches this until `LoginWithPassword` is wired to the SDK login
    // call. Remove the `#[expect]` then.
    #[expect(dead_code)]
    LoginCompleted(UserId, Result<(), String>),
    BackToEmail,
    GetPasswordHint,

    ToggleServerSelector,
    SelectServer(ServerOption),

    SelfHostedUrlChanged(String),
    SelfHostedSave,
    SelfHostedCancel,

    AccountSwitcher(AccountSwitcherMessage),
}

#[derive(Debug, Clone, derive_more::From)]
pub enum LoginEvent {
    SyncRequested,
    /// Unlock attempt completed successfully — App should flip to the vault
    /// screen and kick off the vault list load for this user.
    #[from(skip)]
    Unlocked {
        uid: UserId,
    },
    /// Email + password login completed.
    #[from(skip)]
    LoggedIn {
        uid: UserId,
    },
    /// Account-switcher action. Forwarded verbatim to
    /// `App::handle_account_switcher_event` so login, vault, and send share
    /// one dispatch site.
    AccountSwitcher(AccountSwitcherEvent),
}

pub struct LoginView {
    flow: flow::Flow,
    pub(in crate::views::login) auth_page: AuthPage,
    /// True between `LoginMessage::Unlock` firing and `UnlockCompleted` arriving.
    /// Drives the in-progress spinner on the unlock screen and gates re-entry.
    pub(super) unlock_in_progress: bool,
    /// Other unlock methods the active user has configured, minus the currently
    /// selected one. Populated only while `auth_page` is `Unlock`; cleared on
    /// every transition to `LoginEmail` / `LoginPassword`. Lives here instead
    /// of on `App` because every input (active user, `auth_page`, method
    /// choice) is already owned or observed by this view.
    pub(super) unlock_alternatives: Vec<UnlockMethod>,
    /// Working URL + animation state for the self-hosted environment modal.
    /// `fade.is_open()` is the source of truth for "modal logically open".
    pub(in crate::views::login) self_hosted_modal: SelfHostedModal,
}

#[derive(Default)]
pub(in crate::views::login) struct SelfHostedModal {
    pub(super) fade: FadeInOut,
    pub(super) url_input: String,
    /// Set on Save when validation fails. Cleared on every keystroke so the
    /// inline error disappears as soon as the user starts correcting.
    pub(super) url_error: bool,
}

impl LoginView {
    pub fn new() -> Self {
        Self {
            flow: flow::Flow::default(),
            auth_page: AuthPage::new_unlock(UnlockMethod::MasterPassword),
            unlock_in_progress: false,
            unlock_alternatives: Vec::new(),
            self_hosted_modal: SelfHostedModal::default(),
        }
    }

    /// Recompute the alternatives list from the current `auth_page` + active
    /// user's configured methods. Empty for non-`Unlock` pages and for
    /// unknown users.
    fn refresh_unlock_alternatives(&mut self, active_user: Option<&UserId>, mgr: &ClientManager) {
        self.unlock_alternatives = if let AuthPage::Unlock { method, .. } = self.auth_page {
            active_user
                .and_then(|uid| mgr.unlock_methods(uid))
                .map(|m| m.alternatives(method))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
    }
}

impl View for LoginView {
    type Message = LoginMessage;
    type Event = LoginEvent;

    fn update(&mut self, msg: LoginMessage, ctx: UpdateCtx<'_>) -> Outcome<Self> {
        let UpdateCtx {
            client_manager,
            active_user,
            open_overlay,
            ..
        } = ctx;
        if let Some(outcome) = self.flow_update(&msg, client_manager, active_user.copied()) {
            return outcome;
        }
        match msg {
            LoginMessage::Flow(_) => return Outcome::None,
            LoginMessage::PasswordChanged(pw) => {
                if let AuthPage::Unlock { password_input, .. } = &mut self.auth_page {
                    *password_input = pw;
                }
            }
            LoginMessage::Unlock => {
                // Re-entry guard: ignore Enter-spam while a task is already
                // in flight (the text_input's on_submit fires per keystroke).
                if self.unlock_in_progress {
                    return Outcome::None;
                }
                let password = if let AuthPage::Unlock { password_input, .. } = &mut self.auth_page
                {
                    std::mem::take(password_input)
                } else {
                    return Outcome::None;
                };
                let Some(uid) = active_user.cloned() else {
                    return Outcome::None;
                };
                let Some(data) = client_manager.unlock_data_for(&uid) else {
                    return Outcome::None;
                };
                self.unlock_in_progress = true;
                return Outcome::perform(async move { data.unlock(password).await }, move |res| {
                    LoginMessage::UnlockCompleted(uid, res)
                });
            }
            LoginMessage::UnlockCompleted(msg_uid, result) => {
                self.unlock_in_progress = false;
                // Stale-check: the user may have switched accounts while the
                // unlock was in flight. Drop stale results so they don't
                // clobber a new active session.
                if active_user != Some(&msg_uid) {
                    tracing::debug!(
                        uid = %msg_uid,
                        "unlock result dropped: active user changed while in flight"
                    );
                    return Outcome::None;
                }
                return match result {
                    Ok(()) => Outcome::event(LoginEvent::Unlocked { uid: msg_uid }),
                    Err(err) => {
                        // Log the raw SDK error for debugging, but show a
                        // sanitized message to the user — SDK `Display`
                        // impls can surface internal crypto state / module
                        // paths that shouldn't land in a toast notification.
                        tracing::warn!(
                            uid = %msg_uid,
                            %err,
                            "SDK initialize_user_crypto failed"
                        );
                        Outcome::toast(Toast::error(
                            crate::fl!("login-toast-unlock-failed-body"),
                            Some(&crate::fl!("login-toast-unlock-failed-title")),
                        ))
                    }
                };
            }

            LoginMessage::PinChanged(pin) => {
                if let AuthPage::Unlock { pin_input, .. } = &mut self.auth_page {
                    *pin_input = pin;
                }
            }
            LoginMessage::UnlockWithPin => {
                if let AuthPage::Unlock { pin_input, .. } = &mut self.auth_page {
                    pin_input.clear();
                }
                return Outcome::toast(Toast::warning(
                    crate::fl!("login-toast-pin-unsupported"),
                    None,
                ));
            }

            LoginMessage::UnlockWithBiometrics => {
                return Outcome::toast(Toast::warning(
                    crate::fl!("login-toast-biometrics-unsupported"),
                    None,
                ));
            }

            LoginMessage::SwitchUnlockMethod(method) => {
                self.auth_page = AuthPage::new_unlock(method);
                self.unlock_in_progress = false;
                self.refresh_unlock_alternatives(active_user, client_manager);
                return Outcome::task(self.auto_focus_task());
            }

            LoginMessage::EmailChanged(email) => {
                if let AuthPage::LoginEmail { email_input, .. } = &mut self.auth_page {
                    *email_input = email;
                }
            }
            LoginMessage::ToggleRememberEmail(checked) => {
                if let AuthPage::LoginEmail { remember_email, .. } = &mut self.auth_page {
                    *remember_email = checked;
                }
            }
            LoginMessage::ContinueWithEmail => {
                if let AuthPage::LoginEmail {
                    email_input,
                    selected_server,
                    ..
                } = &mut self.auth_page
                {
                    let email = email_input.clone();
                    let server = selected_server.clone();
                    self.auth_page = AuthPage::new_login_password(email, server);
                    self.unlock_alternatives.clear();
                    return Outcome::task(self.auto_focus_task());
                }
            }
            LoginMessage::UseSingleSignOn => {
                // Stub. See `SSO login flow` in docs/todo.md.
            }

            LoginMessage::LoginPasswordChanged(pw) => {
                if let AuthPage::LoginPassword { password_input, .. } = &mut self.auth_page {
                    *password_input = pw;
                }
            }
            LoginMessage::LoginWithPassword => {
                return self.start_login(client_manager);
            }
            LoginMessage::LoginCompleted(msg_uid, result) => {
                if active_user != Some(&msg_uid) {
                    tracing::debug!(
                        uid = %msg_uid,
                        "login result dropped: active user changed while in flight"
                    );
                    return Outcome::None;
                }
                return match result {
                    Ok(()) => Outcome::event(LoginEvent::LoggedIn { uid: msg_uid }),
                    Err(err) => {
                        // Log the raw SDK error for debugging, but show a
                        // sanitized message to the user — see the matching
                        // treatment in `UnlockCompleted`.
                        tracing::warn!(uid = %msg_uid, %err, "SDK login failed");
                        Outcome::toast(Toast::error(
                            crate::fl!("login-toast-login-failed-body"),
                            Some(&crate::fl!("login-toast-login-failed-title")),
                        ))
                    }
                };
            }
            LoginMessage::BackToEmail => {
                self.cancel_login();
                // Preserve the email + selected server when going back so
                // the user doesn't have to retype what they just entered.
                let (email, server) = match &self.auth_page {
                    AuthPage::LoginPassword {
                        email,
                        selected_server,
                        ..
                    } => (email.clone(), selected_server.clone()),
                    _ => (String::new(), ServerOption::Bitwarden),
                };
                self.auth_page = AuthPage::LoginEmail {
                    email_input: email,
                    remember_email: false,
                    selected_server: server,
                };
                self.unlock_alternatives.clear();
                return Outcome::task(self.auto_focus_task());
            }
            LoginMessage::GetPasswordHint => {
                // Stub. See `Master-password hint request` in docs/todo.md.
            }

            LoginMessage::ToggleServerSelector => {
                *open_overlay = if *open_overlay == Some(Overlay::ServerSelector) {
                    None
                } else {
                    Some(Overlay::ServerSelector)
                };
            }
            LoginMessage::SelectServer(server) => {
                self.cancel_login();
                *open_overlay = None;
                if matches!(server, ServerOption::SelfHosted(_)) {
                    if let Err(error) = self.open_custom_settings(client_manager) {
                        return Outcome::toast(Toast::error(error, None));
                    }
                    return Outcome::task(crate::components::fade_in_out::focus_after_open(
                        SELF_HOSTED_URL_FIELD_ID,
                    ));
                }
                if let Err(error) = self.select_server(server, client_manager) {
                    return Outcome::toast(Toast::error(error, None));
                }
            }

            LoginMessage::SelfHostedUrlChanged(url) => {
                self.self_hosted_modal.url_input = url;
                self.self_hosted_modal.url_error = false;
            }
            LoginMessage::SelfHostedSave => return self.save_server_dialog(client_manager),
            LoginMessage::SelfHostedCancel => {
                self.self_hosted_modal.fade.close();
                self.flow.certificate_password.clear();
            }

            LoginMessage::AccountSwitcher(m) => {
                return m.route(open_overlay, Overlay::AccountSwitcher);
            }
        }
        Outcome::None
    }

    fn view<'a>(&'a self, ctx: &RenderCtx<'a>) -> Element<'a, LoginMessage, AppTheme> {
        if let Some(content) = self.flow_view(ctx) {
            return content;
        }
        let (center_content, status_bar) = match &self.auth_page {
            AuthPage::Unlock {
                method,
                password_input,
                pin_input,
            } => {
                let center = unlock::view(
                    *method,
                    &self.unlock_alternatives,
                    ctx.active_email,
                    password_input,
                    pin_input,
                    self.unlock_in_progress,
                    ctx.colors,
                );
                let status = server_selector::simple_status(ctx.active_server_url, ctx.colors);
                (center, status)
            }
            AuthPage::LoginEmail {
                email_input,
                remember_email,
                selected_server,
            } => {
                let server_selector_open = ctx.open_overlay == Some(Overlay::ServerSelector);
                let center = login_email::view(email_input, *remember_email, ctx.colors);
                let status =
                    server_selector::view(selected_server, server_selector_open, ctx.colors);
                (center, status)
            }
            AuthPage::LoginPassword {
                email,
                password_input,
                selected_server,
            } => {
                let center = login_password::view(email, password_input, ctx.colors);
                let status =
                    server_selector::simple_status(&selected_server.display_name(), ctx.colors);
                (center, status)
            }
        };

        let account_switcher_open = ctx.open_overlay == Some(Overlay::AccountSwitcher);
        let center_content = iced::widget::column![
            center_content,
            crate::components::buttons::secondary("Server / mTLS-Einstellungen")
                .on_press(LoginMessage::Flow(flow::FlowMessage::Settings))
        ]
        .spacing(16)
        .align_x(iced::Alignment::Center)
        .into();
        layout::auth_page_shell(
            center_content,
            status_bar,
            ctx.active_email,
            ctx.accounts,
            account_switcher_open,
            ctx.colors,
        )
    }

    /// Self-hosted server URL modal — pushed onto the overlay stack only
    /// when its fade is visible.
    fn overlays<'a>(&'a self, ctx: &RenderCtx<'a>) -> Vec<Element<'a, LoginMessage, AppTheme>> {
        self_hosted_modal::view(
            &self.self_hosted_modal,
            self.certificate_controls(),
            !self.flow.settings_busy,
            ctx.colors,
        )
        .into_iter()
        .collect()
    }
}

impl LoginView {
    /// Reset the login flow to the email-entry page with empty inputs.
    pub fn reset_to_email_entry(&mut self, mgr: &ClientManager) {
        self.cancel_login();
        self.auth_page = AuthPage::new_login_email();
        self.unlock_in_progress = false;
        self.unlock_alternatives.clear();
        self.self_hosted_modal = SelfHostedModal::default();
        self.restore_server(mgr);
    }

    /// Show the unlock page for the given user, picking their preferred
    /// unlock method. Without a known account, show email entry instead of
    /// an unlock prompt for a nonexistent vault.
    /// Called from the post-load transition, user-switch, and lock-all flows.
    pub fn show_unlock_for(&mut self, uid: Option<&UserId>, mgr: &ClientManager) {
        self.cancel_login();
        let Some(methods) = uid.and_then(|uid| mgr.unlock_methods(uid)) else {
            self.reset_to_email_entry(mgr);
            return;
        };
        let preferred = methods.preferred();
        self.auth_page = AuthPage::new_unlock(preferred);
        self.unlock_in_progress = false;
        self.refresh_unlock_alternatives(uid, mgr);
    }

    /// Focus task for whichever input belongs to the currently-shown auth
    /// page so the user can start typing as soon as the page appears. Returns
    /// `Task::none()` for unlock methods that have no text input (Biometrics).
    /// Callers should fire this whenever they transition into / between login
    /// pages (App-side after `show_unlock_for` / `reset_to_email_entry`, and
    /// inside the in-page transition arms of `update`).
    pub fn auto_focus_task(&self) -> Task<LoginMessage> {
        match &self.auth_page {
            AuthPage::LoginEmail { .. } => iced::widget::operation::focus(LOGIN_EMAIL_FIELD_ID),
            AuthPage::LoginPassword { .. } => {
                iced::widget::operation::focus(LOGIN_PASSWORD_FIELD_ID)
            }
            AuthPage::Unlock {
                method: UnlockMethod::MasterPassword | UnlockMethod::Pin,
                ..
            } => iced::widget::operation::focus(UNLOCK_FIELD_ID),
            AuthPage::Unlock {
                method: UnlockMethod::Biometrics,
                ..
            } => Task::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SelfHostedUrl, validate_self_hosted_url};

    #[test]
    fn absent_accounts_show_email_entry_and_discard_old_unlock_fields() {
        let manager = crate::services::sdk::ClientManager::empty();
        let unknown = "00000000-0000-0000-0000-000000000001".parse().unwrap();
        for uid in [None, Some(&unknown)] {
            let mut view = super::LoginView::new();
            view.auth_page = super::AuthPage::Unlock {
                method: crate::domain::UnlockMethod::MasterPassword,
                password_input: "synthetic secret".into(),
                pin_input: String::new(),
            };
            view.show_unlock_for(uid, &manager);
            assert!(
                matches!(view.auth_page, super::AuthPage::LoginEmail { ref email_input, .. } if email_input.is_empty())
            );
            assert!(!view.unlock_in_progress);
        }
    }

    #[test]
    fn empty_input_is_no_op() {
        assert_eq!(validate_self_hosted_url(""), SelfHostedUrl::Empty);
    }

    #[test]
    fn whitespace_only_is_no_op() {
        // Trim before checking — the user pressing Submit on a blank
        // (or accidentally-spaces) field shouldn't surface an error.
        assert_eq!(validate_self_hosted_url("   "), SelfHostedUrl::Empty);
        assert_eq!(validate_self_hosted_url("\t\n"), SelfHostedUrl::Empty);
    }

    #[test]
    fn https_url_is_accepted() {
        assert_eq!(
            validate_self_hosted_url("https://vault.example.com"),
            SelfHostedUrl::Accepted("https://vault.example.com".into())
        );
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        // Common case: user pastes a URL with a trailing newline.
        assert_eq!(
            validate_self_hosted_url("  https://vault.example.com  \n"),
            SelfHostedUrl::Accepted("https://vault.example.com".into())
        );
    }

    #[test]
    fn http_is_rejected() {
        // The whole point of the gate — http would let the SDK send
        // master-password-derived auth headers in the clear.
        assert_eq!(
            validate_self_hosted_url("http://vault.example.com"),
            SelfHostedUrl::Invalid
        );
    }

    #[test]
    fn bare_host_is_rejected() {
        // No scheme at all → not auto-promoted to https; validation is
        // strict because the user will see this exact string back.
        assert_eq!(
            validate_self_hosted_url("vault.example.com"),
            SelfHostedUrl::Invalid
        );
    }

    #[test]
    fn dangerous_schemes_are_rejected() {
        // Defense in depth — even though the SDK would error, the modal
        // shouldn't store hostile URIs as the active server.
        assert_eq!(
            validate_self_hosted_url("javascript:alert(1)"),
            SelfHostedUrl::Invalid
        );
        assert_eq!(
            validate_self_hosted_url("file:///etc/passwd"),
            SelfHostedUrl::Invalid
        );
        assert_eq!(
            validate_self_hosted_url("ftp://vault.example.com"),
            SelfHostedUrl::Invalid
        );
    }

    #[test]
    fn https_check_is_case_sensitive() {
        // `starts_with` is case-sensitive. `HTTPS://` is rejected — the
        // browser will normalize it but our validation doesn't, and we
        // store the user's exact string. Case-pedantic but durable.
        assert_eq!(
            validate_self_hosted_url("HTTPS://vault.example.com"),
            SelfHostedUrl::Invalid
        );
    }
}

#[cfg(test)]
mod tests_update {
    use super::*;
    use crate::{app::App, test_support::OutcomeExt};

    fn unlocking_view() -> LoginView {
        let mut view = LoginView::new();
        // Simulate the mid-flight state Unlock leaves behind: spinner on,
        // password field cleared.
        view.unlock_in_progress = true;
        view
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unlock_completed_resets_spinner_even_on_stale_result() {
        // The spinner reset runs BEFORE the stale-uid check intentionally:
        // when the user switched accounts mid-unlock, the spinner on the
        // (now-hidden) original login screen still needs to stop so it
        // doesn't show a stuck state if the user switches back.
        let active = UserId::new_v4();
        let stale = UserId::new_v4();

        let mut app = App::test();
        app.active_user = Some(active);

        let mut view = unlocking_view();
        let outcome = view.update(
            LoginMessage::UnlockCompleted(stale, Ok(())),
            app.update_ctx(),
        );
        outcome.expect_none();
        assert!(!view.unlock_in_progress, "spinner cleared on stale result");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unlock_completed_ok_emits_unlocked_event_for_active_user() {
        let uid = UserId::new_v4();
        let mut app = App::test();
        app.active_user = Some(uid);

        let mut view = unlocking_view();
        let ev = view
            .update(LoginMessage::UnlockCompleted(uid, Ok(())), app.update_ctx())
            .expect_event();
        match ev {
            LoginEvent::Unlocked { uid: emitted } => assert_eq!(emitted, uid),
            _ => panic!("expected Unlocked event"),
        }
        assert!(!view.unlock_in_progress);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unlock_completed_err_emits_toast_for_active_user() {
        // Failure path: spinner clears, sanitized error toast surfaces.
        // The raw SDK error string is logged at WARN but not exposed to
        // the user (see the inline comment in `UnlockCompleted`).
        let uid = UserId::new_v4();
        let mut app = App::test();
        app.active_user = Some(uid);

        let mut view = unlocking_view();
        let toast = view
            .update(
                LoginMessage::UnlockCompleted(uid, Err("raw crypto error".to_string())),
                app.update_ctx(),
            )
            .expect_toast();
        assert!(matches!(
            toast.status,
            crate::components::toast::ToastStatus::Error
        ));
        assert!(!view.unlock_in_progress);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unlock_reentry_during_in_flight_is_dropped() {
        // Re-entry guard: text_input's `on_submit` fires per Enter keypress;
        // mashing Enter while the SDK call is already running shouldn't
        // queue duplicate unlock attempts.
        let uid = UserId::new_v4();
        let mut app = App::test();
        app.active_user = Some(uid);

        let mut view = unlocking_view();
        view.update(LoginMessage::Unlock, app.update_ctx())
            .expect_none();
        assert!(
            view.unlock_in_progress,
            "still in flight, no new task spawned"
        );
    }
}

impl std::fmt::Debug for LoginMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LoginMessage(<redacted>)")
    }
}
