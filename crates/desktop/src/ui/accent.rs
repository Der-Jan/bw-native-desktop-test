//! Interaction colors follow the desktop; SVG branding is deliberately separate.
use crate::{
    services::desktop_appearance::{Accent, Appearance, Palette},
    theme::ThemePreference,
};
use eframe::egui::{self, Color32, Id, Stroke};
fn color(rgb: Accent) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}
fn palette(appearance: Appearance, dark: bool) -> Palette {
    appearance
        .palette
        .filter(|p| p.dark() == dark)
        .unwrap_or_else(|| Palette::fallback(dark, appearance.accent))
}
pub fn view_text(ctx: &egui::Context, appearance: Appearance) -> Color32 {
    color(palette(appearance, ctx.theme() == egui::Theme::Dark).view_text)
}
pub fn apply(ctx: &egui::Context, appearance: Appearance, preference: ThemePreference) {
    let native = ctx.system_theme();
    let system = appearance
        .system_dark
        .map(|dark| {
            if dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            }
        })
        .unwrap_or_else(|| {
            if cfg!(target_os = "linux")
                && std::env::var("XDG_CURRENT_DESKTOP")
                    .is_ok_and(|v| v.split(':').any(|s| s.eq_ignore_ascii_case("KDE")))
            {
                egui::Theme::Light
            } else {
                native.unwrap_or(egui::Theme::Light)
            }
        });
    let effective = match preference {
        ThemePreference::System => system,
        ThemePreference::Light => egui::Theme::Light,
        ThemePreference::Dark => egui::Theme::Dark,
    };
    ctx.set_theme(effective);
    let id = Id::unique("applied-desktop-palette");
    let key = (appearance, preference, effective);
    if ctx.data(|data| data.get_temp::<(Appearance, ThemePreference, egui::Theme)>(id)) == Some(key)
    {
        return;
    }
    ctx.data_mut(|data| data.insert_temp(id, key));
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let p = palette(appearance, theme == egui::Theme::Dark);
        ctx.style_mut_of(theme, |style| {
            let mut visuals = match theme {
                egui::Theme::Light => egui::Visuals::light(),
                egui::Theme::Dark => egui::Visuals::dark(),
            };
            let window = color(p.window);
            let text = color(p.window_text);
            let view = color(p.view);
            let button = color(p.button);
            let border = mix(window, text, 0.25);
            let accent = color(appearance.accent);
            let focus = contrasting_accent(color(p.focus), window, 3.0);
            visuals.panel_fill = window;
            visuals.window_fill = window;
            visuals.window_stroke = Stroke::new(1.0, border);
            visuals.extreme_bg_color = view;
            visuals.text_edit_bg_color = Some(view);
            visuals.code_bg_color = color(p.alternate_view);
            visuals.faint_bg_color = color(p.alternate_view);
            visuals.override_text_color = Some(text);
            visuals.weak_text_color = Some(color(p.disabled_text));
            visuals.warn_fg_color = color(p.warning);
            visuals.error_fg_color = color(p.negative);
            visuals.widgets.noninteractive.bg_fill = window;
            visuals.widgets.noninteractive.weak_bg_fill = window;
            visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text);
            visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border);
            visuals.selection.bg_fill = accent;
            let selected_text = color(p.selection_text);
            visuals.selection.stroke = Stroke::new(
                1.0,
                if contrast(accent, selected_text) >= 4.5 {
                    selected_text
                } else {
                    foreground(accent)
                },
            );
            visuals.hyperlink_color = contrasting_accent(color(p.link), window, 4.5);
            visuals.text_cursor.stroke = Stroke::new(2.0, focus);
            visuals.widgets.inactive.bg_fill = button;
            visuals.widgets.inactive.weak_bg_fill = button;
            visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border);
            visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, color(p.button_text));
            for (widget, amount, width) in [
                (&mut visuals.widgets.hovered, 0.12, 1.0),
                (&mut visuals.widgets.active, 0.22, 1.5),
                (&mut visuals.widgets.open, 0.18, 1.0),
            ] {
                let fill = mix(button, color(p.hover), amount);
                widget.bg_fill = fill;
                widget.weak_bg_fill = fill;
                widget.bg_stroke = Stroke::new(width, focus);
                widget.fg_stroke.color = color(p.button_text);
            }
            visuals.window_shadow = egui::epaint::Shadow::NONE;
            visuals.popup_shadow = egui::epaint::Shadow::NONE;
            style.visuals = visuals;
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
            apply(
                &ctx,
                Appearance {
                    accent: rgb,
                    ..Default::default()
                },
                ThemePreference::System,
            );
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
        apply(
            &reopened,
            Appearance {
                accent: [190, 70, 130],
                ..Default::default()
            },
            ThemePreference::System,
        );
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

#[cfg(test)]
mod palette_tests {
    use super::*;
    #[test]
    fn kde_light_wins_over_a_conflicting_native_hint_and_manual_modes_still_work() {
        let ctx = egui::Context::default();
        let palette = Palette::fallback(false, [61, 174, 233]);
        let appearance = Appearance {
            accent: palette.selection,
            system_dark: Some(false),
            palette: Some(palette),
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                system_theme: Some(egui::Theme::Dark),
                ..Default::default()
            },
            |_| apply(&ctx, appearance, ThemePreference::System),
        );
        output.textures_delta.clear();
        assert_eq!(ctx.theme(), egui::Theme::Light);
        let style = ctx.global_style();
        assert_eq!(style.visuals.panel_fill, color(palette.window));
        assert_eq!(style.visuals.text_edit_bg_color, Some(color(palette.view)));
        assert_eq!(style.visuals.text_color(), color(palette.window_text));
        apply(&ctx, appearance, ThemePreference::Dark);
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        assert_eq!(
            ctx.global_style().visuals.panel_fill,
            color(Palette::fallback(true, appearance.accent).window)
        );
        apply(&ctx, appearance, ThemePreference::Light);
        assert_eq!(ctx.theme(), egui::Theme::Light);
    }
}
