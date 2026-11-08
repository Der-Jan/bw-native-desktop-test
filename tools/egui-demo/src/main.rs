mod model;
mod typography;

use eframe::egui::{
    self, Align, Color32, FontId, Id, Key, Modifiers, RichText, Sense, Stroke, TextEdit,
};
use egui_extras::{Column as TableColumn, TableBuilder};
use model::{Column, FOLDERS, Filters, Kind, Model, VAULTS};

const BLUE: Color32 = Color32::from_rgb(29, 56, 139);
const ACCENT: Color32 = Color32::from_rgb(22, 92, 225);
const MUTED: Color32 = Color32::from_rgb(91, 110, 145);
const BACKGROUND: Color32 = Color32::from_rgb(245, 247, 250);
const ROW_HEIGHT: f32 = 43.0;
fn list_id() -> Id {
    Id::unique("vault-list-focus")
}
fn search_id() -> Id {
    Id::unique("vault-search")
}

struct Demo {
    model: Model,
    detail_open: bool,
    reveal_password: bool,
    pending: Option<usize>,
    confirmation: String,
    confirmation_error: bool,
    focus_confirmation: bool,
    scroll_to: Option<usize>,
    font_label: String,
    dataset_size: usize,
}
impl Demo {
    fn new(
        cc: &eframe::CreationContext<'_>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        cc.egui_ctx.set_theme(egui::Theme::Light);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let font_label = typography::install(&cc.egui_ctx)?;
        if let Some(gl) = &cc.gl {
            use eframe::glow::HasContext;
            // Query only our own graphics context, no vault/account information.
            eprintln!("egui renderer: {}", unsafe {
                gl.get_parameter_string(eframe::glow::RENDERER)
            });
        }
        Ok(Self {
            model: Model::new(10_000),
            detail_open: false,
            reveal_password: false,
            pending: None,
            confirmation: String::new(),
            confirmation_error: false,
            focus_confirmation: false,
            scroll_to: None,
            font_label,
            dataset_size: 10_000,
        })
    }

