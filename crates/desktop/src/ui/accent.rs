//! Interaction colors follow the desktop; SVG branding is deliberately separate.
use crate::services::desktop_appearance::Accent;
use eframe::egui::{self, Color32, Id, Stroke};

pub fn apply(ctx: &egui::Context, rgb: Accent) {
    let id = Id::unique("applied-desktop-accent");
    if ctx.data(|data| data.get_temp::<Accent>(id)) == Some(rgb) {
        return;
    }
    ctx.data_mut(|data| data.insert_temp(id, rgb));
    let accent = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.style_mut_of(theme, |style| {
            let visuals = &mut style.visuals;
            let text = if theme == egui::Theme::Light {
                Color32::from_rgb(30, 40, 57)
            } else {
                Color32::from_rgb(225, 229, 237)
            };
            let focus = contrasting_accent(accent, visuals.panel_fill, 3.0);
            visuals.selection.bg_fill = accent;
            visuals.selection.stroke = Stroke::new(1.0, foreground(accent));
            visuals.hyperlink_color = contrasting_accent(accent, visuals.panel_fill, 4.5);
            visuals.text_cursor.stroke = Stroke::new(2.0, focus);
            // Neutral resting buttons, with accent-tinted interaction states.
            // Keep their foreground readable even on frameless menu buttons.
            let base = match theme {
                egui::Theme::Light => Color32::from_gray(238),
                egui::Theme::Dark => Color32::from_gray(45),
            };
            visuals.widgets.inactive.fg_stroke.color = text;
            for (widget, amount, width) in [
                (&mut visuals.widgets.hovered, 0.12, 1.0),
                (&mut visuals.widgets.active, 0.22, 1.5),
                (&mut visuals.widgets.open, 0.18, 1.0),
            ] {
                let fill = mix(base, accent, amount);
                widget.bg_fill = fill;
                widget.weak_bg_fill = fill;
                widget.bg_stroke = Stroke::new(width, focus);
                widget.fg_stroke.color = text;
            }
        });
    }
    ctx.request_repaint();
}
fn mix(a: Color32, b: Color32, amount: f32) -> Color32 {
    let [r, g, b] = std::array::from_fn(|index| {
        (f32::from(a[index]) * (1.0 - amount) + f32::from(b[index]) * amount).round() as u8
    });
    Color32::from_rgb(r, g, b)
}
fn luminance(color: Color32) -> f32 {
    let [r, g, b] = [color.r(), color.g(), color.b()].map(|value| {
        let value = f32::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}
fn contrast(a: Color32, b: Color32) -> f32 {
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
fn foreground(background: Color32) -> Color32 {
    if contrast(background, Color32::BLACK) >= contrast(background, Color32::WHITE) {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}
fn contrasting_accent(accent: Color32, background: Color32, minimum: f32) -> Color32 {
    let toward = foreground(background);
    for step in 0..=20 {
        let candidate = mix(accent, toward, step as f32 / 20.0);
        if contrast(candidate, background) >= minimum {
            return candidate;
        }
    }
    toward
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_and_focus_remain_readable_with_bright_and_dark_desktop_accents() {
        for rgb in [
            [61, 174, 233],
            [255, 255, 255],
            [0, 0, 0],
            [255, 255, 0],
            [80, 0, 120],
        ] {
            let accent = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            assert!(contrast(accent, foreground(accent)) >= 4.5);
            for bg in [Color32::WHITE, Color32::from_gray(27)] {
                assert!(contrast(contrasting_accent(accent, bg, 3.0), bg) >= 3.0);
            }
        }
    }
    #[test]
    fn runtime_updates_apply_to_both_themes_and_fresh_window_contexts() {
        let ctx = egui::Context::default();
        for rgb in [[61, 174, 233], [190, 70, 130]] {
            apply(&ctx, rgb);
            for theme in [egui::Theme::Light, egui::Theme::Dark] {
                let style = ctx.style_of(theme);
                assert_eq!(
                    style.visuals.selection.bg_fill,
                    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
                );
                assert!(contrast(style.visuals.hyperlink_color, style.visuals.panel_fill) >= 4.5);
            }
        }
        let reopened = egui::Context::default();
        apply(&reopened, [190, 70, 130]);
        assert_eq!(
            reopened
                .style_of(egui::Theme::Light)
                .visuals
                .selection
                .bg_fill,
            Color32::from_rgb(190, 70, 130)
        );
    }
}
