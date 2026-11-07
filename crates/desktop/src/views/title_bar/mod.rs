mod dropdown;
mod handler;
pub(super) mod window_chrome;

use iced::{
    Alignment, Background, Border, Color, Element, Fill, Padding, Shadow,
    widget::{button, container, mouse_area, row, text},
};

use crate::{
    app::{Outcome, Overlay, UpdateCtx, View},
    services::menu,
    theme::AppTheme,
};

use self::window_chrome::{chrome_button, icon as chrome_icon};

const DROPDOWN_WIDTH: f32 = 280.0;
const SUBMENU_WIDTH: f32 = 220.0;
const ITEM_PADDING: Padding = Padding {
    top: 8.0,
    right: 16.0,
    bottom: 8.0,
    left: 16.0,
};
const PANEL_RADIUS: f32 = 8.0;
pub const TITLE_BAR_HEIGHT: f32 = 32.0;
const WINDOW_BTN_WIDTH: f32 = 46.0;

#[derive(Debug, Clone)]
pub enum TitleBarMessage {
    TopLevelClicked(usize),
    TopLevelHovered(usize),
    /// Leaf-item click. Carries the resolved [`menu::MenuAction`] directly —
    /// the renderer looks the action up via [`menu::MenuEntry::effective_children`]
    /// at the click site so dispatch is action-driven rather than index-driven.
    /// Works uniformly for top-level leaves and submenu entries (static and
    /// dynamic).
    ItemAction(menu::MenuAction),
    SubMenuHovered(usize, usize),
    DismissMenu,
    MinimizeClicked,
    MaximizeClicked,
    CloseClicked,
    DragStart,
    ResizeEdge(iced::window::Direction),
}

// ── Events ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum WindowAction {
    Minimize,
    Maximize,
    Close,
    Drag,
    ResizeEdge(iced::window::Direction),
}

#[derive(Debug, Clone, derive_more::From)]
pub enum TitleBarEvent {
    /// User invoked a menu entry mapping to a global `MenuAction`.
    MenuInvoked(crate::services::menu::MenuAction),
    /// User clicked a window-chrome button or started a drag/resize.
    Window(WindowAction),
}

/// Stateless view — menu open/close lives in the app-level [`Overlay`] cell.
/// The struct is kept (rather than making everything free functions) so the
/// compositional MVU pattern stays uniform across views.
pub struct TitleBarView;

impl TitleBarView {
    pub fn new() -> Self {
        Self
    }
}

impl View for TitleBarView {
    type Message = TitleBarMessage;
    type Event = TitleBarEvent;

    /// Compositional MVU update. Title bar has no async work so the returned
    /// task is always `Task::none()`; the event carries the domain fact.
    fn update(&mut self, msg: TitleBarMessage, ctx: UpdateCtx<'_>) -> Outcome<Self> {
        // Read the previously open top-level menu, if any, so toggle-on-
        // same-index can close and clicks on a different index can switch.
        let current_menu = match *ctx.open_overlay {
            Some(Overlay::TitleBarMenu { menu, .. }) => Some(menu),
            _ => None,
        };

        // Menu-navigation arms mutate overlay state; button-click arms bubble
        // a `TitleBarEvent::Window(...)` action.
        let window_action = match msg {
            TitleBarMessage::TopLevelClicked(i) => {
                *ctx.open_overlay = if current_menu == Some(i) {
                    None
                } else {
                    Some(Overlay::TitleBarMenu {
                        menu: i,
                        submenu: None,
                    })
                };
                return Outcome::None;
            }
            TitleBarMessage::DismissMenu => {
                *ctx.open_overlay = None;
                return Outcome::None;
            }
            TitleBarMessage::TopLevelHovered(i) => {
                *ctx.open_overlay = Some(Overlay::TitleBarMenu {
                    menu: i,
                    submenu: None,
                });
                return Outcome::None;
            }
            TitleBarMessage::SubMenuHovered(menu, item) => {
                let submenu = if item == usize::MAX { None } else { Some(item) };
                *ctx.open_overlay = Some(Overlay::TitleBarMenu { menu, submenu });
                return Outcome::None;
            }
            TitleBarMessage::ItemAction(action) => {
                *ctx.open_overlay = None;
                return Outcome::event(TitleBarEvent::MenuInvoked(action));
            }
            TitleBarMessage::MinimizeClicked => WindowAction::Minimize,
            TitleBarMessage::MaximizeClicked => WindowAction::Maximize,
            TitleBarMessage::CloseClicked => WindowAction::Close,
            TitleBarMessage::DragStart => WindowAction::Drag,
            TitleBarMessage::ResizeEdge(dir) => WindowAction::ResizeEdge(dir),
        };
        Outcome::event(TitleBarEvent::Window(window_action))
    }

