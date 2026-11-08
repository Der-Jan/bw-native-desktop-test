//! Export vault modal — wired to the sidebar Export button.
//!
//! Two stacked dialogs: the format/password picker (Compose), and a
//! master-password "Confirm vault export" gate that opens on top once the
//! user clicks Continue. Validation goes through the SDK
//! (`ClientManager::validate_master_password`); on success App is asked to
//! pop a native save dialog (rfd) and write the export to the chosen path.
//!
//! Format options mirror [`bitwarden_exporters::ExportFormat`] (Csv, Json,
//! EncryptedJson). Selecting `EncryptedJson` reveals a file-password input
//! below the picker; the encryption password is folded into the SDK-shaped
//! format value at the moment Continue is pressed.

use bitwarden_core::OrganizationId;
use iced::{
    Alignment, Element, Fill,
    widget::{self, column, container, row, text},
};

use crate::{
    app::{Outcome, RenderCtx, UpdateCtx, View},
    components::{FadeInOut, fade_in_out, icons, inputs, modal, toast::Toast},
    domain::UserId,
    fl,
    services::sdk::{Organization, VaultChoice},
    theme::{AppColors, AppTheme, RADIUS_LG},
};

pub const CONFIRM_PASSWORD_FIELD_ID: widget::Id = widget::Id::new("export-confirm-password-field");

// ── Export format catalog ─────────────────────────────────────────────────
//
// Mirrors the variants of [`bitwarden_exporters::ExportFormat`]. The SDK
// enum carries `password: String` inside `EncryptedJson`, but the picker
// only needs identity (which variant) — the password is collected from a
// separate input. `to_sdk` reassembles the SDK-shaped value at submit time.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormatChoice {
    Json,
    Csv,
    EncryptedJson,
}

impl ExportFormatChoice {
    const ALL: &'static [Self] = &[Self::Json, Self::Csv, Self::EncryptedJson];

    fn label(self) -> &'static str {
        match self {
            Self::Json => ".json",
            Self::Csv => ".csv",
            Self::EncryptedJson => ".json (Encrypted)",
        }
    }

    fn to_sdk(self, password: String) -> bitwarden_exporters::ExportFormat {
        match self {
            Self::Json => bitwarden_exporters::ExportFormat::Json,
            Self::Csv => bitwarden_exporters::ExportFormat::Csv,
            Self::EncryptedJson => bitwarden_exporters::ExportFormat::EncryptedJson { password },
        }
    }

    #[expect(
        dead_code,
        reason = "we keep this to ensure a compilation error when new SDK formats are added"
    )]
    fn from_sdk(format: &bitwarden_exporters::ExportFormat) -> Self {
        match format {
            bitwarden_exporters::ExportFormat::Json => Self::Json,
            bitwarden_exporters::ExportFormat::Csv => Self::Csv,
            bitwarden_exporters::ExportFormat::EncryptedJson { .. } => Self::EncryptedJson,
        }
    }
}

impl std::fmt::Display for ExportFormatChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

// ── State ─────────────────────────────────────────────────────────────────

pub struct ExportView {
    /// Outer fade — owns the Compose dialog (format picker).
    pub(super) fade: FadeInOut,
    /// Inner fade — owns the Confirm dialog (master password). Opens only
    /// while the user is on the master-password gate.
    pub(super) confirm_fade: FadeInOut,
    /// Active user email — shown verbatim in the personal-vault banner.
    /// Set via [`Self::open`] each time the modal is opened.
    email: String,
    /// Personal + per-org options for the source-vault dropdown. Seeded
    /// synchronously from the active user's cached org list on open.
    vault_choices: Vec<VaultChoice>,
    selected_vault: VaultChoice,
    selected_format: ExportFormatChoice,
    /// Encryption password for the encrypted-json variant. Cleared on open
    /// and on successful submit. Only read when
    /// `selected_format == EncryptedJson`.
    file_password: String,
    /// Master-password input on the Confirm dialog. Cleared on open and
    /// every time the Confirm dialog re-opens.
    master_password: String,
    /// Disables the Confirm dialog's Continue button while the SDK
    /// validation call is in flight.
    validating: bool,
    validation_id: uuid::Uuid,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Clone)]
