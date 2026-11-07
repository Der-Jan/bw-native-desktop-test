use super::TrayAction;
use ksni::blocking::TrayMethods;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::broadcast;

struct LinuxTray {
    events: broadcast::Sender<TrayAction>,
    online: Arc<AtomicBool>,
    pixels: Vec<u8>,
    width: i32,
    height: i32,
    labels: [String; 3],
    icon_name: String,
    theme_path: String,
}
impl ksni::Tray for LinuxTray {
    fn id(&self) -> String {
        "com.bitwarden.desktop.next".into()
    }
    fn title(&self) -> String {
        "Bitwarden".into()
    }
    fn activate(&mut self, _: i32, _: i32) {
        let _ = self.events.send(TrayAction::ToggleShowHide);
    }
    fn icon_name(&self) -> String {
        self.icon_name.clone()
    }
    fn icon_theme_path(&self) -> String {
        self.theme_path.clone()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: self.width,
            height: self.height,
            data: self.pixels.clone(),
        }]
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: self.labels[0].clone(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.events.send(TrayAction::ToggleShowHide);
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: self.labels[1].clone(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.events.send(TrayAction::LockVault);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: self.labels[2].clone(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.events.send(TrayAction::Exit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
    fn watcher_offline(&self, _: ksni::OfflineReason) -> bool {
        self.online.store(false, Ordering::SeqCst);
        let _ = self.events.send(TrayAction::Unavailable);
        true
    }
    fn watcher_online(&self) {
        self.online.store(true, Ordering::SeqCst);
    }
}

pub struct TrayHandle {
    handle: ksni::blocking::Handle<LinuxTray>,
    online: Arc<AtomicBool>,
}
impl TrayHandle {
    pub fn available(&self) -> bool {
        self.online.load(Ordering::SeqCst) && !self.handle.is_closed()
    }
    pub fn resolve(&self, _: &tray_icon::menu::MenuId) -> Option<TrayAction> {
        None
    }
}
impl Drop for TrayHandle {
    fn drop(&mut self) {
        let _ = self.handle.shutdown();
    }
}
pub fn build() -> Option<TrayHandle> {
    let image = image::load_from_memory(include_bytes!(
        "../../../../../assets/tray/icon-template@2x.png"
    ))
    .ok()?
    .to_rgba8();
    let (width, height) = image.dimensions();
    let mut pixels = image.into_raw();
    // SNI pixmaps use network byte-order ARGB, not RGBA.
    for p in pixels.as_chunks_mut::<4>().0 {
        p.rotate_right(1);
    }
    let online = Arc::new(AtomicBool::new(true));
    let (icon_name, theme_path) = install_symbolic_icon().unwrap_or_default();
    let tray = LinuxTray {
        events: super::event_sender(),
        online: online.clone(),
        pixels,
        icon_name,
        theme_path,
        width: width as i32,
        height: height as i32,
        labels: [
            crate::fl!("tray-show-hide"),
            crate::fl!("tray-lock-vault"),
            crate::fl!("tray-exit"),
        ],
    };
    // zbus is also used by the keyring with its Tokio backend. Use the same
    // backend, and register on a plain thread so ksni can enter its own runtime
    // even when this call originates inside an application Tokio task.
    let registration = std::thread::spawn(move || tray.spawn()).join();
    match registration {
        Ok(Ok(handle)) => Some(TrayHandle { handle, online }),
        Ok(Err(error)) => {
            tracing::warn!(%error,"StatusNotifier tray unavailable");
            None
        }
        Err(_) => {
            tracing::warn!("StatusNotifier registration thread failed");
            None
        }
    }
}

/// KDE's SVG loader replaces ColorScheme-Text with the panel foreground color.
/// Other SNI hosts can use the monochrome ARGB fallback.
fn install_symbolic_icon() -> Option<(String, String)> {
    let directory = crate::paths::initialized_data_dir()?.join("tray-icons");
    std::fs::create_dir_all(&directory).ok()?;
    std::fs::write(
        directory.join("bitwarden-symbolic.svg"),
        include_bytes!("../../../../../assets/tray/bitwarden-symbolic.svg"),
    )
    .ok()?;
    Some((
        "bitwarden-symbolic".into(),
        directory.to_string_lossy().into_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksni::Tray;
    #[test]
    fn activation_menu_and_watcher_loss_reach_the_app() {
        let (events, mut receiver) = broadcast::channel(8);
        let online = Arc::new(AtomicBool::new(true));
        let mut tray = LinuxTray {
            events,
            online: online.clone(),
            pixels: vec![255, 1, 2, 3],
            width: 1,
            height: 1,
            labels: ["Show".into(), "Lock".into(), "Exit".into()],
            icon_name: String::new(),
            theme_path: String::new(),
        };
        tray.activate(0, 0);
        assert!(matches!(
            receiver.try_recv().unwrap(),
            TrayAction::ToggleShowHide
        ));
        let menu = tray.menu();
        let ksni::MenuItem::Standard(lock) = &menu[2] else {
            panic!("lock action")
        };
        (lock.activate)(&mut tray);
        assert!(matches!(
            receiver.try_recv().unwrap(),
            TrayAction::LockVault
        ));
        tray.watcher_online();
        assert!(online.load(Ordering::SeqCst));
        assert!(tray.watcher_offline(ksni::OfflineReason::No));
        assert!(!online.load(Ordering::SeqCst));
        assert!(matches!(
            receiver.try_recv().unwrap(),
            TrayAction::Unavailable
        ));
        assert_eq!(tray.icon_pixmap()[0].data, vec![255, 1, 2, 3]);
    }
    /// Requires the caller's real desktop SNI host; ordinary CI has none.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires a running StatusNotifier host"]
    async fn registers_with_desktop_host() {
        crate::services::i18n::init();
        let tray = build().expect("desktop tray registration");
        assert!(tray.available());
    }
}
