use super::*;
impl Core {
    pub fn pick_export_path(
        &mut self,
        organization: Option<bitwarden_core::OrganizationId>,
        format: bitwarden_exporters::ExportFormat,
    ) {
        let Some(scope) = self.scope() else {
            return;
        };
        if self.remote() || self.export_request.is_some() || self.prompt.is_some() {
            return;
        }
        let extension = if matches!(format, bitwarden_exporters::ExportFormat::Csv) {
            "csv"
        } else {
            "json"
        };
        let request = uuid::Uuid::new_v4();
        self.export_request = Some(request);
        self.spawn_scoped(
            scope,
            async move {
                rfd::AsyncFileDialog::new()
                    .set_title("Export vault")
                    .set_file_name(format!("bitwarden-export.{extension}"))
                    .add_filter("Vault export", &[extension])
                    .save_file()
                    .await
            },
            move |core, path| {
                if core.export_request != Some(request) {
                    return;
                }
                core.export_request = None;
                if !core.export_open {
                    return;
                }
                if let Some(path) = path {
                    core.request(Request::Export {
                        organization,
                        format,
                        path: path.path().into(),
                    });
                }
            },
        );
    }
}
