use super::*;
use crate::{
    core::{model::Request, vault::kind},
    services::{
        vault_navigation::*,
        vault_sort::{SortColumn, SortDirection},
    },
};
use bitwarden_vault::{CipherListViewType, CipherType};
use egui::{Modifiers, Sense};
use egui_extras::{Column, TableBuilder};
const ROW_HEIGHT: f32 = 30.;

pub const TYPES: &[(CipherType, &str)] = &[
    (CipherType::Login, "Logins"),
    (CipherType::SecureNote, "Secure notes"),
    (CipherType::Card, "Cards"),
    (CipherType::Identity, "Identities"),
    (CipherType::SshKey, "SSH keys"),
    (CipherType::BankAccount, "Bank accounts"),
    (CipherType::DriversLicense, "Driver’s licenses"),
    (CipherType::Passport, "Passports"),
];
impl Application {
    pub(super) fn sidebar(&mut self, ui: &mut egui::Ui) {
        if self.core.settings.sidebar_collapsed {
            egui::Panel::left("collapsed-navigation")
                .exact_size(44.)
                .show(ui, |ui| {
                    if ui.button("»").clicked() {
                        self.core.settings.sidebar_collapsed = false;
                        self.core.save_settings();
                    }
                });
            return;
        }
        egui::Panel::left("navigation")
            .default_size(225.)
            .min_size(175.)
            .resizable(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Image::from_bytes(
                            "bytes://password-manager.svg",
                            crate::assets::PASSWORD_MANAGER_LOGO,
                        )
                        .tint(if ui.visuals().dark_mode {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_rgb(22, 92, 225)
                        })
                        .fit_to_exact_size(egui::vec2(170., 42.)),
                    );
                    if ui.small_button("«").clicked() {
                        self.core.settings.sidebar_collapsed = true;
                        self.core.save_settings();
                    }
                });
                let accounts = self.core.manager.accounts();
                let selected = accounts
                    .iter()
                    .find(|a| Some(a.user_id) == self.core.active)
                    .map(|a| a.email.clone())
                    .unwrap_or_default();
                egui::ComboBox::from_id_salt("account")
                    .width(ui.available_width())
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for a in accounts {
                            if ui
                                .selectable_label(
                                    Some(a.user_id) == self.core.active,
                                    format!(
                                        "{}{}",
                                        a.email,
                                        if a.locked { " (locked)" } else { "" }
                                    ),
                                )
                                .clicked()
                            {
                                self.core.switch_user(a.user_id);
                            }
                        }
                        if ui.button(super::labels::text("Add account")).clicked() {
                            self.core.add_account();
                        }
                    });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("navigation-scroll")
                    .show(ui, |ui| {
                        let mut filter = self.core.filter;
                        self.group(ui, NavigationGroup::Vaults, "Vaults", |app, ui| {
                            ui.selectable_value(
                                &mut filter.scope,
                                VaultScope::All,
                                super::labels::text("All vaults"),
                            );
                            ui.selectable_value(
                                &mut filter.scope,
                                VaultScope::Personal,
                                "My vault",
                            );
                            if let Some(uid) = app.core.active {
                                for org in app.core.manager.list_organizations(&uid) {
                                    ui.selectable_value(
                                        &mut filter.scope,
                                        VaultScope::Organization(org.id),
                                        org.name,
                                    );
                                }
                            }
                        });
                        self.group(ui, NavigationGroup::View, "View", |_, ui| {
                            for (value, label) in [
                                (VaultSection::Active, "All items"),
                                (VaultSection::Favorites, "Favorites"),
                                (VaultSection::Archive, "Archive"),
                                (VaultSection::Trash, "Trash"),
                            ] {
                                ui.selectable_value(&mut filter.section, value, label);
                            }
                        });
                        self.group(ui, NavigationGroup::Types, "Item types", |_, ui| {
                            ui.selectable_value(
                                &mut filter.category,
                                None,
                                super::labels::text("All types"),
                            );
                            for (kind, label) in TYPES {
                                ui.selectable_value(
                                    &mut filter.category,
                                    Some(*kind),
                                    super::labels::text(label),
                                );
                            }
                        });
                        let folders = self
                            .core
                            .current_cache()
                            .map(|c| {
                                c.folders
                                    .iter()
                                    .map(|f| (f.id, f.name.clone()))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();
                        self.group(ui, NavigationGroup::Folders, "Folders", |app, ui| {
                            if ui.small_button("+ New folder").clicked() {
                                app.core.open_folder(None, false);
                            }
                            ui.selectable_value(
                                &mut filter.folder,
                                FolderFilter::All,
                                "All folders",
                            );
                            ui.selectable_value(
                                &mut filter.folder,
                                FolderFilter::None,
                                "No folder",
                            );
                            for (id, name) in folders {
                                ui.selectable_value(
                                    &mut filter.folder,
                                    FolderFilter::Folder(id),
                                    &name,
                                )
                                .context_menu(|ui| {
                                    if ui.button("Rename folder…").clicked() {
                                        app.core.open_folder(Some(id), false);
                                        ui.close();
                                    }
                                    if ui.button("New subfolder…").clicked() {
                                        app.core.open_folder(None, false);
                                        if let Some(editor) = app.core.folder_editor.as_mut() {
                                            editor.name = format!("{name}/");
                                        }
                                        ui.close();
                                    }
                                    if ui.button("Delete folder…").clicked() {
                                        app.core.open_folder(Some(id), true);
                                        ui.close();
                                    }
                                });
                            }
                        });
                        let collections = self
                            .core
                            .current_cache()
                            .map(|c| c.collections.clone())
                            .unwrap_or_default();
                        self.group(ui, NavigationGroup::Collections, "Collections", |_, ui| {
                            ui.selectable_value(
                                &mut filter.collection,
                                CollectionFilter::All,
                                "All collections",
                            );
                            ui.selectable_value(
                                &mut filter.collection,
                                CollectionFilter::None,
                                "No collection",
                            );
                            for c in collections {
                                ui.selectable_value(
                                    &mut filter.collection,
                                    CollectionFilter::Collection(c.id),
                                    c.name,
                                );
                            }
                        });
                        if filter != self.core.filter {
                            self.core.screen = Screen::Vault;
                            self.core.set_filter(filter);
                        }
                        ui.add_enabled_ui(false, |ui| {
                            self.group(ui, NavigationGroup::Send, "Send", |_, ui| {
                                ui.selectable_label(false, "Sends")
                                    .on_disabled_hover_text("Not available yet");
                            });
                        });
                    });
            });
    }
    fn group(
        &mut self,
        ui: &mut egui::Ui,
        id: NavigationGroup,
        label: &str,
        body: impl FnOnce(&mut Self, &mut egui::Ui),
    ) {
        let collapsed = self.core.settings.collapsed_navigation_groups.contains(&id);
        if ui
            .add(
                egui::Button::new(format!("{}  {label}", if collapsed { "▸" } else { "▾" }))
                    .frame(false),
            )
            .clicked()
        {
            if collapsed {
                self.core.settings.collapsed_navigation_groups.remove(&id);
            } else {
                self.core.settings.collapsed_navigation_groups.insert(id);
            }
            self.core.save_settings();
        }
        if !self.core.settings.collapsed_navigation_groups.contains(&id) {
            ui.indent(id, |ui| body(self, ui));
        }
        ui.add_space(5.);
    }
    fn filter_title(&self) -> String {
        let f = self.core.filter;
        let mut parts = vec![match f.scope {
            VaultScope::All => "All vaults".into(),
            VaultScope::Personal => "My vault".into(),
            VaultScope::Organization(id) => self
                .core
                .active
                .and_then(|uid| {
                    self.core
                        .manager
                        .list_organizations(&uid)
                        .into_iter()
                        .find(|o| o.id == id)
                })
                .map(|o| o.name)
                .unwrap_or("Organization".into()),
        }];
        if let Some(category) = f.category {
            parts.push(
                TYPES
                    .iter()
                    .find(|(t, _)| *t == category)
                    .map(|(_, s)| *s)
                    .unwrap_or("Items")
                    .into(),
            );
        }
        if f.section != VaultSection::Active {
            parts.push(format!("{:?}", f.section));
        }
        match f.folder {
            FolderFilter::None => parts.push("No folder".into()),
            FolderFilter::Folder(id) => {
                if let Some(folder) = self
                    .core
                    .current_cache()
                    .and_then(|c| c.folders.iter().find(|f| f.id == id))
                {
                    parts.push(folder.name.clone());
                }
            }
            _ => {}
        }
        if let CollectionFilter::Collection(id) = f.collection
            && let Some(c) = self
                .core
                .current_cache()
                .and_then(|c| c.collections.iter().find(|c| c.id == id))
        {
            parts.push(c.name.clone());
        }
        if f.collection == CollectionFilter::None {
            parts.push("No collection".into());
        }
        parts.join(" / ")
    }
    pub(super) fn vault(&mut self, ui: &mut egui::Ui) {
        // Labels must not steal row clicks or offer an unscoped copy path.
        ui.style_mut().interaction.selectable_labels = false;
        ui.horizontal(|ui| {
            ui.heading(self.filter_title());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(!self.core.remote(), |ui| {
                    ui.menu_button("+ New", |ui| {
                        for (kind, label) in TYPES {
                            if ui.button(super::labels::text(label)).clicked() {
                                self.core.new_item(*kind);
                                ui.close();
                            }
                        }
                    });
                });
            });
        });
        ui.horizontal(|ui| {
            let mut query = self.core.query.clone();
            let response = ui.add_sized(
                [ui.available_width() - 75., 30.],
                egui::TextEdit::singleline(&mut query)
                    .id(Id::unique("vault-search"))
                    .hint_text("Search vault"),
            );
            if self.focus_search {
                response.request_focus();
                self.focus_search = false;
            }
            if ui.button("Clear").clicked() {
                query.clear();
            }
            if query != self.core.query {
                self.core.set_query(query);
            }
        });
        let count = self.core.current_cache().map_or(0, |c| c.visible.len());
        ui.horizontal(|ui| {
            ui.weak(format!("{count} items"));
            if let Some(sync) = self.core.active.and_then(|uid| self.core.sync.get(&uid)) {
                if sync.busy {
                    ui.spinner();
                    ui.weak("Synchronizing…");
                }
                if let Some(e) = &sync.error {
                    ui.colored_label(egui::Color32::RED, e);
                }
            }
        });
        if let Some(count) = self
            .core
            .active
            .map(|uid| self.core.manager.local_edit_count(&uid))
            .filter(|count| *count > 0)
        {
            ui.weak(format!(
                "{count} pending local changes — File → Synchronize retries the upload"
            ));
        }
        if let Some(uid) = self.core.active {
            let conflicts = self.core.manager.local_conflicts(&uid);
            if !conflicts.is_empty() {
                ui.collapsing(format!("{} sync conflicts / unconfirmed updates",conflicts.len()), |ui| {
                    ui.label("Server versions stay current. Drafts are retained and will not be uploaded automatically. Review and save a draft to reapply and synchronize it.");
                    for id in conflicts {
                        let item = self.core.current_cache().and_then(|c|c.all.iter().find(|v|v.id==Some(id))).cloned();
                        ui.push_id(id, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(item.as_ref().map_or_else(||format!("Unavailable server entry ({id})"),|i|i.name.clone()));
                                if ui.add_enabled(item.is_some(),egui::Button::new("Server version")).clicked() {self.core.request(crate::core::model::Request::Read(id));}
                                if ui.add_enabled(item.as_ref().is_some_and(|i|i.edit&&i.view_password),egui::Button::new("Review local draft…")).clicked() {self.core.request(crate::core::model::Request::ReviewLocal(id));}
                                ui.menu_button("Discard draft…", |ui| {
                                    ui.label("Permanently discard these local changes?");
                                    if ui.button("Confirm discard").clicked() {self.core.request(crate::core::model::Request::DiscardLocal(id));ui.close();}
                                });
                            });
                        });
                    }
                });
            }
        }
        if self
            .core
            .active
            .is_some_and(|uid| self.core.manager.has_folder_intent(&uid))
        {
            ui.group(|ui| {
                ui.label("A folder request is awaiting reconciliation. Synchronize to check its outcome before repeating it.");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Check server").clicked() {self.core.check_folder_outcome();}
                    ui.menu_button("Dismiss pending record…",|ui| {
                        ui.label("This only clears the local request record. The server may already have applied the change. Check your folders before creating another copy.");
                        if ui.button("Confirm dismiss").clicked() {self.core.dismiss_folder_intent();ui.close();}
                    });
                });
            });
        }
        let area = ui.available_rect_before_wrap();
        let list_id = Id::unique("vault-list-focus");
        let focus = ui.interact(area, list_id, Sense::focusable_noninteractive());
        ui.ctx().memory_mut(|m| {
            m.set_focus_lock_filter(
                list_id,
                egui::EventFilter {
                    vertical_arrows: true,
                    escape: true,
                    ..Default::default()
                },
            )
        });
        if self.list_focus {
            focus.request_focus();
            self.list_focus = false;
        }
        if focus.has_focus() && !self.any_modal() {
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.core.close_details();
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                self.core.navigate(true);
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                self.core.navigate(false);
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
                && let Some(id) = self.core.selected
            {
                self.core.request(Request::Read(id));
            }
            let down = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::PageDown));
            let up = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::PageUp));
            if (down || up)
                && let Some(c) = self.core.current_cache()
            {
                let current = c
                    .visible
                    .iter()
                    .position(|v| v.id == self.core.selected)
                    .unwrap_or(0);
                let page =
                    (area.height() / (ROW_HEIGHT + ui.spacing().item_spacing.y)).max(1.) as usize;
                let next = if down {
                    current.saturating_add(page)
                } else {
                    current.saturating_sub(page)
                }
                .min(c.visible.len().saturating_sub(1));
                if let Some(id) = c.visible.get(next).and_then(|v| v.id) {
                    self.core.request(Request::Read(id));
                    self.core.scroll_to = Some(next);
                }
            }
        }
        let mut clicked = None;
        let mut sort = None;
        let selected = self.core.selected;
        let sorting = self.core.settings.vault_sort;
        let scroll = self.core.scroll_to.take();
        let restore_offset = self.restore_offset.take();
        let mut measured_offset = self.list_offset;
        let show_icons = self.core.settings.show_favicons;
        let uid = self.core.active;
        let favicon = self.core.favicon.clone();
        let textures = &mut self.icon_textures;
        // Borrow the cached projection: no vault-sized clone or sorting during drawing.
        let items = self
            .core
            .current_cache()
            .map(|c| c.visible.as_slice())
            .unwrap_or(&[]);
        egui::ScrollArea::horizontal()
            .id_salt("vault-horizontal")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(area.width().max(760.));
                let mut table = TableBuilder::new(ui)
                    .id_salt("vault-table-username")
                    .resizable(true)
                    .sense(Sense::CLICK)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::remainder().at_least(180.).clip(true))
                    .column(Column::initial(180.).clip(true))
                    .column(Column::initial(145.).clip(true))
                    .column(Column::initial(220.).clip(true))
                    .auto_shrink([false, false]);
                if let Some(offset) = restore_offset {
                    table = table.vertical_scroll_offset(offset);
                }
                if let Some(row) = scroll {
                    table = table.scroll_to_row(row, None);
                }
                let output = table
                    .header(28., |mut header| {
                        for (label, column) in [
                            ("Name", SortColumn::Name),
                            ("Username", SortColumn::Username),
                            ("Last changed", SortColumn::LastChanged),
                            ("Base URL", SortColumn::BaseUrl),
                        ] {
                            header.col(|ui| {
                                let suffix = if sorting.column == column {
                                    if sorting.direction == SortDirection::Ascending {
                                        " ↑"
                                    } else {
                                        " ↓"
                                    }
                                } else {
                                    ""
                                };
                                if ui
                                    .add(
                                        egui::Button::new(format!(
                                            "{}{suffix}",
                                            super::labels::text(label)
                                        ))
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    sort = Some(column);
                                }
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(ROW_HEIGHT, items.len(), |mut row| {
                            let item = &items[row.index()];
                            let chosen = item.id == selected;
                            row.set_selected(chosen);
                            row.col(|ui| {
                                ui.horizontal(|ui| {
                                    let mark = match kind(&item.r#type) {
                                        CipherType::Login => "L",
                                        CipherType::SshKey => "SSH",
                                        CipherType::SecureNote => "N",
                                        CipherType::Card => "C",
                                        _ => "I",
                                    };
                                    let mut shown = false;
                                    if show_icons
                                        && let (Some(uid), CipherListViewType::Login(login)) =
                                            (uid, &item.r#type)
                                        && let Some(host) = login.uris.as_ref().and_then(|u| {
                                            u.iter().find_map(|u| {
                                                u.uri
                                                    .as_deref()
                                                    .and_then(services::favicon::hostname_for_fetch)
                                            })
                                        })
                                        && let services::favicon::IconState::Found(image) =
                                            favicon.get(&uid, &host)
                                    {
                                        let texture = textures.entry(host).or_insert_with(|| {
                                            ui.ctx().load_texture(
                                                "website-icon",
                                                (*image).clone(),
                                                egui::TextureOptions::LINEAR,
                                            )
                                        });
                                        ui.image((texture.id(), egui::vec2(22., 22.)));
                                        shown = true;
                                    }
                                    if !shown {
                                        if chosen {
                                            ui.label(mark);
                                        } else {
                                            ui.weak(mark);
                                        }
                                    }
                                    ui.add(egui::Label::new(&item.name).truncate());
                                });
                            });
                            row.col(|ui| {
                                if let CipherListViewType::Login(login) = &item.r#type
                                    && let Some(username) = &login.username
                                {
                                    ui.add(egui::Label::new(username).truncate());
                                }
                            });
                            row.col(|ui| {
                                ui.small(
                                    item.revision_date
                                        .with_timezone(&chrono::Local)
                                        .format("%Y-%m-%d %H:%M")
                                        .to_string(),
                                );
                            });
                            row.col(|ui| {
                                if let CipherListViewType::Login(login) = &item.r#type {
                                    let url = login
                                        .uris
                                        .as_ref()
                                        .and_then(|u| {
                                            u.iter().filter_map(|u| u.uri.as_deref()).find_map(
                                                services::vault_sort::base_url_for_display,
                                            )
                                        })
                                        .unwrap_or_default();
                                    ui.add(egui::Label::new(url).truncate());
                                }
                            });
                            if row.response().clicked() {
                                clicked = item.id;
                            }
                        })
                    });
                measured_offset = output.state.clamped_offset().y;
            });
        self.list_offset = measured_offset;
        if let Some(column) = sort {
            self.core.sort_by(column);
        }
        if let Some(id) = clicked {
            self.core.request(Request::Read(id));
            focus.request_focus();
        }
        if self.core.detail_open
            && !self.any_modal()
            && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.core.close_details();
            self.list_focus = true;
        }
    }
}
