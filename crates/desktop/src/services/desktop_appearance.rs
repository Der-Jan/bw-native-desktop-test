//! Public desktop appearance settings, read off the UI thread.
use futures::Stream;
use std::pin::Pin;

pub type Accent = [u8; 3];
pub const FALLBACK_ACCENT: Accent = [22, 92, 225];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub window: Accent,
    pub window_text: Accent,
    pub view: Accent,
    pub view_text: Accent,
    pub alternate_view: Accent,
    pub button: Accent,
    pub button_text: Accent,
    pub disabled_text: Accent,
    pub selection: Accent,
    pub selection_text: Accent,
    pub link: Accent,
    pub focus: Accent,
    pub hover: Accent,
    pub negative: Accent,
    pub warning: Accent,
}
impl Palette {
    pub fn dark(self) -> bool {
        let [r, g, b] = self.window;
        299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b) < 128000
    }
    pub fn fallback(dark: bool, accent: Accent) -> Self {
        let (window, view, alternate, button, text, weak) = if dark {
            (
                [49, 54, 59],
                [35, 38, 41],
                [42, 46, 50],
                [49, 54, 59],
                [239, 240, 241],
                [170, 175, 180],
            )
        } else {
            (
                [239, 240, 241],
                [255, 255, 255],
                [247, 247, 247],
                [252, 252, 252],
                [35, 38, 41],
                [112, 125, 138],
            )
        };
        Self {
            window,
            window_text: text,
            view,
            view_text: text,
            alternate_view: alternate,
            button,
            button_text: text,
            disabled_text: weak,
            selection: accent,
            selection_text: [255; 3],
            link: accent,
            focus: accent,
            hover: accent,
            negative: [218, 68, 83],
            warning: [246, 116, 0],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Appearance {
    pub accent: Accent,
    pub system_dark: Option<bool>,
    pub palette: Option<Palette>,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            accent: FALLBACK_ACCENT,
            system_dark: None,
            palette: None,
        }
    }
}
/// Portal notifications wake the app; no timer or per-frame config reads.
pub fn updates() -> Pin<Box<dyn Stream<Item = Appearance> + Send>> {
    #[cfg(target_os = "linux")]
    {
        use futures::{SinkExt, StreamExt};
        use std::sync::Arc;
        Box::pin(super::broadcast_stream::channel(
            4,
            |mut sender| async move {
                let theme = tokio::task::spawn_blocking(system_theme::SystemTheme::new)
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .map(Arc::new);
                let Some(theme) = theme else {
                    let color = tokio::task::spawn_blocking(|| read_appearance(None))
                        .await
                        .unwrap_or_default();
                    let _ = sender.send(color).await;
                    return;
                };
                let mut changes = Box::pin(theme.subscribe());
                loop {
                    let reader = theme.clone();
                    let color = tokio::task::spawn_blocking(move || read_appearance(Some(&reader)))
                        .await
                        .unwrap_or_default();
                    if sender.send(color).await.is_err() || changes.next().await.is_none() {
                        break;
                    }
                }
            },
        ))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::pin(futures::stream::once(async { Appearance::default() }))
    }
}