pub enum ExportMessage {
    Close,
    /// Continue on the Compose dialog → open the master-password gate.
    Continue,
    VaultSelected(VaultChoice),
    FormatSelected(ExportFormatChoice),
    FilePasswordChanged(String),
    MasterPasswordChanged(String),
    /// Continue on the Confirm dialog → kick off SDK validation.
    ConfirmSubmit,
    /// Cancel / X / backdrop on the Confirm dialog → close it but keep the
    /// Compose dialog open so the user can adjust and retry.
    BackToCompose,
    /// SDK validation result. `Ok(uid)` proceeds to the save-as picker via
    /// [`ExportEvent::PickPathThenRun`] — the view rebuilds the SDK format
    /// from `self.selected_format` + `self.file_password` at that moment
    /// (the SDK's `ExportFormat` doesn't derive `Clone` / `Debug`, so we
    /// can't carry it inside an `ExportMessage`). The validated `uid` is
    /// echoed back so it flows through `PickPathThenRun` unchanged — an
    /// active-user switch in flight can't redirect the export to the
    /// wrong account. `Err` re-enables the Confirm button and surfaces
    /// an "Invalid master password" toast.
    ValidationCompleted(uuid::Uuid, Result<UserId, String>),
    /// Result of the async SDK export — `Ok(Some(path))` is the file the
    /// App handler wrote the export to, `Ok(None)` means the user cancelled
    /// the native save dialog (silent close), `Err` is a human-readable
    /// error.
    Completed(Result<Option<String>, String>),
}

pub enum ExportEvent {
    /// Master password verified. App pops a native save dialog (rfd) for
    /// the user to pick a destination, then runs the SDK export with the
    /// already-folded format value. `uid` is captured at validation time
    /// so that an active-user switch between Confirm and the save dialog
    /// can't redirect the export to a different account's vault.
    /// `organization_id` is `Some` when the user picked an org from the
    /// source-vault dropdown — the App handler routes it to
    /// [`ClientManager::export_organization_vault`] instead of the
    /// personal exporter.
    PickPathThenRun {
        uid: UserId,
        organization_id: Option<OrganizationId>,
        format: bitwarden_exporters::ExportFormat,
    },
}

impl View for ExportView {
    type Message = ExportMessage;
    type Event = ExportEvent;

    fn update(&mut self, msg: ExportMessage, ctx: UpdateCtx<'_>) -> Outcome<Self> {
        if let ExportMessage::ValidationCompleted(id, _) = &msg
            && *id != self.validation_id
        {
            return Outcome::None;
        }
        match msg {
            ExportMessage::Close => {
                ctx.client_manager.clear_grants();
                self.close_all();
                Outcome::None
            }
            ExportMessage::VaultSelected(v) => {
                self.selected_vault = v;
                Outcome::None
            }
            ExportMessage::FormatSelected(f) => {
                self.selected_format = f;
                if !matches!(f, ExportFormatChoice::EncryptedJson) {
                    self.file_password.clear();
                }
                Outcome::None
            }
            ExportMessage::FilePasswordChanged(p) => {
                self.file_password = p;
                Outcome::None
            }
            ExportMessage::Continue => {
                self.master_password.clear();
                self.validating = false;
                self.confirm_fade.open();
                Outcome::task(fade_in_out::focus_after_open(CONFIRM_PASSWORD_FIELD_ID))
            }
            ExportMessage::MasterPasswordChanged(p) => {
                self.master_password = p;
                Outcome::None
            }
            ExportMessage::BackToCompose => {
                self.cancel_authorization();
                ctx.client_manager.clear_grants();
                self.confirm_fade.close();
                self.master_password.clear();
                self.validating = false;
                Outcome::None
            }
            ExportMessage::ConfirmSubmit => {
                if self.master_password.is_empty() || self.validating {
                    return Outcome::None;
                }
                let Some(uid) = ctx.active_user.copied() else {
                    return Outcome::None;
                };
                self.validating = true;
                self.cancel_authorization();
                self.cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let cancelled = self.cancelled.clone();
                let request = self.validation_id;
                let password = std::mem::take(&mut self.master_password);
                let Some((client, key)) = ctx.client_manager.validation_data_for(&uid) else {
                    return Outcome::None;
                };
                Outcome::perform(
                    async move {
                        client
                            .authorize_password_scoped(
                                key,
                                password,
                                crate::services::sdk::Authorization::export(),
                                cancelled,
                            )
                            .await
                            .map(|()| uid)
                    },
                    move |r| ExportMessage::ValidationCompleted(request, r),
                )
            }
            ExportMessage::ValidationCompleted(_, Ok(uid)) => {
                self.validating = false;
                self.master_password.clear();
                // The validation task can outlive a user-initiated close
                // (outer X / backdrop). Drop the result on the floor in
                // that case rather than popping a save dialog from
                // nowhere.
                if !self.fade.is_open() {
                    return Outcome::None;
                }
                let format = self.selected_format.to_sdk(self.file_password.clone());
                let organization_id = match &self.selected_vault {
                    VaultChoice::Personal => None,
                    VaultChoice::Org { id, .. } => Some(*id),
                };
                Outcome::event(ExportEvent::PickPathThenRun {
                    uid,
                    organization_id,
                    format,
                })
            }
            ExportMessage::ValidationCompleted(_, Err(_)) => {
                self.validating = false;
                Outcome::toast(Toast::error(fl!("export-confirm-error"), None))
            }
            ExportMessage::Completed(Ok(Some(path))) => {
                self.file_password.clear();
                self.close_all();
                Outcome::toast(Toast::success(
                    fl!("export-toast-success", path = path.as_str()),
                    None,
                ))
            }
            ExportMessage::Completed(Ok(None)) => {
                ctx.client_manager.clear_grants();
                // User cancelled the native save dialog — close the modals
                // silently with no toast.
                self.close_all();
                Outcome::None
            }
            ExportMessage::Completed(Err(err)) => {
                // Leave the modals open so the user can correct & retry —
                // typical case is a wrong / missing encryption password.
                self.validating = false;
                tracing::error!(%err, "vault export failed");
                Outcome::toast(Toast::error(
                    fl!("export-toast-failed-body"),
                    Some(&fl!("export-toast-failed-title")),
                ))
            }
        }
    }

