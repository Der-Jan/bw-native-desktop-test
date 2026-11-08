use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId, TextStyle};

type LoadedFonts = (FontDefinitions, String, f32);
static FONTS: std::sync::OnceLock<Result<LoadedFonts, String>> = std::sync::OnceLock::new();
fn load() -> Result<LoadedFonts, String> {
    let (family, points) = kde_font().unwrap_or_else(|| ("sans-serif".into(), 11.0));
    let mut db = fontdb::Database::new();
    #[cfg(target_os = "linux")]
    for requested in [&family, "monospace", "DejaVu Sans"] {
        if let Ok(output) = std::process::Command::new("fc-match")
            .args(["--format=%{file}", requested])
            .output()
            && output.status.success()
            && let Ok(path) = std::str::from_utf8(&output.stdout)
        {
            let _ = db.load_font_file(path.trim());
        }
    }
    if db.faces().next().is_none() {
        db.load_system_fonts();
    }
    let mono_family = db
        .faces()
        .find(|f| f.monospaced)
        .and_then(|f| f.families.first())
        .map(|f| f.0.clone());
    if let Some(family) = mono_family {
        db.set_monospace_family(family);
    }
    let families = [fontdb::Family::Name(&family), fontdb::Family::SansSerif];
    let id = db
        .query(&fontdb::Query {
            families: &families,
            ..Default::default()
        })
        .or_else(|| db.faces().next().map(|f| f.id))
        .ok_or("Keine installierte Systemschrift gefunden")?;
    let data = db
        .with_face_data(id, |bytes, index| {
            let mut data = FontData::from_owned(bytes.to_vec());
            data.index = index;
            data
        })
        .ok_or("Systemschrift konnte nicht gelesen werden")?;
    let mut fonts = FontDefinitions::empty();
    fonts.font_data.insert("desktop".into(), data.into());
    fonts
        .families
        .insert(FontFamily::Proportional, vec!["desktop".into()]);
    fonts
        .families
        .insert(FontFamily::Monospace, vec!["desktop".into()]);
    // Keep desktop text metrics, but let installed symbol fonts supply arrows
    // and other UI glyphs absent from some Noto Sans builds.
    if let Some(symbols) = db.query(&fontdb::Query {
        families: &[
            fontdb::Family::Name("DejaVu Sans"),
            fontdb::Family::Name("Noto Sans Symbols"),
            fontdb::Family::Name("Noto Sans Symbols 2"),
        ],
        ..Default::default()
    }) && symbols != id
        && let Some(data) = db.with_face_data(symbols, |bytes, index| {
            let mut data = FontData::from_owned(bytes.to_vec());
            data.index = index;
            data
        })
    {
        fonts.font_data.insert("system-symbols".into(), data.into());
        for family in fonts.families.values_mut() {
            family.push("system-symbols".into());
        }
    }
    if let Some(mono) = db.query(&fontdb::Query {
        families: &[fontdb::Family::Monospace],
        ..Default::default()
    }) && let Some(data) = db.with_face_data(mono, |bytes, index| {
        let mut data = FontData::from_owned(bytes.to_vec());
        data.index = index;
        data
    }) {
        fonts.font_data.insert("desktop-mono".into(), data.into());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "desktop-mono".into());
    }
    let body = (points * 0.9).round().max(9.) * 96. / 72.;
    Ok((
        fonts,
        format!("{family} · {:.0} pt", body * 72. / 96.),
        body,
    ))
}
pub fn install(ctx: &egui::Context) -> Result<String, String> {
    let (fonts, description, body) = FONTS.get_or_init(load).as_ref().map_err(Clone::clone)?;
    let body = *body;
    ctx.set_fonts(fonts.clone());
    ctx.options_mut(|options| options.fallback_theme = egui::Theme::Light);
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.style_mut_of(theme, |style| {
            style
                .text_styles
                .insert(TextStyle::Body, FontId::proportional(body));
            style
                .text_styles
                .insert(TextStyle::Button, FontId::proportional(body));
            style
                .text_styles
                .insert(TextStyle::Small, FontId::proportional(12.0));
            style
                .text_styles
                .insert(TextStyle::Heading, FontId::proportional(24.0));
            style
                .text_styles
                .insert(TextStyle::Monospace, FontId::monospace(body));
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(10.0, 6.0);
            style.visuals.override_text_color = Some(if theme == egui::Theme::Light {
                egui::Color32::from_rgb(30, 40, 57)
            } else {
                egui::Color32::from_rgb(225, 229, 237)
            });
            style.visuals.selection.bg_fill = egui::Color32::from_rgb(22, 92, 225);
            style.visuals.selection.stroke = egui::Stroke::new(1., egui::Color32::WHITE);
            style.animation_time = 0.0;
            style.visuals.window_shadow = egui::epaint::Shadow::NONE;
            style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
        });
    }
    Ok(description.clone())
}

fn kde_font() -> Option<(String, f32)> {
    if !std::env::var("XDG_CURRENT_DESKTOP")
        .is_ok_and(|v| v.split(':').any(|s| s.eq_ignore_ascii_case("KDE")))
    {
        return None;
    }
    for command in ["kreadconfig6", "kreadconfig5"] {
        let Ok(output) = std::process::Command::new(command)
            .args([
                "--file",
                "kdeglobals",
                "--group",
                "General",
                "--key",
                "font",
            ])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let Ok(value) = std::str::from_utf8(&output.stdout) else {
            continue;
        };
        let mut parts = value.trim().split(',');
        let family = parts.next()?.trim();
        let points: f32 = parts.next()?.parse().ok()?;
        if !family.is_empty() && points.is_finite() && (6.0..=96.0).contains(&points) {
            return Some((family.into(), points));
        }
    }
    None
}
