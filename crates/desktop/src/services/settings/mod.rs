//! App-wide user settings, read once from `data/settings.json`. Behavioral
//! branches re-read `self.settings.<field>` at event time (never cached), so
//! live changes apply without a restart. See `docs/decisions.md`.

use std::{collections::HashMap, io::BufReader};

use bitwarden_core::UserId;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{services::preferences::UserPreferences, theme::ThemePreference};

/// Sentinel for "follow the OS locale". Stored in `language` when no explicit
/// language has been picked.
pub const LANGUAGE_SYSTEM: &str = "";

/// UI zoom multiplier in tenths (10 = 1.0×). Stepping is exact integer
/// arithmetic — no floating-point drift after a few +/- presses. Self-clamps
/// to `MIN..=MAX` on construction and on deserialize, so a hand-edited
/// `zoom_factor: 0` in `settings.json` snaps to `MIN` instead of bricking the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ZoomFactor(u8);

impl ZoomFactor {
    pub const DEFAULT: u8 = 10;
    pub const MIN: u8 = 5;
    pub const MAX: u8 = 30;

    pub fn new(tenths: u8) -> Self {
        Self(tenths.clamp(Self::MIN, Self::MAX))
    }

    /// Tenths value as an f32 multiplier ready for iced's `scale_factor` callback.
    pub fn scale(self) -> f32 {
        f32::from(self.0) / 10.0
    }

    /// Bump one tenth up, saturating at [`Self::MAX`]. Returns `true` if changed.
    pub fn step_in(&mut self) -> bool {
        if self.0 < Self::MAX {
            self.0 += 1;
            true
        } else {
            false
        }
    }

    /// Drop one tenth down, saturating at [`Self::MIN`]. Returns `true` if changed.
    pub fn step_out(&mut self) -> bool {
        if self.0 > Self::MIN {
            self.0 -= 1;
            true
        } else {
            false
        }
    }

    /// Restore to [`Self::DEFAULT`]. Returns `true` if changed.
    pub fn reset(&mut self) -> bool {
        if self.0 != Self::DEFAULT {
            self.0 = Self::DEFAULT;
            true
        } else {
            false
        }
    }
}

impl Default for ZoomFactor {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

impl<'de> Deserialize<'de> for ZoomFactor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        u8::deserialize(d).map(Self::new)
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub vault_filters: HashMap<UserId, crate::services::vault_navigation::VaultFilter>,
    pub collapsed_navigation_groups:
        std::collections::HashSet<crate::services::vault_navigation::NavigationGroup>,
    pub sidebar_collapsed: bool,
    pub vault_sort: crate::services::vault_sort::VaultSort,
    pub theme: ThemePreference,
    /// BCP-47 language tag (e.g. `"en"`, `"es"`) or [`LANGUAGE_SYSTEM`] for OS
    /// preferred. Plain string so new locales need only an `assets/i18n/<tag>/`
    /// directory, no code change.
    pub language: String,

    /// Stub (not wired) — shown in the Security tab.
    pub open_at_login: bool,

    /// Integrations; SSH agent is implemented, browser integration remains a stub.
    pub browser_integration: bool,
    pub browser_integration_fingerprint: bool,
    pub ssh_agent: bool,

    /// Stub — the real autotype engine hasn't been ported yet.
    pub autotype_enabled: bool,

    /// Vault and magnify rows render fetched favicons (via
    /// [`crate::services::favicon`]) when `true`; an initial-letter
    /// circle otherwise. Read by the item-list / magnify-row renderers
    /// straight off `RenderCtx` — no startup wiring needed.
    pub show_favicons: bool,

    /// Master switch: keep the tray icon visible even when the app is in the
    /// foreground.
    pub show_tray_icon: bool,
    /// Clicking the minimize button hides the window (not the taskbar
    /// minimize). Auto-creates the tray if not already shown.
    pub minimize_to_tray: bool,
    /// Clicking the OS close button hides the window instead of exiting.
    /// Auto-creates the tray if not already shown.
    pub close_to_tray: bool,

    /// Stub — macOS-only `NSApplication.activationPolicy` toggle.
    pub always_show_dock: bool,
    /// Whether the user permits screen capture / screen recording. When
    /// `false` (the [`bool`] default), the main window is excluded from
    /// captures via [`crate::services::screenshot_protection`] —
    /// secure-by-default, diverging from the official client which
    /// ships protection off. Toggling on (= flipping protection off)
    /// opens a confirm-still-visible dialog with auto-revert. Linux has
    /// no upstream protocol, so the toggle no-ops there after a
    /// one-shot toast.
    pub allow_screenshots: bool,

