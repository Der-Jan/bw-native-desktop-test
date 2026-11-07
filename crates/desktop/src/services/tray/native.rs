use super::TrayAction;
use std::collections::HashMap;

use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuId, MenuItem},
};

pub struct TrayHandle {
    // Retained: dropping would remove the OS tray entry.
    _icon: TrayIcon,
    actions: HashMap<MenuId, TrayAction>,
}

impl TrayHandle {
    pub fn available(&self) -> bool {
        true
    }
    /// Resolve a muda `MenuId` to a `TrayAction` if it belongs to the tray
    /// menu. The caller drains `muda::MenuEvent::receiver()` — a global
    /// singleton shared with the app's native menu — and dispatches by id.
    pub fn resolve(&self, id: &MenuId) -> Option<TrayAction> {
        self.actions.get(id).copied()
    }
}

/// Returns `None` on failure (e.g. Linux without an SNI host); tray-dependent
/// settings are all `Option::is_some` checked.
pub fn build() -> Option<TrayHandle> {
    let icon = match decode_icon() {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!(error = %e, "failed to decode tray icon");
            return None;
        }
    };

    let show_hide = MenuItem::new(crate::fl!("tray-show-hide"), true, None);
    let lock_vault = MenuItem::new(crate::fl!("tray-lock-vault"), true, None);
    let exit = MenuItem::new(crate::fl!("tray-exit"), true, None);
    let sep = tray_icon::menu::PredefinedMenuItem::separator();

    let menu = match Menu::with_items(&[&show_hide, &sep, &lock_vault, &exit]) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "failed to assemble tray menu");
            return None;
        }
    };

    let mut actions = HashMap::new();
    actions.insert(show_hide.id().clone(), TrayAction::ToggleShowHide);
    actions.insert(lock_vault.id().clone(), TrayAction::LockVault);
    actions.insert(exit.id().clone(), TrayAction::Exit);

    let builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_tooltip("Bitwarden")
        .with_menu_on_left_click(false);

    // Template image — NSStatusBar recolours for light/dark menubar.
    #[cfg(target_os = "macos")]
    let builder = builder.with_icon_as_template(true);

    let tray = match builder.build() {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "tray icon initialisation failed; tray features disabled");
            return None;
        }
    };

    Some(TrayHandle {
        _icon: tray,
        actions,
    })
}

pub fn install_event_handler(tx: tokio::sync::broadcast::Sender<TrayAction>) {
    use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            let _ = tx.send(TrayAction::ToggleShowHide);
        }
    }));
}

fn decode_icon() -> Result<tray_icon::Icon, Box<dyn std::error::Error>> {
    let img = image::load_from_memory(crate::assets::TRAY_ICON)?.to_rgba8();
    let (w, h) = img.dimensions();
    Ok(tray_icon::Icon::from_rgba(img.into_raw(), w, h)?)
}
