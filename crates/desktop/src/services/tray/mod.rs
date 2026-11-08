//! Platform tray adapters. Linux uses a real StatusNotifier service, independent
//! of the Iced/winit event loop; macOS/Windows retain tray-icon's native adapter.
use futures::Stream;
use std::sync::OnceLock;
use tokio::sync::broadcast;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod native;
#[cfg(target_os = "linux")]
pub use linux::{TrayHandle, build};
#[cfg(not(target_os = "linux"))]
pub use native::{TrayHandle, build};

#[derive(Clone, Copy, Debug)]
pub enum TrayAction {
    ToggleShowHide,
    LockVault,
    Exit,
    Unavailable,
}
static EVENTS: OnceLock<broadcast::Receiver<TrayAction>> = OnceLock::new();
static SENDER: OnceLock<broadcast::Sender<TrayAction>> = OnceLock::new();
fn event_sender() -> broadcast::Sender<TrayAction> {
    SENDER
        .get_or_init(|| {
            let (tx, rx) = broadcast::channel(32);
            let _ = EVENTS.set(rx);
            tx
        })
        .clone()
}
pub fn install_event_handler() {
    let sender = event_sender();
    #[cfg(not(target_os = "linux"))]
    native::install_event_handler(sender);
    #[cfg(target_os = "linux")]
    let _ = sender;
}
pub fn click_stream() -> impl Stream<Item = TrayAction> {
    super::broadcast_stream::from_once_lock(&EVENTS, "tray")
}
