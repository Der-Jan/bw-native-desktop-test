use std::path::PathBuf;

use crate::services::sdk::SessionClient;
use bitwarden_core::OrganizationId;
use iced::Task;

use crate::{
    app::{App, Message},
    views::export::{ExportEvent, ExportMessage},
};

impl App {
    pub(crate) fn handle_export_event(&mut self, event: ExportEvent) -> Task<Message> {
        match event {
            // `uid` is captured from the validated user, not re-read from
            // `self.active_user` — guards against an account switch between
            // Confirm and the rfd dialog redirecting the export.
            ExportEvent::PickPathThenRun {
                uid,
                organization_id,
                format,
            } => {
                let extension = extension_for(&format);
                let default_name = format!(
                    "bitwarden-export-{}.{}",
                    chrono::Local::now().format("%Y%m%d-%H%M%S"),
                    extension,
                );
                self.perform_with_client(
                    uid,
                    move |client| {
                        pick_and_export(client, organization_id, format, extension, default_name)
                    },
                    |r| ExportMessage::Completed(r).into(),
                )
            }
        }
    }

    /// Open the Export modal, passing the active account's email so the
    /// personal-vault banner can name the user, and seeding the source-vault
    /// dropdown from the cached org snapshot held by `VaultView`.
    pub(crate) fn open_export_modal(&mut self) -> Task<Message> {
        let Some(uid) = self.require_active_user_and_close_overlay() else {
            return Task::none();
        };
        let email = self
            .active_account_entry()
            .map(|a| a.email.clone())
            .unwrap_or_default();
        self.views.export.open(email);

        let orgs = self
            .views
            .vault
            .organizations_for(&uid)
            .map(|s| s.to_vec())
            .unwrap_or_default();
        self.views.export.set_organizations(&orgs);
        Task::none()
    }
}

fn extension_for(format: &bitwarden_exporters::ExportFormat) -> &'static str {
    match format {
        bitwarden_exporters::ExportFormat::Csv => "csv",
        bitwarden_exporters::ExportFormat::Json
        | bitwarden_exporters::ExportFormat::EncryptedJson { .. } => "json",
    }
}

/// Open a native save dialog and run the export to the chosen path. Returns
/// `Ok(None)` if the user cancelled the dialog (silent close), `Ok(Some(path))`
/// after a successful write, or `Err(_)` on SDK / I/O failure.
///
/// `rfd::AsyncFileDialog` is the resolution to iced #1002 — the sync variant
/// blocks the iced event loop on Linux/Windows and deadlocks on macOS where
/// iced and rfd both want the main thread; the async one cooperates with our
/// `Task::perform` runtime.
async fn pick_and_export(
    client: SessionClient,
    organization_id: Option<OrganizationId>,
    format: bitwarden_exporters::ExportFormat,
    extension: &'static str,
    default_name: String,
) -> Result<Option<String>, String> {
    let Some(handle) = rfd::AsyncFileDialog::new()
        .set_file_name(&default_name)
        .add_filter(extension, &[extension])
        .save_file()
        .await
    else {
        return Ok(None);
    };
    let path: PathBuf = handle.path().to_path_buf();
    write_export(client, organization_id, format, path)
        .await
        .map(Some)
}

/// Call the SDK exporter and write the result to the user-chosen file.
/// The session lease covers SDK serialization and private staged publication.
async fn write_export(
    client: SessionClient,
    organization_id: Option<OrganizationId>,
    format: bitwarden_exporters::ExportFormat,
    path: PathBuf,
) -> Result<String, String> {
    client.export_to_path(organization_id, format, path).await
}