    fn should_render(&self) -> bool {
        self.fade.is_visible()
    }

    fn view<'a>(&'a self, ctx: &RenderCtx<'a>) -> Element<'a, ExportMessage, AppTheme> {
        self.compose_dialog(ctx.colors, self.fade.progress_when_visible())
    }

    /// Confirm-master-password dialog — sits on top of the compose dialog
    /// when the user has hit Continue.
    fn overlays<'a>(&'a self, ctx: &RenderCtx<'a>) -> Vec<Element<'a, ExportMessage, AppTheme>> {
        match self.confirm_fade.progress_if_visible() {
            Some(progress) => vec![self.confirm_dialog(ctx.colors, progress)],
            None => Vec::new(),
        }
    }
}

// ── Lifecycle ─────────────────────────────────────────────────────────────

impl ExportView {
    pub fn new() -> Self {
        Self {
            fade: FadeInOut::default(),
            confirm_fade: FadeInOut::default(),
            email: String::new(),
            vault_choices: vec![VaultChoice::Personal],
            selected_vault: VaultChoice::Personal,
            selected_format: ExportFormatChoice::Json,
            file_password: String::new(),
            master_password: String::new(),
            validating: false,
            validation_id: uuid::Uuid::new_v4(),
            cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn open(&mut self, email: String) {
        self.cancel_authorization();
        self.fade.open();
        self.confirm_fade.close();
        self.email = email;
        self.selected_vault = VaultChoice::Personal;
        self.selected_format = ExportFormatChoice::Json;
        self.file_password.clear();
        self.master_password.clear();
        self.validating = false;
    }

    /// Replace the source-vault dropdown options with "My vault" + the
    /// active user's org list. Called by App on each open from the cached
    /// org snapshot held by `VaultView` — same pattern as the import modal.
    pub fn set_organizations(&mut self, orgs: &[Organization]) {
        self.vault_choices = VaultChoice::list_with_personal(orgs);
    }

    fn cancel_authorization(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.validation_id = uuid::Uuid::new_v4();
    }
    fn close_all(&mut self) {
        self.cancel_authorization();
        self.fade.close();
        self.confirm_fade.close();
        self.master_password.clear();
        self.validating = false;
    }

    fn compose_dialog<'a>(
        &'a self,
        colors: &'a AppColors,
        progress: f32,
    ) -> Element<'a, ExportMessage, AppTheme> {
        let header = modal::dialog_header(fl!("export-modal-title"), ExportMessage::Close, colors);

        let banner = self.banner(colors);

        // Both pickers sit directly on the dialog body (which is painted
        // with `card_bg`), so the floating label chip needs to match —
        // otherwise it shows as a contrasting tile on the border.
        let personal_label = fl!("export-modal-vault-personal");
        let vault_picker = inputs::select_field_on(
            fl!("export-modal-vault-label"),
            Some(self.selected_vault.clone()),
            self.vault_choices.clone(),
            move |v: &VaultChoice| v.label(&personal_label),
            ExportMessage::VaultSelected,
            |c| c.card_bg,
            colors,
        );

        let format_picker = inputs::select_field_on(
            fl!("export-modal-file-format-label"),
            Some(self.selected_format),
            ExportFormatChoice::ALL.to_vec(),
            |f: &ExportFormatChoice| f.label().to_string(),
            ExportMessage::FormatSelected,
            |c| c.card_bg,
            colors,
        );

        let mut body = column![header, banner, vault_picker, format_picker].spacing(16);

        if matches!(self.selected_format, ExportFormatChoice::EncryptedJson) {
            // Same chip-bg story as the format picker — sits directly on
            // the dialog `card_bg`. `bare_text_input` gives the standard
            // field look; `.secure(true)` masks the value.
            let password_input = inputs::bare_text_input(&self.file_password)
                .secure(true)
                .on_input(ExportMessage::FilePasswordChanged);
            body = body.push(inputs::field_frame_on(
                fl!("export-modal-password-label"),
                password_input,
                |c| c.card_bg,
                colors,
            ));
        }

        let continue_disabled = matches!(self.selected_format, ExportFormatChoice::EncryptedJson)
            && self.file_password.is_empty();

        let on_continue = (!continue_disabled).then_some(ExportMessage::Continue);
        let footer = modal::footer_actions(
            fl!("export-modal-continue"),
            on_continue,
            fl!("export-modal-cancel"),
            ExportMessage::Close,
        );

        let body = body.push(footer).padding(modal::BODY_PADDING).width(Fill);

        modal::dialog(
            460.0,
            None,
            |c| c.card_bg,
            progress,
            container(body),
            ExportMessage::Close,
        )
    }