    fn choose(&mut self, id: usize, ctx: &egui::Context) {
        if !self.model.visible.contains(&id) {
            return;
        }
        self.scroll_to = self.model.visible.iter().position(|v| *v == id);
        if self.model.items[id].protected {
            self.pending = Some(id);
            self.confirmation.clear();
            self.confirmation_error = false;
            self.focus_confirmation = true;
        } else {
            self.open_details(id, ctx);
        }
    }
    fn open_details(&mut self, id: usize, ctx: &egui::Context) {
        self.model.select(id);
        self.detail_open = true;
        self.reveal_password = false;
        self.scroll_to = self.model.selected_position();
        focus_list(ctx);
    }
    fn filter_changed(&mut self) {
        self.model.refilter();
        self.pending = None;
        self.confirmation.clear();
        if self.model.selected.is_none() {
            self.detail_open = false;
        }
        self.scroll_to = Some(0);
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("demo-toolbar")
            .frame(egui::Frame::new().fill(Color32::WHITE).inner_margin(12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("egui Vergleich").strong());
                    ui.separator();
                    ui.label(RichText::new("Ausschließlich synthetische Testdaten").color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let previous = self.dataset_size;
                        egui::ComboBox::from_id_salt("dataset-size")
                            .selected_text(format!("{} Einträge", number(self.dataset_size)))
                            .show_ui(ui, |ui| {
                                for size in [1000, 10_000, 50_000] {
                                    ui.selectable_value(
                                        &mut self.dataset_size,
                                        size,
                                        format!("{} Einträge", number(size)),
                                    );
                                }
                            });
                        if previous != self.dataset_size {
                            self.model.replace_data(self.dataset_size);
                            self.filter_changed();
                        }
                    });
                });
            });
        egui::Panel::bottom("demo-status").frame(egui::Frame::new().fill(BACKGROUND).inner_margin(8)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.small(format!("{} von {} Einträgen", number(self.model.visible.len()), number(self.model.items.len())));
                ui.separator();
                ui.small("Tab: Fokus wechseln   ↑/↓: Auswahl in der Liste   Esc: Details schließen   Strg+F: Suche");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| { ui.small(&self.font_label); });
            });
        });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("filters")
            .exact_size(220.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(BLUE).inner_margin(14))
            .show(ui, |ui| {
                ui.visuals_mut().override_text_color = Some(Color32::WHITE);
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.visuals_mut().widgets.inactive.fg_stroke.color = Color32::WHITE;
                ui.visuals_mut().widgets.hovered.fg_stroke.color = Color32::WHITE;
                ui.visuals_mut().widgets.noninteractive.fg_stroke.color = Color32::WHITE;
                ui.visuals_mut().selection.bg_fill = Color32::from_rgb(10, 31, 107);
                ui.visuals_mut().selection.stroke = Stroke::new(1.0, Color32::WHITE);
                ui.visuals_mut().widgets.hovered.weak_bg_fill = Color32::from_rgb(54, 78, 158);
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                ui.add(
                    egui::Image::new(egui::include_image!(
                        "../../../assets/password-manager-logo.svg"
                    ))
                    .fit_to_exact_size(egui::vec2(188.0, 46.06)),
                );
                ui.add_space(20.0);
                let before = self.model.filters.clone();
                egui::ScrollArea::vertical()
                    .id_salt("filters-scroll")
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        egui::CollapsingHeader::new("Tresore")
                            .default_open(true)
                            .show(ui, |ui| {
                                filter_option(
                                    ui,
                                    &mut self.model.filters.vault,
                                    None,
                                    "Alle Tresore",
                                );
                                for (index, label) in VAULTS.iter().enumerate() {
                                    filter_option(
                                        ui,
                                        &mut self.model.filters.vault,
                                        Some(index),
                                        label,
                                    );
                                }
                            });
                        ui.add_space(12.0);
                        egui::CollapsingHeader::new("Eintragstyp")
                            .default_open(true)
                            .show(ui, |ui| {
                                filter_option(ui, &mut self.model.filters.kind, None, "Alle Typen");
                                for kind in Kind::ALL {
                                    filter_option(
                                        ui,
                                        &mut self.model.filters.kind,
                                        Some(kind),
                                        kind.label(),
                                    );
                                }
                            });
                        ui.add_space(12.0);
                        egui::CollapsingHeader::new("Ordner")
                            .default_open(true)
                            .show(ui, |ui| {
                                filter_option(
                                    ui,
                                    &mut self.model.filters.folder,
                                    None,
                                    "Alle Ordner",
                                );
                                for (index, label) in FOLDERS.iter().enumerate() {
                                    filter_option(
                                        ui,
                                        &mut self.model.filters.folder,
                                        Some(index),
                                        label,
                                    );
                                }
                            });
                        ui.add_space(12.0);
                        ui.checkbox(&mut self.model.filters.favorites, "Nur Favoriten");
                        ui.add_space(14.0);
                        if ui.button("Alle Filter zurücksetzen").clicked() {
                            self.model.filters = Filters::default();
                        }
                    });
                if self.model.filters != before {
                    self.filter_changed();
                }
            });
    }

    fn details(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.model.selected.filter(|_| self.detail_open) else {
            return;
        };
        let mut close = false;
        egui::Panel::right("item-details")
            .default_size(350.0)
            .min_size(290.0)
            .max_size(600.0)
            .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(20))
            .show(ui, |ui| {
                let item = &self.model.items[id];
                ui.horizontal(|ui| {
                    ui.heading("Details");
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        close = ui
                            .button("Schließen  ×")
                            .on_hover_text("Escape schließt die Detailansicht")
                            .clicked();
                    });
                });
                ui.add_space(16.0);
                egui::ScrollArea::vertical()
                    .id_salt(("detail-scroll", id))
                    .show(ui, |ui| {
                        ui.label(RichText::new(&item.name).size(19.0).strong());
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(format!(
                                "{} · {}",
                                VAULTS[item.vault], FOLDERS[item.folder]
                            ))
                            .color(MUTED),
                        );
                        ui.label(RichText::new(item.kind.label()).color(MUTED));
                        ui.add_space(16.0);
                        for (label, value) in &item.fields {
                            ui.label(RichText::new(*label).small().color(MUTED));
                            egui::Frame::new()
                                .fill(Color32::WHITE)
                                .inner_margin(10)
                                .corner_radius(4)
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal_wrapped(|ui| {
                                        if *label == "Passwort" && !self.reveal_password {
                                            ui.label("••••••••••••••••");
                                        } else {
                                            ui.add(egui::Label::new(value).selectable(true).wrap());
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        if ui.small_button("Kopieren").clicked() {
                                            ui.ctx().copy_text(value.clone());
                                        }
                                        if *label == "Passwort" {
                                            ui.checkbox(&mut self.reveal_password, "Anzeigen");
                                        }
                                    });
                                });
                            ui.add_space(10.0);
                        }
                        if !item.url.is_empty() {
                            ui.label(RichText::new("Base URL").small().color(MUTED));
                            ui.add(egui::Label::new(&item.url).selectable(true).wrap());
                            ui.add_space(12.0);
                        }
                        ui.separator();
                        ui.label(RichText::new("Notizen").strong());
                        ui.add(egui::Label::new(&item.notes).selectable(true).wrap());
                        ui.add_space(14.0);
                        ui.label(
                            RichText::new(format!("Geändert: {} UTC", item.changed_label))
                                .small()
                                .color(MUTED),
                        );
                        if item.protected {
                            ui.add_space(10.0);
                            ui.small("Demo-Passwortbestätigung für diesen Eintrag aktiviert.");
                        }
                    });
            });
        if close {
            self.detail_open = false;
            focus_list(ui.ctx());
        }
    }

    fn table(&mut self, ui: &mut egui::Ui) {
        ui.heading(
            self.model
                .filters
                .vault
                .map(|v| VAULTS[v])
                .unwrap_or("Alle Tresore"),
        );
        ui.label(RichText::new(self.model.filter_summary()).color(MUTED));
        ui.add_space(8.0);
        let focus_search = self.pending.is_none()
            && ui.ctx().input_mut(|i| {
                i.consume_key(Modifiers::CTRL, Key::F) || i.consume_key(Modifiers::COMMAND, Key::F)
            });
        ui.horizontal(|ui| {
            let search = ui.add_sized(
                [ui.available_width() - 84.0, 30.0],
                TextEdit::singleline(&mut self.model.filters.query)
                    .id(search_id())
                    .hint_text("Suchen: Name, Benutzername, URL, Lizenzschlüssel …"),
            );
            if focus_search {
                search.request_focus();
            }
            let clear = ui.button("Leeren").clicked();
            if clear {
                self.model.filters.query.clear();
            }
            if search.changed() || clear {
                self.filter_changed();
            }
        });
        ui.add_space(8.0);
        let area = ui.available_rect_before_wrap();
        // One normal egui focus target for the list, independent of row culling.
        let focus = ui.interact(area, list_id(), Sense::focusable_noninteractive());
        let focused = focus.has_focus();
        ui.ctx().memory_mut(|m| {
            m.set_focus_lock_filter(
                list_id(),
                egui::EventFilter {
                    vertical_arrows: true,
                    escape: true,
                    ..Default::default()
                },
            )
        });
        if focused && self.pending.is_none() {
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.detail_open = false;
                ui.ctx().request_repaint();
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
                && let Some(id) = self.model.selected
            {
                self.choose(id, ui.ctx());
            }
            let down = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
            let up = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
            let page_down = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::PageDown));
            let page_up = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::PageUp));
            if down || up {
                if let Some(id) = self.model.adjacent(down) {
                    self.choose(id, ui.ctx());
                }
            } else if page_down || page_up {
                let page = (area.height() / (ROW_HEIGHT + ui.spacing().item_spacing.y))
                    .floor()
                    .max(1.0) as usize;
                let current = self.model.selected_position().unwrap_or(0);
                let position = if page_down {
                    current.saturating_add(page)
                } else {
                    current.saturating_sub(page)
                };
                if let Some(id) = self
                    .model
                    .visible
                    .get(position.min(self.model.visible.len().saturating_sub(1)))
                    .copied()
                {
                    self.choose(id, ui.ctx());
                }
            }
        }
        let mut sort = None;
        let mut clicked = None;
        let width = area.width().max(620.0);
        egui::ScrollArea::horizontal()
            .id_salt("table-horizontal")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(width);
                let mut table = TableBuilder::new(ui)
                    .id_salt("vault-table")
                    .striped(false)
                    .resizable(true)
                    .sense(Sense::CLICK)
                    .column(TableColumn::remainder().at_least(180.0).clip(true))
                    .column(TableColumn::initial(148.0).range(100.0..=210.0).clip(true))
                    .column(TableColumn::initial(230.0).range(120.0..=420.0).clip(true))
                    .auto_shrink([false, false]);
                if let Some(row) = self.scroll_to.take() {
                    table = table.scroll_to_row(row, None);
                }
                let items = &self.model.items;
                let selected = self.model.selected;
                let visible = &self.model.visible;
                table
                    .header(28.0, |mut header| {
                        for (label, column) in [
                            ("Name", Column::Name),
                            ("Last changed", Column::Changed),
                            ("Base URL", Column::Url),
                        ] {
                            header.col(|ui| {
                                let arrow = if self.model.sort.column == column {
                                    if self.model.sort.descending {
                                        " ↓"
                                    } else {
                                        " ↑"
                                    }
                                } else {
                                    ""
                                };
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(format!("{label}{arrow}"))
                                                .strong()
                                                .color(MUTED),
                                        )
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
                        body.rows(ROW_HEIGHT, visible.len(), |mut row| {
                            let id = visible[row.index()];
                            let item = &items[id];
                            row.set_selected(selected == Some(id));
                            row.col(|ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(29.0, 29.0),
                                        Sense::hover(),
                                    );
                                    let colors = [
                                        Color32::from_rgb(46, 155, 126),
                                        ACCENT,
                                        Color32::from_rgb(125, 78, 179),
                                        Color32::from_rgb(180, 128, 37),
                                        Color32::from_rgb(176, 64, 107),
                                    ];
                                    ui.painter().circle_filled(
                                        rect.center(),
                                        14.0,
                                        colors[id % colors.len()],
                                    );
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        item.kind.mark(),
                                        FontId::proportional(13.0),
                                        Color32::WHITE,
                                    );
                                    ui.vertical(|ui| {
                                        ui.add(egui::Label::new(&item.name).truncate());
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&item.subtitle).small().color(MUTED),
                                            )
                                            .truncate(),
                                        );
                                    });
                                });
                            });
                            row.col(|ui| {
                                ui.label(RichText::new(&item.changed_label).small().color(MUTED));
                            });
                            row.col(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(if item.url.is_empty() {
                                            "—"
                                        } else {
                                            &item.url
                                        })
                                        .color(MUTED),
                                    )
                                    .truncate(),
                                );
                            });
                            if row.response().clicked() {
                                clicked = Some(id);
                            }
                        });
                    });
            });
        if let Some(column) = sort {
            self.model.sort_by(column);
            self.scroll_to = self.model.selected_position().or(Some(0));
            ui.ctx().request_repaint();
        }
        if let Some(id) = clicked {
            self.choose(id, ui.ctx());
        }
        if self.model.visible.is_empty() {
            ui.painter().text(
                area.center(),
                egui::Align2::CENTER_CENTER,
                "Keine Einträge für diese Filter",
                FontId::proportional(16.0),
                MUTED,
            );
        }
        if focused {
            ui.painter().rect_stroke(
                area,
                3,
                Stroke::new(1.0, ACCENT.gamma_multiply(0.55)),
                egui::StrokeKind::Inside,
            );
        }
    }

    fn confirmation_dialog(&mut self, ctx: &egui::Context) {
        let Some(id) = self.pending else {
            return;
        };
        let mut confirm = false;
        let mut cancel = false;
        let response = egui::Modal::new(Id::unique("demo-password-confirmation")).show(ctx, |ui| {
            ui.set_width(400.0);
            ui.heading("Master-Passwort bestätigen");
            ui.label(&self.model.items[id].name);
            ui.add_space(8.0);
            ui.label("UI-Test: Bitte mit dem Demo-Passwort „demo“ bestätigen.");
            let input = ui.add(
                TextEdit::singleline(&mut self.confirmation)
                    .password(true)
                    .desired_width(f32::INFINITY),
            );
            if self.focus_confirmation {
                input.request_focus();
                self.focus_confirmation = false;
            }
            confirm |= input.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if self.confirmation_error {
                ui.colored_label(
                    Color32::from_rgb(180, 40, 40),
                    "Das Demo-Passwort lautet: demo",
                );
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                confirm |= ui.button("Bestätigen").clicked();
                cancel |= ui.button("Abbrechen").clicked();
            });
        });
        cancel |= response.should_close();
        if cancel {
            self.pending = None;
            self.confirmation.clear();
            focus_list(ctx);
        } else if confirm {
            if self.confirmation == "demo" {
                self.pending = None;
                self.confirmation.clear();
                self.open_details(id, ctx);
            } else {
                self.confirmation.clear();
                self.confirmation_error = true;
                self.focus_confirmation = true;
                ctx.request_repaint();
            }
        }
    }
}
impl Demo {
    fn show(&mut self, ui: &mut egui::Ui) {
        self.toolbar(ui);
        self.sidebar(ui);
        self.details(ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::WHITE).inner_margin(18))
            .show(ui, |ui| self.table(ui));
        self.confirmation_dialog(ui.ctx());
        // The top modal consumes Escape first; otherwise it closes the detail
        // panel even when one of that panel's controls had keyboard focus.
        if self.pending.is_none()
            && self.detail_open
            && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.detail_open = false;
            focus_list(ui.ctx());
        }
    }
}