#[cfg(target_os = "linux")]
fn read_appearance(theme: Option<&system_theme::SystemTheme>) -> Appearance {
    let palette = kde_palette();
    let accent = palette
        .map(|p| p.selection)
        .or_else(|| {
            theme
                .and_then(|t| t.get_accent().ok())
                .and_then(|c| from_components([c.red, c.green, c.blue]))
        })
        .unwrap_or(FALLBACK_ACCENT);
    let system_dark = palette.map(Palette::dark).or_else(|| {
        theme
            .and_then(|t| t.get_scheme().ok())
            .map(|s| s == system_theme::ThemeScheme::Dark)
    });
    Appearance {
        accent,
        system_dark,
        palette,
    }
}
#[cfg(any(target_os = "linux", test))]
type ColorSettings = std::collections::HashMap<String, std::collections::HashMap<String, String>>;
#[cfg(target_os = "linux")]
fn kde_palette() -> Option<Palette> {
    // Read the host's effective palette, even when the app uses a separate
    // XDG_CONFIG_HOME/profile. Generic color-scheme hints can describe GTK
    // rather than KDE, so actual KDE color roles take precedence.
    let connection = zbus::blocking::Connection::session().ok();
    if let Some(connection) = connection {
        let namespaces = [
            "org.kde.kdeglobals.Colors:Window",
            "org.kde.kdeglobals.Colors:View",
            "org.kde.kdeglobals.Colors:Button",
            "org.kde.kdeglobals.Colors:Selection",
        ];
        if let Ok(message) = connection.call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "ReadAll",
            &(&namespaces[..],),
        ) && let Ok(groups) = message.body().deserialize::<std::collections::HashMap<
            String,
            std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        >>() {
            let settings = groups
                .into_iter()
                .map(|(group, values)| {
                    (
                        group,
                        values
                            .into_iter()
                            .filter_map(|(key, value)| {
                                String::try_from(value).ok().map(|value| (key, value))
                            })
                            .collect(),
                    )
                })
                .collect();
            if let Some(palette) = palette_from_settings(&settings) {
                return Some(palette);
            }
        }
    }
    if !std::env::var("XDG_CURRENT_DESKTOP")
        .is_ok_and(|v| v.split(':').any(|s| s.eq_ignore_ascii_case("KDE")))
    {
        return None;
    }
    let config = read_config(&dirs::config_dir()?.join("kdeglobals"))?;
    let mut settings = ColorSettings::new();
    if let Some(name) = config
        .get("General")
        .and_then(|g| g.get("ColorScheme"))
        .filter(|s| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
        })
    {
        let mut roots = vec![];
        if let Some(root) = dirs::data_dir() {
            roots.push(root);
        }
        roots.extend(std::env::split_paths(
            &std::env::var_os("XDG_DATA_DIRS")
                .unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
        ));
        for root in roots {
            if let Some(scheme) =
                read_config(&root.join("color-schemes").join(format!("{name}.colors")))
            {
                settings = scheme;
                break;
            }
        }
    }
    for (group, values) in config {
        settings.entry(group).or_default().extend(values);
    }
    palette_from_settings(&settings)
}
#[cfg(target_os = "linux")]
fn read_config(path: &std::path::Path) -> Option<ColorSettings> {
    use std::io::Read;
    let mut contents = String::new();
    std::fs::File::open(path)
        .ok()?
        .take(1024 * 1024)
        .read_to_string(&mut contents)
        .ok()?;
    let mut result = ColorSettings::new();
    let mut group = String::new();
    for line in contents
        .lines()
        .map(str::trim)
        .filter(|s| !s.starts_with(['#', ';']))
    {
        if line.starts_with('[') && line.ends_with(']') {
            group = line
                .trim_end_matches("[$i]")
                .trim_start_matches('[')
                .trim_end_matches(']')
                .to_owned();
        } else if let Some((key, value)) = line.split_once('=') {
            result.entry(group.clone()).or_default().insert(
                key.trim().trim_end_matches("[$i]").to_owned(),
                value.trim().to_owned(),
            );
        }
    }
    Some(result)
}
#[cfg(any(target_os = "linux", test))]
fn palette_from_settings(settings: &ColorSettings) -> Option<Palette> {
    let read = |group: &str, key: &str| {
        settings
            .get(&format!("org.kde.kdeglobals.{group}"))
            .or_else(|| settings.get(group))
            .and_then(|values| values.get(key))
            .and_then(|v| parse_rgb(v))
    };
    let window = read("Colors:Window", "BackgroundNormal")?;
    let text = read("Colors:Window", "ForegroundNormal")?;
    let view = read("Colors:View", "BackgroundNormal")?;
    let view_text = read("Colors:View", "ForegroundNormal")?;
    let accent = read("Colors:Selection", "BackgroundNormal").unwrap_or(FALLBACK_ACCENT);
    let mut palette = Palette::fallback(false, accent);
    palette.window = window;
    palette.window_text = text;
    palette.view = view;
    palette.view_text = view_text;
    palette.alternate_view = read("Colors:View", "BackgroundAlternate").unwrap_or(view);
    palette.button = read("Colors:Button", "BackgroundNormal").unwrap_or(window);
    palette.button_text = read("Colors:Button", "ForegroundNormal").unwrap_or(text);
    palette.disabled_text =
        read("Colors:Window", "ForegroundInactive").unwrap_or(palette.disabled_text);
    palette.selection_text =
        read("Colors:Selection", "ForegroundNormal").unwrap_or(palette.selection_text);
    palette.link = read("Colors:View", "ForegroundLink").unwrap_or(accent);
    palette.focus = read("Colors:View", "DecorationFocus").unwrap_or(accent);
    palette.hover = read("Colors:Button", "DecorationHover").unwrap_or(accent);
    palette.negative = read("Colors:View", "ForegroundNegative").unwrap_or(palette.negative);
    palette.warning = read("Colors:View", "ForegroundNeutral").unwrap_or(palette.warning);
    Some(palette)
}