    /// Info-blue banner above the pickers. Both arms share a bold heading
    /// + body shape; the copy swaps to name the active user's email or the chosen org.
    fn banner<'a>(&'a self, colors: &'a AppColors) -> Element<'a, ExportMessage, AppTheme> {
        let (title, body) = match &self.selected_vault {
            VaultChoice::Personal => (
                fl!("export-modal-banner-personal-title"),
                fl!("export-modal-banner-personal", email = self.email.as_str()),
            ),
            VaultChoice::Org { name, .. } => (
                fl!("export-modal-banner-org-title"),
                fl!("export-modal-banner-org-body", name = name.as_str()),
            ),
        };

        let copy = column![
            text(title)
                .size(crate::text_size(13))
                .font(crate::app_font_bold())
                .color(colors.text_primary),
            text(body)
                .size(crate::text_size(13))
                .color(colors.text_primary),
        ]
        .spacing(4);

        container(
            row![icons::INFO_CIRCLE.render(16.0, colors.accent), copy]
                .spacing(10)
                .align_y(Alignment::Start),
        )
        .padding(12)
        .width(Fill)
        .style(|theme: &AppTheme| {
            iced::widget::container::Style::default()
                .background(theme.colors.accent.scale_alpha(0.12))
                .border(iced::Border::default().rounded(RADIUS_LG))
        })
        .into()
    }

    fn confirm_dialog<'a>(
        &'a self,
        colors: &'a AppColors,
        progress: f32,
    ) -> Element<'a, ExportMessage, AppTheme> {
        let header = modal::dialog_header(
            fl!("export-confirm-title"),
            ExportMessage::BackToCompose,
            colors,
        );

        // `fl!` is a proc macro that requires a literal id, so the
        // format → key mapping has to live in match arms here. Mirrors
        // `verifyUser()` in the official client
        // (`clients/libs/tools/export/vault-export/vault-export-ui/src/components/export.component.ts`).
        // We don't expose an account-encrypted export today, so the
        // EncryptedJson variant always reads as file-encrypted.
        let warning_text = match self.selected_format {
            ExportFormatChoice::Json | ExportFormatChoice::Csv => {
                fl!("export-confirm-warning-unencrypted")
            }
            ExportFormatChoice::EncryptedJson => fl!("export-confirm-warning-file-encrypted"),
        };
        let warning = text(warning_text)
            .size(crate::text_size(14))
            .color(colors.text_primary);

        let mut password_input = inputs::bare_text_input(&self.master_password)
            .id(CONFIRM_PASSWORD_FIELD_ID)
            .secure(true)
            .on_input(ExportMessage::MasterPasswordChanged);
        if !self.master_password.is_empty() && !self.validating {
            password_input = password_input.on_submit(ExportMessage::ConfirmSubmit);
        }
        let password_field = inputs::field_frame_on(
            fl!("export-confirm-password-label"),
            password_input,
            |c| c.card_bg,
            colors,
        );

        let helper = text(fl!("export-confirm-helper"))
            .size(crate::text_size(12))
            .color(colors.text_secondary);

        let confirm_disabled = self.master_password.is_empty() || self.validating;
        let on_confirm = (!confirm_disabled).then_some(ExportMessage::ConfirmSubmit);
        let footer = modal::footer_actions(
            fl!("export-modal-continue"),
            on_confirm,
            fl!("export-modal-cancel"),
            ExportMessage::BackToCompose,
        );

        let body = column![header, warning, password_field, helper, footer]
            .spacing(16)
            .padding(modal::BODY_PADDING)
            .width(Fill);

        modal::dialog(
            460.0,
            None,
            |c| c.card_bg,
            progress,
            container(body),
            ExportMessage::BackToCompose,
        )
    }
}

