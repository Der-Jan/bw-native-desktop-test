//! Startup typography. Let KConfig resolve KDE's user/system defaults; Iced
//! already applies monitor DPI, so only the logical font size belongs here.
use iced::{
    Font,
    font::{Family, Weight},
};
use std::sync::LazyLock;

pub struct Typography {
    pub font: Font,
    pub scale: f32,
}
impl Typography {
    pub fn text_size(&self, design: u32) -> f32 {
        // Approximate the preferred 90% typography, rounded to whole points.
        // Keep captions readable. Undo the desktop layout scale here because
        // Iced applies it later; monitor DPI and explicit zoom remain separate.
        let points = (design as f32 * self.scale * 0.9 * 72.0 / 96.0)
            .round()
            .max(9.0);
        points * (96.0 / 72.0) / self.scale
    }
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            font: Font::DEFAULT,
            scale: 1.0,
        }
    }
}
static CURRENT: LazyLock<Typography> = LazyLock::new(discover);
pub fn current() -> &'static Typography {
    &CURRENT
}

// UI fixtures deliberately retain the embedded font, independent of the test host.
#[cfg(test)]
fn discover() -> Typography {
    Typography {
        font: Font {
            family: Family::Name("Inter 18pt"),
            weight: Weight::Medium,
            ..Font::DEFAULT
        },
        ..Typography::default()
    }
}

#[cfg(not(test))]
fn discover() -> Typography {
    #[cfg(target_os = "linux")]
    if std::env::var("XDG_CURRENT_DESKTOP")
        .is_ok_and(|s| s.split(':').any(|d| d.eq_ignore_ascii_case("KDE")))
    {
        for program in ["kreadconfig6", "kreadconfig5"] {
            let Ok(output) = std::process::Command::new(program)
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
            if output.status.success()
                && let Ok(value) = std::str::from_utf8(&output.stdout)
                && let Some((family, scale)) = parse_kde_font(value)
            {
                tracing::info!(font_family = %family, desktop_base_logical_pixels = 14.0 * scale, "Using KDE typography");
                // Iced Font requires a static family name. One allocation for
                // the process lifetime; configuration is read once at startup.
                let family = Box::leak(family.into_boxed_str());
                return Typography {
                    font: Font {
                        family: Family::Name(family),
                        weight: Weight::Normal,
                        ..Font::DEFAULT
                    },
                    scale,
                };
            }
        }
        tracing::debug!("KDE font unavailable; using system sans-serif");
    }
    Typography::default()
}

#[cfg(any(target_os = "linux", test))]
fn parse_kde_font(value: &str) -> Option<(String, f32)> {
    let mut fields = value.trim().split(',');
    let family = fields.next()?.trim();
    let points = fields.next()?.parse::<f32>().ok()?;
    if family.is_empty() || !points.is_finite() || !(6.0..=96.0).contains(&points) {
        return None;
    }
    // Existing body text is 14 logical pixels. Qt points use 96/72 pixels;
    // never apply the physical monitor DPI here a second time.
    Some((family.to_owned(), points * (96.0 / 72.0) / 14.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kde_points_map_to_logical_body_size_without_monitor_dpi() {
        let (family, scale) = parse_kde_font("Noto Sans,11,-1,5,50,0,0,0,0,0\n").unwrap();
        assert_eq!(family, "Noto Sans");
        assert!((14.0 * scale - 14.666667).abs() < 0.0001);
        assert!((parse_kde_font("Noto Sans,21").unwrap().1 - 2.0).abs() < 0.0001);
    }
    #[test]
    fn compact_kde_fonts_have_whole_point_sizes_and_readable_captions() {
        let typography = Typography {
            scale: parse_kde_font("Noto Sans,11").unwrap().1,
            ..Typography::default()
        };
        for (design, points) in [(12, 9.0), (14, 10.0), (16, 11.0), (18, 13.0), (28, 20.0)] {
            let actual = typography.text_size(design) * typography.scale * 72.0 / 96.0;
            assert!((actual - points).abs() < 0.0001);
        }
    }

    #[test]
    fn invalid_or_pixel_only_configuration_keeps_fallback() {
        for value in [
            "",
            ",11",
            "Noto Sans,-1",
            "Noto Sans,NaN",
            "Noto Sans,inf",
            "Noto Sans,0",
            "Noto Sans,999",
        ] {
            assert!(parse_kde_font(value).is_none(), "{value}");
        }
    }
}
