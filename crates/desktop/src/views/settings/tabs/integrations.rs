use iced::{
    Element, Fill,
    widget::{Space, column, text},
};

use crate::{
    components::inputs,
    fl,
    services::preferences::SshPromptBehavior,
    theme::{AppColors, AppTheme},
};

use super::{
    super::{SettingChange, SettingsSnapshot},
    SECTION_GAP_PX, setting_checkbox, settings_section,
};

pub fn view<'a>(
    snap: &'a SettingsSnapshot,
    colors: &'a AppColors,
    agent: &'a crate::services::ssh_agent::AgentRuntime,
) -> Element<'a, SettingChange, AppTheme> {
    let browser = setting_checkbox(
        snap.settings.browser_integration,
        fl!("settings-integrations-browser-enable"),
        SettingChange::BrowserIntegration,
    );
    let browser_fp = setting_checkbox(
        snap.settings.browser_integration_fingerprint,
        fl!("settings-integrations-browser-fingerprint"),
        SettingChange::BrowserIntegrationFingerprint,
    );

    let ssh = setting_checkbox(
        snap.settings.ssh_agent,
        fl!("settings-integrations-ssh-enable"),
        SettingChange::SshAgent,
    );
    let ssh_prompt = inputs::select_field(
        fl!("settings-integrations-ssh-prompt"),
        Some(snap.prefs.ssh_prompt_behavior),
        SshPromptBehavior::ALL.to_vec(),
        |v: &SshPromptBehavior| v.label(),
        SettingChange::SshPromptBehavior,
        colors,
    );

    column![
        settings_section(
            fl!("settings-integrations-browser"),
            column![browser, browser_fp].spacing(10),
            colors,
        ),
        Space::new().height(SECTION_GAP_PX),
        settings_section(
            fl!("settings-integrations-ssh"),
            column![
                ssh,
                ssh_prompt,
                iced::widget::text(agent.status()).size(14),
                iced::widget::text(agent.path().display().to_string()).size(12),
                crate::components::buttons::secondary(text(fl!("ssh-agent-copy-socket")))
                    .on_press(SettingChange::CopySshSocket),
                text(agent.shell_setup()).size(12),
                crate::components::buttons::secondary(text(fl!("ssh-agent-copy-setup")))
                    .on_press(SettingChange::CopySshSetup),
                iced::widget::text(fl!("ssh-agent-help")).size(12),
                iced::widget::text(
                    if snap.prefs.ssh_prompt_behavior == SshPromptBehavior::Never {
                        fl!("ssh-agent-unattended")
                    } else {
                        fl!("ssh-agent-protected")
                    }
                )
                .size(12),
            ]
            .spacing(16),
            colors,
        ),
    ]
    .width(Fill)
    .into()
}
