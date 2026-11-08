//! Wayland tray support without a hidden helper window.
//!
//! A native surface belongs to one eframe run; application authority outlives
//! it. eframe reuses winit's event loop (run_and_return). While no surface exists,
//! the UI thread waits on the same wake condition used by Core's bounded queue.
//! It still handles lock timers, logout, instance activation and SSH requests.
use super::*;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Condvar, Mutex},
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Main,
    Approval,
    Magnify,
}
#[derive(Default)]
struct Wake {
    state: Mutex<(u64, Option<egui::Context>)>,
    changed: Condvar,
}
impl Wake {
    fn signal(&self) {
        let ctx = {
            let mut state = self.state.lock().unwrap();
            state.0 = state.0.wrapping_add(1);
            self.changed.notify_one();
            state.1.clone()
        };
        if let Some(ctx) = ctx {
            ctx.request_repaint();
        }
    }
    fn context(&self, ctx: Option<egui::Context>) {
        self.state.lock().unwrap().1 = ctx;
    }
    fn epoch(&self) -> u64 {
        self.state.lock().unwrap().0
    }
    fn wait(&self, epoch: u64) {
        let mut state = self.state.lock().unwrap();
        while state.0 == epoch {
            state = self.changed.wait(state).unwrap();
        }
    }
}
struct Shell {
    app: Rc<RefCell<Option<Application>>>,
}
impl eframe::App for Shell {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(app) = self.app.borrow_mut().as_mut() {
            eframe::App::logic(app, ctx, frame);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if let Some(app) = self.app.borrow_mut().as_mut() {
            eframe::App::ui(app, ui, frame);
        }
    }
}
pub fn run() -> eframe::Result {
    let app: Rc<RefCell<Option<Application>>> = Rc::new(RefCell::new(None));
    let wake = Arc::new(Wake::default());
    let mut surface = Surface::Main;
    loop {
        let size = match surface {
            Surface::Main => [1200., 840.],
            Surface::Approval => [500., 350.],
            Surface::Magnify => [600., 420.],
        };
        let title = match surface {
            Surface::Main => "Bitwarden",
            Surface::Approval => "Allow SSH authentication",
            Surface::Magnify => "Bitwarden quick search",
        };
        let mut viewport = egui::ViewportBuilder::default()
            .with_title(title)
            .with_app_id("com.bitwarden.desktop.next")
            .with_inner_size(size)
            .with_icon(crate::assets::window_icon().unwrap_or_default());
        if surface == Surface::Main {
            viewport = viewport.with_min_inner_size([800., 620.]);
        } else {
            viewport = viewport.with_always_on_top();
        }
        let options = eframe::NativeOptions {
            viewport,
            renderer: eframe::Renderer::Glow,
            run_and_return: true,
            ..Default::default()
        };
        let shared = app.clone();
        let creation_wake = wake.clone();
        eframe::run_native(
            "bitwarden-desktop-next",
            options,
            Box::new(move |cc| {
                creation_wake.context(Some(cc.egui_ctx.clone()));
                if shared.borrow().is_none() {
                    let notify = creation_wake.clone();
                    *shared.borrow_mut() = Some(Application::new_with_wake(
                        cc,
                        Arc::new(move || notify.signal()),
                    )?);
                } else {
                    let mut shared = shared.borrow_mut();
                    let app = shared.as_mut().unwrap();
                    app.font_description = typography::install(&cc.egui_ctx)?;
                    egui_extras::install_image_loaders(&cc.egui_ctx);
                    app.restore_offset = Some(app.list_offset);
                    app.icon_textures.clear();
                    app.ssh_seen = None;
                }
                let mut app_ref = shared.borrow_mut();
                let app = app_ref.as_mut().unwrap();
                app.surface = surface;
                services::screenshot_protection::apply(cc, !app.core.settings.allow_screenshots);
                #[cfg(target_os = "macos")]
                {
                    app.core.native_menu =
                        services::menu::attach_menu(0, &services::menu::menu_tree());
                }
                #[cfg(target_os = "windows")]
                {
                    use raw_window_handle::HasWindowHandle;
                    if let Ok(handle) = cc.window_handle() {
                        if let raw_window_handle::RawWindowHandle::Win32(handle) = handle.as_raw() {
                            app.core.native_menu = services::menu::attach_menu(
                                handle.hwnd.get() as u64,
                                &services::menu::menu_tree(),
                            );
                        }
                    }
                }
                drop(app_ref);
                Ok(Box::new(Shell { app: shared }))
            }),
        )?;
        wake.context(None);
        // Poll before choosing the next surface. No busy loop and no renderer
        // are needed while the tray is the application's only desktop presence.
        loop {
            let epoch = wake.epoch();
            {
                let mut shared = app.borrow_mut();
                let app = shared.as_mut().unwrap();
                app.core.poll();
                if let Some(action) = app.core.window_action.take() {
                    app.core.menu_action(action);
                }
                if app.clear_epoch != app.core.clear_epoch {
                    app.cleanup(&egui::Context::default());
                }
                if app.core.ready_to_quit() {
                    app.core.ssh_agent.stop();
                    return Ok(());
                }
                if app.core.quit_requested {
                }
                // Finish durable cleanup without reopening.
                else if !app.core.hidden {
                    surface = Surface::Main;
                    break;
                } else if app.core.ssh_approval.is_some() {
                    surface = Surface::Approval;
                    break;
                } else if app.core.magnify_requested {
                    surface = Surface::Magnify;
                    break;
                }
            }
            wake.wait(epoch);
        }
    }
}
