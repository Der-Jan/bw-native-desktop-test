//! Public desktop appearance settings, read off the UI thread.
use futures::Stream;
use std::pin::Pin;

pub type Accent = [u8; 3];
pub const FALLBACK_ACCENT: Accent = [22, 92, 225];

/// Portal notifications wake the app; no timer or per-frame config reads.
pub fn accent_updates() -> Pin<Box<dyn Stream<Item = Accent> + Send>> {
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
                    let color = tokio::task::spawn_blocking(kde_accent)
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or(FALLBACK_ACCENT);
                    let _ = sender.send(color).await;
                    return;
                };
                let mut changes = Box::pin(theme.subscribe());
                loop {
                    let reader = theme.clone();
                    let color = tokio::task::spawn_blocking(move || {
                        reader
                            .get_accent()
                            .ok()
                            .and_then(|c| from_components([c.red, c.green, c.blue]))
                            .or_else(kde_accent)
                            .unwrap_or(FALLBACK_ACCENT)
                    })
                    .await
                    .unwrap_or(FALLBACK_ACCENT);
                    if sender.send(color).await.is_err() || changes.next().await.is_none() {
                        break;
                    }
                }
            },
        ))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::pin(futures::stream::once(async { FALLBACK_ACCENT }))
    }
}

#[cfg(target_os = "linux")]
fn kde_accent() -> Option<Accent> {
    if !std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|value| {
        value
            .split(':')
            .any(|part| part.eq_ignore_ascii_case("KDE"))
    }) {
        return None;
    }
    for command in ["kreadconfig6", "kreadconfig5"] {
        let Ok(output) = std::process::Command::new(command)
            .args([
                "--file",
                "kdeglobals",
                "--group",
                "Colors:Selection",
                "--key",
                "BackgroundNormal",
            ])
            .output()
        else {
            continue;
        };
        if output.status.success()
            && let Some(color) = std::str::from_utf8(&output.stdout).ok().and_then(parse_rgb)
        {
            return Some(color);
        }
    }
    None
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