    /// Draw the title bar: menu labels on the left, window buttons on the right.
    /// Each menu label wraps a `DropDown` that shows its panel via iced's overlay
    /// system. All window-state inputs (maximized, derived menu gating, open
    /// menu/submenu indices) ride through [`crate::app::RenderCtx`].
    ///
    /// On platforms that defer to a native system title bar (macOS without
    /// `DEV_BOTH_MENUS`), this renders a flat header-coloured spacer so the
    /// app-content column still sits below the OS chrome at the same y.
    fn view<'a>(
        &'a self,
        ctx: &crate::app::RenderCtx<'a>,
    ) -> Element<'a, TitleBarMessage, AppTheme> {
        if !menu::should_use_custom_menu_bar() {
            return container(iced::widget::Space::new())
                .width(Fill)
                .height(TITLE_BAR_HEIGHT)
                .style(|theme: &AppTheme| {
                    container::Style::default().background(theme.colors.header_bg)
                })
                .into();
        }

        let menu_items: Vec<Element<'_, TitleBarMessage, AppTheme>> = ctx
            .menu
            .sections
            .iter()
            .enumerate()
            .map(|(i, section)| {
                let is_open = ctx.open_title_bar_menu == Some(i);
                let btn = button(text(section.label.clone()).size(14).color(Color::WHITE))
                    .on_press(TitleBarMessage::TopLevelClicked(i))
                    .padding([4, 10])
                    .style(move |theme: &AppTheme, status| {
                        let bg = if is_open {
                            theme.colors.titlebar_btn_hover
                        } else {
                            match status {
                                button::Status::Hovered => theme.colors.titlebar_btn_hover,
                                _ => Color::TRANSPARENT,
                            }
                        };
                        button::Style {
                            background: Some(Background::Color(bg)),
                            text_color: Color::WHITE,
                            border: Border::default(),
                            shadow: Shadow::default(),
                            snap: false,
                        }
                    });

                let panel = dropdown::menu_panel(
                    &section.entries,
                    i,
                    ctx.open_title_bar_submenu,
                    ctx.accounts,
                    &ctx.menu_state,
                    ctx.colors,
                );
                let dd: Element<'_, TitleBarMessage, AppTheme> =
                    crate::components::drop_down::DropDown::new(btn, panel, is_open)
                        .on_dismiss(TitleBarMessage::DismissMenu)
                        .alignment(crate::components::drop_down::Alignment::BelowLeft)
                        .width(iced::Length::Shrink)
                        .offset(0.0)
                        .into();

                if ctx.open_title_bar_menu.is_some() {
                    mouse_area(dd)
                        .on_enter(TitleBarMessage::TopLevelHovered(i))
                        .into()
                } else {
                    dd
                }
            })
            .collect();

        let menu_row = row(menu_items).spacing(0).align_y(Alignment::Center);

        let minimize_btn = chrome_button(
            chrome_icon::MINIMIZE,
            chrome_icon::FONT,
            ctx.colors.titlebar_btn_hover,
            TitleBarMessage::MinimizeClicked,
        );
        let maximize_btn = chrome_button(
            if ctx.is_maximized {
                chrome_icon::RESTORE
            } else {
                chrome_icon::MAXIMIZE
            },
            chrome_icon::FONT,
            ctx.colors.titlebar_btn_hover,
            TitleBarMessage::MaximizeClicked,
        );
        let close_btn = chrome_button(
            chrome_icon::CLOSE,
            chrome_icon::FONT,
            ctx.colors.titlebar_close_hover,
            TitleBarMessage::CloseClicked,
        );

        let bar_content = row![
            menu_row,
            iced::widget::Space::new().width(Fill),
            minimize_btn,
            maximize_btn,
            close_btn,
        ]
        .spacing(0)
        .align_y(Alignment::Center)
        .padding(Padding::default().left(4));

        let bar = container(bar_content)
            .width(Fill)
            .height(TITLE_BAR_HEIGHT)
            .style(|theme: &AppTheme| {
                container::Style::default().background(theme.colors.header_bg)
            });

        mouse_area(bar).on_press(TitleBarMessage::DragStart).into()
    }
}

