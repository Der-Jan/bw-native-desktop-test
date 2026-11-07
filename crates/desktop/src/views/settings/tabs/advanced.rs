use iced::{
    Element, Fill,
    widget::{Space, column},
};

use crate::{
    fl,
    theme::{AppColors, AppTheme},
};

use super::{
    super::{SettingChange, SettingsSnapshot},
    SECTION_GAP_PX, setting_checkbox, settings_section,
};

pub fn view<'a>(
    snap: &'a SettingsSnapshot,
    colors: &'a AppColors,
) -> Element<'a, SettingChange, AppTheme> {
    let tray_enabled = setting_checkbox(
        snap.settings.show_tray_icon,
        fl!("settings-advanced-tray-enable"),
        SettingChange::TrayEnabled,
    );
    let min_to_tray = setting_checkbox(
        snap.settings.minimize_to_tray,
        fl!("settings-advanced-minimize-to-tray"),
        SettingChange::MinimizeToTray,
    );
    let close_to_tray = setting_checkbox(
        snap.settings.close_to_tray,
        fl!("settings-advanced-close-to-tray"),
        SettingChange::CloseToTray,
    );

    let always_dock = setting_checkbox(
        snap.settings.always_show_dock,
        fl!("settings-advanced-always-show-dock"),
        SettingChange::AlwaysShowDock,
    );
    let allow_screenshots = setting_checkbox(
        snap.settings.allow_screenshots,
        fl!("settings-advanced-allow-screenshots"),
        SettingChange::AllowScreenshots,
    );

    column![
        settings_section(
            fl!("settings-advanced-tray"),
            column![tray_enabled, min_to_tray, close_to_tray].spacing(10),
            colors,
        ),
        Space::new().height(SECTION_GAP_PX),
        settings_section(
            fl!("settings-advanced-platform"),
            column![always_dock, allow_screenshots].spacing(10),
            colors,
        ),
    ]
    .width(Fill)
    .into()
}