#[cfg(any(target_os = "linux", test))]
fn from_components(components: [f32; 3]) -> Option<Accent> {
    components
        .iter()
        .all(|c| c.is_finite() && (0.0..=1.0).contains(c))
        .then(|| components.map(|c| (c * 255.0).round() as u8))
}
#[cfg(any(target_os = "linux", test))]
fn parse_rgb(value: &str) -> Option<Accent> {
    let mut parts = value.trim().split(',');
    let result = [
        parts.next()?.trim().parse().ok()?,
        parts.next()?.trim().parse().ok()?,
        parts.next()?.trim().parse().ok()?,
    ];
    parts.next().is_none().then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portal_unset_and_invalid_values_do_not_replace_the_fallback() {
        assert_eq!(
            from_components([61.0 / 255.0, 174.0 / 255.0, 233.0 / 255.0]),
            Some([61, 174, 233])
        );
        for values in [
            [-1.0; 3],
            [f32::NAN, 0.0, 0.0],
            [f32::INFINITY, 0.0, 0.0],
            [1.1, 0.0, 0.0],
        ] {
            assert_eq!(from_components(values), None);
        }
    }
    #[test]
    fn kde_colors_require_exactly_three_valid_byte_components() {
        assert_eq!(parse_rgb(" 61, 174,233\n"), Some([61, 174, 233]));
        for value in ["", "61,174", "256,0,0", "-1,0,0", "1,2,3,4", "invalid"] {
            assert_eq!(parse_rgb(value), None);
        }
    }
}

#[cfg(test)]
mod palette_tests {
    use super::*;
    #[test]
    fn kde_roles_supply_the_actual_window_and_view_palette() {
        let mut settings = ColorSettings::new();
        for (group, key, value) in [
            ("Colors:Window", "BackgroundNormal", "239,240,241"),
            ("Colors:Window", "ForegroundNormal", "35,38,41"),
            ("Colors:View", "BackgroundNormal", "255,255,255"),
            ("Colors:View", "ForegroundNormal", "35,38,41"),
            ("Colors:Button", "BackgroundNormal", "252,252,252"),
            ("Colors:Selection", "BackgroundNormal", "61,174,233"),
        ] {
            settings
                .entry(format!("org.kde.kdeglobals.{group}"))
                .or_default()
                .insert(key.into(), value.into());
        }
        let palette = palette_from_settings(&settings).unwrap();
        assert!(!palette.dark());
        assert_eq!(palette.window, [239, 240, 241]);
        assert_eq!(palette.view, [255, 255, 255]);
        assert_eq!(palette.button, [252, 252, 252]);
        settings
            .get_mut("org.kde.kdeglobals.Colors:Window")
            .unwrap()
            .insert("BackgroundNormal".into(), "49,54,59".into());
        assert!(palette_from_settings(&settings).unwrap().dark());
        settings
            .get_mut("org.kde.kdeglobals.Colors:View")
            .unwrap()
            .insert("BackgroundNormal".into(), "invalid".into());
        assert!(palette_from_settings(&settings).is_none());
    }
}