/// Wrap content with invisible resize handles on all edges.
pub use self::window_chrome::resize_wrapper;

#[cfg(test)]
mod tests_update {
    use super::*;
    use crate::{
        app::{App, Overlay},
        test_support::OutcomeExt,
    };

    #[tokio::test(flavor = "current_thread")]
    async fn top_level_clicked_opens_menu_when_none_open() {
        let mut app = App::test();
        let mut view = TitleBarView::new();
        view.update(TitleBarMessage::TopLevelClicked(1), app.update_ctx())
            .expect_none();
        assert_eq!(
            app.open_overlay,
            Some(Overlay::TitleBarMenu {
                menu: 1,
                submenu: None,
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn top_level_clicked_same_index_toggles_closed() {
        let mut app = App::test();
        app.open_overlay = Some(Overlay::TitleBarMenu {
            menu: 0,
            submenu: None,
        });
        let mut view = TitleBarView::new();
        view.update(TitleBarMessage::TopLevelClicked(0), app.update_ctx())
            .expect_none();
        assert_eq!(app.open_overlay, None);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn top_level_clicked_different_index_switches() {
        let mut app = App::test();
        app.open_overlay = Some(Overlay::TitleBarMenu {
            menu: 0,
            submenu: Some(2),
        });
        let mut view = TitleBarView::new();
        view.update(TitleBarMessage::TopLevelClicked(1), app.update_ctx())
            .expect_none();
        // Switching the top-level menu resets `submenu` — the new menu is
        // freshly opened with no submenu hover-state carried over.
        assert_eq!(
            app.open_overlay,
            Some(Overlay::TitleBarMenu {
                menu: 1,
                submenu: None,
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dismiss_menu_clears_overlay() {
        let mut app = App::test();
        app.open_overlay = Some(Overlay::TitleBarMenu {
            menu: 0,
            submenu: None,
        });
        let mut view = TitleBarView::new();
        view.update(TitleBarMessage::DismissMenu, app.update_ctx())
            .expect_none();
        assert_eq!(app.open_overlay, None);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submenu_hovered_max_clears_submenu_slot() {
        // `usize::MAX` is the sentinel for "no submenu hovered" — used when
        // the cursor enters the parent panel but is not over any submenu
        // row. Avoids needing a separate "submenu unhovered" message.
        let mut app = App::test();
        app.open_overlay = Some(Overlay::TitleBarMenu {
            menu: 0,
            submenu: Some(3),
        });
        let mut view = TitleBarView::new();
        view.update(
            TitleBarMessage::SubMenuHovered(0, usize::MAX),
            app.update_ctx(),
        )
        .expect_none();
        assert_eq!(
            app.open_overlay,
            Some(Overlay::TitleBarMenu {
                menu: 0,
                submenu: None,
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn item_action_clears_overlay_and_emits_menu_invoked() {
        let mut app = App::test();
        app.open_overlay = Some(Overlay::TitleBarMenu {
            menu: 0,
            submenu: None,
        });
        let mut view = TitleBarView::new();
        let ev = view
            .update(
                TitleBarMessage::ItemAction(menu::MenuAction::Quit),
                app.update_ctx(),
            )
            .expect_event();
        assert!(matches!(
            ev,
            TitleBarEvent::MenuInvoked(menu::MenuAction::Quit)
        ));
        // Any click on a menu item dismisses the dropdown.
        assert_eq!(app.open_overlay, None);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn window_chrome_clicks_bubble_window_event() {
        let mut app = App::test();
        let mut view = TitleBarView::new();

        let ev = view
            .update(TitleBarMessage::MinimizeClicked, app.update_ctx())
            .expect_event();
        assert!(matches!(ev, TitleBarEvent::Window(WindowAction::Minimize)));

        let ev = view
            .update(TitleBarMessage::CloseClicked, app.update_ctx())
            .expect_event();
        assert!(matches!(ev, TitleBarEvent::Window(WindowAction::Close)));

        let ev = view
            .update(TitleBarMessage::DragStart, app.update_ctx())
            .expect_event();
        assert!(matches!(ev, TitleBarEvent::Window(WindowAction::Drag)));
    }
}
