use super::*;
use crate::core::merge::Phase;
impl Application {
    pub(super) fn merge_dialog(&mut self, ctx: &egui::Context) {
        if self.core.prompt.is_some() {
            return;
        }
        let Some(merge) = self.core.merge.as_ref() else {
            return;
        };
        let mut cancel = false;
        let mut confirm = false;
        let mut change_primary = None;
        let response = egui::Modal::new(Id::unique(("merge-duplicates",merge.request))).show(ctx,|ui| {
            ui.set_width(520.);
            ui.heading("Merge duplicate logins");
            ui.label(format!("Keep one primary entry and move {} secondary entries to trash after saving.",merge.ids.len()-1));
            if merge.phase==Phase::Review {
                ui.label("Choose the primary (its username, password and URL rules stay unchanged):");
                egui::ScrollArea::vertical().id_salt("merge-primary-choice").max_height(240.).show(ui, |ui| {
                for (index,view) in merge.loaded.iter().enumerate() {
                    let name=view.data.get("name").and_then(|v|v.as_str()).unwrap_or("Unnamed login");
                    let username=view.data.pointer("/login/username").and_then(|v|v.as_str()).unwrap_or("");
                    if ui.radio(index==0,format!("{name} — {username}")).clicked() && index!=0 {change_primary=Some(index);}
                }
                });
                let count=merge.preview.as_ref().and_then(|v|v.data.get("fields")).and_then(|v|v.as_array()).map_or(0,|v|v.len());
                ui.label(format!("The primary will contain {count} custom fields. Differing passwords and notes are stored as hidden custom fields. TOTP and passkeys are retained in their normal login fields; multiple source entries containing either kind block the merge."));
                ui.weak("The primary is saved first. If saving or synchronization fails, secondary entries are kept. After a partial trash failure, already trashed entries remain recoverable in Trash.");
                confirm=ui.button("Merge and move secondary entries to trash").clicked();
            } else if merge.phase==Phase::Stopped {
                ui.colored_label(ui.visuals().error_fg_color,merge.error.as_deref().unwrap_or("Merge stopped"));
            } else {
                ui.spinner();
                ui.label(match merge.phase {Phase::Loading=>"Loading selected entries…",Phase::Saving=>"Saving primary…",Phase::Uploading=>"Waiting for server confirmation…",_=>"Moving secondary entries to trash…"});
            }
            cancel=ui.button(if merge.phase==Phase::Stopped {"Close"} else {"Cancel remaining steps"}).clicked();
        });
        cancel |= response.should_close();
        if cancel {
            self.core.merge = None;
            self.core.cancel_prompt();
        } else if let Some(index) = change_primary {
            let mut ids = self.core.merge.take().unwrap().ids;
            ids.swap(0, index);
            self.core.begin_merge(ids);
        } else if confirm {
            self.core.confirm_merge();
        }
    }
}