impl eframe::App for Demo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

fn focus_list(ctx: &egui::Context) {
    ctx.memory_mut(|m| {
        m.request_focus(list_id());
        m.set_focus_lock_filter(
            list_id(),
            egui::EventFilter {
                vertical_arrows: true,
                escape: true,
                ..Default::default()
            },
        );
    });
    ctx.request_repaint();
}

fn filter_option<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    current: &mut Option<T>,
    value: Option<T>,
    label: &str,
) {
    let response = ui.add_sized(
        [ui.available_width(), 27.0],
        egui::Button::selectable(*current == value, label),
    );
    if response.clicked() {
        *current = value;
    }
}
fn number(value: usize) -> String {
    let text = value.to_string();
    let mut output = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i > 0 && (text.len() - i).is_multiple_of(3) {
            output.push('.');
        }
        output.push(ch);
    }
    output
}
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Bitwarden — egui Demo (Testdaten)")
            .with_inner_size([1250.0, 840.0])
            .with_min_inner_size([960.0, 620.0])
            .with_app_id("com.bitwarden.egui.demo"),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "bw-egui-demo",
        options,
        Box::new(|cc| Ok(Box::new(Demo::new(cc)?))),
    )
}

#[cfg(test)]
mod ui_tests {
    use super::*;

    fn setup() -> (egui::Context, Demo) {
        let ctx = egui::Context::default();
        ctx.set_theme(egui::Theme::Light);
        let mut fonts = egui::FontDefinitions::empty();
        // Reproducible UI-test font only; the normal demo embeds no text fonts.
        fonts.font_data.insert(
            "test".into(),
            egui::FontData::from_static(include_bytes!(
                "../../../assets/Inter/static/Inter_18pt-Medium.ttf"
            ))
            .into(),
        );
        fonts
            .families
            .insert(egui::FontFamily::Proportional, vec!["test".into()]);
        fonts
            .families
            .insert(egui::FontFamily::Monospace, vec!["test".into()]);
        ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(&ctx);
        let app = Demo {
            model: Model::new(100),
            detail_open: false,
            reveal_password: false,
            pending: None,
            confirmation: String::new(),
            confirmation_error: false,
            focus_confirmation: false,
            scroll_to: None,
            font_label: "Test font".into(),
            dataset_size: 100,
        };
        (ctx, app)
    }
    fn frame(ctx: &egui::Context, app: &mut Demo, events: Vec<egui::Event>) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1250.0, 840.0),
                )),
                events,
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        output.textures_delta.clear();
    }
    fn key(key: Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }
    #[test]
    fn arrows_only_change_selection_with_list_focus_and_escape_keeps_cursor() {
        let (ctx, mut app) = setup();
        app.model.filters.kind = Some(Kind::Login);
        app.model.refilter();
        frame(&ctx, &mut app, vec![]);
        let first = app.model.visible[0];
        let second = app.model.visible[1];
        app.open_details(first, &ctx);
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![key(Key::ArrowDown)]);
        assert_eq!(app.model.selected, Some(second));
        frame(&ctx, &mut app, vec![key(Key::Escape)]);
        assert!(!app.detail_open);
        assert_eq!(app.model.selected, Some(second));
        ctx.memory_mut(|m| m.request_focus(search_id()));
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![key(Key::ArrowDown)]);
        assert_eq!(
            app.model.selected,
            Some(second),
            "search input must not navigate the list"
        );
    }
    #[test]
    fn confirmation_blocks_navigation_and_enter_confirms_only_demo_password() {
        let (ctx, mut app) = setup();
        frame(&ctx, &mut app, vec![]);
        app.choose(4, &ctx); // Protected synthetic software license.
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![key(Key::ArrowDown)]);
        assert!(app.model.selected.is_none());
        assert_eq!(app.pending, Some(4));
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Text("wrong".into()), key(Key::Enter)],
        );
        assert!(app.confirmation_error);
        assert_eq!(app.pending, Some(4));
        frame(&ctx, &mut app, vec![]);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Text("demo".into()), key(Key::Enter)],
        );
        assert!(app.pending.is_none());
        assert_eq!(app.model.selected, Some(4));
        assert!(app.detail_open);
    }
}