impl std::fmt::Debug for ExportMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ExportMessage(<redacted>)")
    }
}

impl Drop for ExportView {
    fn drop(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests_update {
    use super::*;
    use crate::test_support::{OutcomeExt, ViewTestExt};

    fn primed_view() -> ExportView {
        let mut view = ExportView::new();
        view.open("test@example.com".to_string());
        view
    }

    #[tokio::test(flavor = "current_thread")]
    async fn format_switch_away_from_encrypted_json_clears_file_password() {
        // EncryptedJson is the only format that uses `file_password`. Any
        // other format MUST clear it so a stale password doesn't sneak
        // into a later switch-back-and-submit flow.
        let mut view = primed_view();
        view.selected_format = ExportFormatChoice::EncryptedJson;
        view.file_password = "secret".to_string();

        view.run(ExportMessage::FormatSelected(ExportFormatChoice::Json))
            .await
            .expect_none();
        assert!(view.file_password.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn format_switch_to_encrypted_json_preserves_file_password() {
        // The mirror case: switching INTO EncryptedJson must keep any
        // password the user already typed (clearing here would surprise
        // them after a brief detour to another format).
        let mut view = primed_view();
        view.selected_format = ExportFormatChoice::Json;
        view.file_password = "secret".to_string();

        view.run(ExportMessage::FormatSelected(
            ExportFormatChoice::EncryptedJson,
        ))
        .await
        .expect_none();
        assert_eq!(view.file_password, "secret");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn continue_opens_confirm_dialog_and_resets_state() {
        // Pressing Continue on the Compose dialog opens the
        // master-password gate. The master_password field must be empty
        // on each open — a stale value from a previous run would auto-
        // fill the gate with the previous attempt's input.
        let mut view = primed_view();
        view.master_password = "leftover".to_string();
        view.validating = true; // simulate a stale flag from a prior run

        let _ = view.run(ExportMessage::Continue).await;
        assert!(view.confirm_fade.is_open());
        assert!(view.master_password.is_empty());
        assert!(!view.validating);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn confirm_submit_with_empty_password_is_dropped() {
        // Defensive: the Continue button is disabled in the UI when
        // password is empty, but a stray keyboard-driven submit
        // shouldn't fire the SDK call either.
        let mut view = primed_view();
        view.confirm_fade.open();
        // master_password left empty
        view.run(ExportMessage::ConfirmSubmit).await.expect_none();
        assert!(!view.validating);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn confirm_submit_while_validating_is_dropped() {
        // Re-entry guard: validation is in flight, don't queue another.
        let mut view = primed_view();
        view.confirm_fade.open();
        view.master_password = "pw".to_string();
        view.validating = true;
        view.run(ExportMessage::ConfirmSubmit).await.expect_none();
        assert!(view.validating, "still validating, no new task spawned");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn validation_failure_clears_validating_and_emits_error_toast() {
        // Wrong-password path: reset the flag, surface an error toast,
        // KEEP the confirm dialog open so the user can retry.
        let mut view = primed_view();
        view.confirm_fade.open();
        view.master_password = "wrong".to_string();
        view.validating = true;

        let toast = view
            .run(ExportMessage::ValidationCompleted(
                view.validation_id,
                Err("invalid".into()),
            ))
            .await
            .expect_toast();
        assert!(matches!(
            toast.status,
            crate::components::toast::ToastStatus::Error
        ));
        assert!(!view.validating);
        assert!(view.confirm_fade.is_open(), "dialog stays open for retry");
    }
}