    /// Per-user preferences. Populated lazily; persisted alongside the app-wide
    /// fields so unlock-with-PIN, clipboard delay, etc. survive across launches.
    pub user_preferences: HashMap<UserId, UserPreferences>,

    /// UI scale fed into iced's per-window `scale_factor` callback. Composes
    /// multiplicatively with OS DPI. Stepped from the View → Zoom in / out / reset menu.
    pub zoom_factor: ZoomFactor,
}

impl Settings {
    /// Current preferences for `uid`, or defaults if none are saved.
    pub fn preferences_for(&self, uid: &UserId) -> UserPreferences {
        self.user_preferences.get(uid).copied().unwrap_or_default()
    }

    // ── Persistence ───────────────────────────────────────────────────────

    /// Missing file → defaults. Malformed → defaults + `tracing::warn!`.
    pub fn load() -> Self {
        let path = crate::paths::data_dir().join("settings.json");
        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "settings.json unreadable; using defaults");
                return Self::default();
            }
        };
        match serde_json::from_reader(BufReader::new(file)) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "settings.json malformed; using defaults");
                Self::default()
            }
        }
    }

    /// Atomic write: serialize into a sibling `.tmp` and rename over the
    /// target. A crash during the write leaves the original file intact.
    // App-state tests must never write to an actual user profile. Serialization
    // is exercised separately with disposable settings values.
    #[cfg(test)]
    pub fn save(&self) {}

    #[cfg(not(test))]
    pub fn save(&self) {
        let path = crate::paths::data_dir().join("settings.json");
        let tmp_path = path.with_extension("json.tmp");
        let file = match std::fs::File::create(&tmp_path) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(path = %tmp_path.display(), error = %e, "settings.json write failed");
                return;
            }
        };
        if let Err(e) = serde_json::to_writer_pretty(file, self) {
            tracing::warn!(path = %tmp_path.display(), error = %e, "settings.json serialize failed");
            let _ = std::fs::remove_file(&tmp_path);
            return;
        }
        if let Err(e) = std::fs::rename(&tmp_path, &path) {
            tracing::warn!(path = %path.display(), error = %e, "settings.json rename failed");
            let _ = std::fs::remove_file(&tmp_path);
        }
    }

    /// Whether *any* tray feature is active — drives tray creation at startup.
    pub fn wants_tray(&self) -> bool {
        self.show_tray_icon || self.minimize_to_tray || self.close_to_tray
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obsolete_gpu_settings_are_ignored_without_losing_preferences() {
        let settings: Settings = serde_json::from_str(
            r#"{
                "hardware_acceleration": true,
                "duck_duck_go": true,
                "wgpu_backend_verified": "vulkan",
                "wgpu_backend_pending": "vulkan",
                "zoom_factor": 15,
                "language": "de"
            }"#,
        )
        .expect("existing settings remain readable");
        assert_eq!(settings.zoom_factor.scale(), 1.5);
        assert_eq!(settings.language, "de");
        let saved = serde_json::to_value(settings).expect("settings serialize");
        assert!(saved.get("hardware_acceleration").is_none());
        assert!(saved.get("duck_duck_go").is_none());
        assert!(saved.get("wgpu_backend_verified").is_none());
        assert!(saved.get("wgpu_backend_pending").is_none());
    }

    #[test]
    fn navigation_roundtrip_keeps_account_choices_and_collapsed_groups() {
        use crate::services::vault_navigation::*;
        let first = UserId::new_v4();
        let second = UserId::new_v4();
        let mut settings = Settings::default();
        let filter = VaultFilter::default()
            .with_scope(VaultScope::Personal)
            .with_category(Some(bitwarden_vault::CipherType::SshKey))
            .with_section(VaultSection::Favorites)
            .with_folder(FolderFilter::Folder(bitwarden_vault::FolderId::new_v4()));
        settings.vault_filters.insert(first, filter);
        settings
            .vault_filters
            .insert(second, VaultFilter::default());
        settings
            .collapsed_navigation_groups
            .insert(NavigationGroup::Folders);
        settings.sidebar_collapsed = true;
        let encoded = serde_json::to_string(&settings).unwrap();
        let restored: Settings = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored.vault_filters[&first], filter);
        assert_eq!(restored.vault_filters[&second], VaultFilter::default());
        assert!(
            restored
                .collapsed_navigation_groups
                .contains(&NavigationGroup::Folders)
        );
        assert!(restored.sidebar_collapsed);
        assert!(!encoded.contains("search"));
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert!(old.vault_filters.is_empty());
        assert!(!old.sidebar_collapsed);
    }

    // ── ZoomFactor::new — clamping ─────────────────────────────────────────

    #[test]
    fn new_clamps_below_min_to_min() {
        assert_eq!(ZoomFactor::new(0).0, ZoomFactor::MIN);
        assert_eq!(ZoomFactor::new(ZoomFactor::MIN - 1).0, ZoomFactor::MIN);
    }

    #[test]
    fn new_clamps_above_max_to_max() {
        assert_eq!(ZoomFactor::new(ZoomFactor::MAX + 1).0, ZoomFactor::MAX);
        assert_eq!(ZoomFactor::new(255).0, ZoomFactor::MAX);
    }

    #[test]
    fn new_passes_through_in_range() {
        assert_eq!(ZoomFactor::new(ZoomFactor::DEFAULT).0, ZoomFactor::DEFAULT);
        assert_eq!(ZoomFactor::new(ZoomFactor::MIN).0, ZoomFactor::MIN);
        assert_eq!(ZoomFactor::new(ZoomFactor::MAX).0, ZoomFactor::MAX);
    }

    #[test]
    fn default_is_one_x_scale() {
        assert_eq!(ZoomFactor::default().scale(), 1.0);
        assert_eq!(ZoomFactor::default().0, ZoomFactor::DEFAULT);
    }

    // ── step_in / step_out / reset bool returns ────────────────────────────

    #[test]
    fn step_in_at_max_returns_false_and_does_not_overflow() {
        let mut z = ZoomFactor::new(ZoomFactor::MAX);
        assert!(!z.step_in());
        assert_eq!(z.0, ZoomFactor::MAX);
    }

    #[test]
    fn step_in_below_max_increments_and_returns_true() {
        let mut z = ZoomFactor::new(ZoomFactor::MAX - 1);
        assert!(z.step_in());
        assert_eq!(z.0, ZoomFactor::MAX);
    }

    #[test]
    fn step_out_at_min_returns_false_and_does_not_underflow() {
        let mut z = ZoomFactor::new(ZoomFactor::MIN);
        assert!(!z.step_out());
        assert_eq!(z.0, ZoomFactor::MIN);
    }

    #[test]
    fn step_out_above_min_decrements_and_returns_true() {
        let mut z = ZoomFactor::new(ZoomFactor::MIN + 1);
        assert!(z.step_out());
        assert_eq!(z.0, ZoomFactor::MIN);
    }

    #[test]
    fn reset_at_default_returns_false() {
        let mut z = ZoomFactor::default();
        assert!(!z.reset());
        assert_eq!(z.0, ZoomFactor::DEFAULT);
    }

    #[test]
    fn reset_from_non_default_restores_and_returns_true() {
        let mut z = ZoomFactor::new(ZoomFactor::MAX);
        assert!(z.reset());
        assert_eq!(z.0, ZoomFactor::DEFAULT);
    }

    // ── Deserialize re-routes through new() ────────────────────────────────

    #[test]
    fn deserialize_clamps_below_min() {
        // The "hand-edited 0 doesn't brick the UI" guarantee.
        let z: ZoomFactor = serde_json::from_str("0").expect("u8 deserializes");
        assert_eq!(z.0, ZoomFactor::MIN);
    }

    #[test]
    fn deserialize_clamps_above_max() {
        let z: ZoomFactor = serde_json::from_str("255").expect("u8 deserializes");
        assert_eq!(z.0, ZoomFactor::MAX);
    }

    #[test]
    fn deserialize_in_range_round_trips() {
        let z: ZoomFactor =
            serde_json::from_str(&ZoomFactor::DEFAULT.to_string()).expect("u8 deserializes");
        assert_eq!(z, ZoomFactor::default());
    }

    #[test]
    fn serialize_is_bare_integer() {
        // `#[serde(transparent)]` ensures we don't write `{"0": 10}` or similar.
        let s = serde_json::to_string(&ZoomFactor::default()).expect("serializes");
        assert_eq!(s, ZoomFactor::DEFAULT.to_string());
    }
}
