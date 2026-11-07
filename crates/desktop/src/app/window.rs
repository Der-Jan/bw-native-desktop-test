//! Per-window metadata stored on `App`, keyed by `iced::window::Id` and
//! mutated on resize / maximize / fullscreen transitions.

#[derive(Debug)]
pub struct WindowInfo {
    pub kind: WindowKind,
    pub fullscreen: bool,
    pub maximized: bool,
    pub always_on_top: bool,
    /// Last reported logical size. Updated on `window::Event::Resized`.
    pub size: iced::Size,
}

impl WindowInfo {
    pub fn new(kind: WindowKind, size: iced::Size) -> Self {
        Self {
            kind,
            fullscreen: false,
            maximized: false,
            always_on_top: false,
            size,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    Main,
    About,
    SshApproval,
    /// Launcher surface exists only between an explicit summon and dismissal.
    Magnify,
}
