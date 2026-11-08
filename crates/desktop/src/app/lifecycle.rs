//! `App::new` (construction) + `App::subscription` (event sources).

use std::{collections::HashMap, sync::Arc};

use iced::{Subscription, Task};

use crate::{
    components::sidebar::SidebarState,
    domain::{Screen, UserId},
    services::{clipboard::ClipboardManager, sdk::ClientManager, settings::Settings},
    views::magnify,
};

use super::{
    App, MAIN_WINDOW_SIZE, Message, SystemMessage, ThemeState, ViewCache, Views, WindowMessage,
    helpers::main_window_platform_specific,
    window::{WindowInfo, WindowKind},
};

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let settings = Settings::load();
        let user_theme = settings.theme;

        // Apply the persisted language preference before any `fl!()` call
        // resolves user-visible strings. Empty string = "follow OS locale"
        // (already handled by `i18n::init()` in `main`).
        if !settings.language.is_empty()
            && let Ok(tag) = settings.language.parse()
        {
            crate::services::i18n::set_language(tag);
        }

        // Must run *before* any menu or tray icon is built — both crates'
        // `set_event_handler` are one-shot.
        crate::services::menu::install_event_handler();
        crate::services::tray::install_event_handler();
        // Global hotkey registration failure (e.g. Wayland) is logged
        // internally; Magnify is then just unreachable.
        crate::services::global_hotkey::install_event_handler();
        // Spawn the session-timeout driver in iced's tokio runtime. Idle
        // (parked on `pending()`) until the first user enrolls. Owned by
        // App; aborted on drop.
        let session_timeout = crate::services::session_timeout::SessionTimeout::new();

        // `--autostart` forces the app to launch hidden in the tray. Force
        // the tray to build in that case even if no tray settings are on,
        // otherwise the hidden window would have no entry point.
        let autostart = std::env::args().any(|a| a == "--autostart");

        let mut tray = None;
        if settings.wants_tray() || autostart {
            tray = crate::services::tray::build();
            if tray.is_none() {
                tracing::warn!("tray requested but failed to initialise");
            }
        }

        // When `--autostart` is set and the tray initialised, open hidden so
        // the user sees only the tray. `visible: false` flows through winit's
        // `with_visible(false)` — no flash, iced still owns the id and keeps
        // firing `view()`. Toggling later is a cheap `Mode::Windowed`.
        let start_hidden = autostart && tray.is_some();
        let (main_id, open_task) = iced::window::open(iced::window::Settings {
            size: MAIN_WINDOW_SIZE,
            min_size: Some(iced::Size::new(800.0, 750.0)),
            decorations: crate::services::menu::should_use_native_title_bar(),
            visible: !start_hidden,
            // Required for tray "close to tray": without this, the OS-sent
            // `CloseRequested` event would close the window before our
            // `WindowAction::Close` handler can choose to hide instead.
            // The About window keeps the default `true` (never hides).
            exit_on_close_request: false,
            platform_specific: main_window_platform_specific(),
            icon: crate::assets::window_icon(),
            ..Default::default()
        });
        let mut windows = HashMap::new();
        windows.insert(main_id, WindowInfo::new(WindowKind::Main, MAIN_WINDOW_SIZE));

        // No native launcher surface until summoned: Wayland ignores visible:false.
        let magnify_id = iced::window::Id::unique();

        // Discover users in `<workspace-root>/data/` and open one SQLite DB
        // per user. The `ClientManagerLoaded` handler `take()`s the manager
        // out of the slot — Arc<Mutex<Option<_>>> so `Message` keeps
        // deriving `Clone` without making `ClientManager` itself cloneable.
        let load_task = Task::perform(
            async { std::sync::Arc::new(std::sync::Mutex::new(Some(ClientManager::load().await))) },
            |slot| Message::System(SystemMessage::ClientManagerLoaded(slot)),
        );

        // Favicon service. Every user points at the cloud default for now;
        // see `docs/todo.md` "Show favicons" for the per-user `icons_url` swap.
        let favicon = crate::services::favicon::FaviconService::new(Arc::new(|_uid: &UserId| {
            "https://icons.bitwarden.net".to_string()
        }));

        let sidebar = SidebarState {
            mode: if settings.sidebar_collapsed {
                crate::components::sidebar::SidebarMode::Collapsed
            } else {
                crate::components::sidebar::SidebarMode::Expanded
            },
            collapsed_groups: settings.collapsed_navigation_groups.clone(),
            ..SidebarState::default()
        };
        let mut app = Self {
            active_user: None,
            presentation_generation: 0,
            storage_warning: None,
            pending_logouts: Default::default(),
            sync_status: Default::default(),
            exit_after_logout: false,
            client_manager: ClientManager::empty(),
            settings,

            screen: Screen::Loading,
            sidebar,

            views: Views::new(),

            windows,
            main_window: main_id,
            main_window_focused: true,
            main_window_hidden: start_hidden,
            main_window_suspended: false,
            magnify: magnify::MagnifyView::new(magnify_id),

            theme: ThemeState::new(user_theme),
            menu: crate::services::menu::menu_tree(),
            native_menu: None,
            main_window_raw_id: None,
            tray,

            clipboard: ClipboardManager::new(),
            favicon,
            session_timeout,
            ssh_agent: crate::services::ssh_agent::AgentRuntime::new(),
            ssh_approval: None,
            ssh_approval_window: None,
            shutting_down: false,

            open_overlay: None,
            toasts: Vec::new(),

            cache: ViewCache::default(),
        };

        let agent_task = app.refresh_ssh_agent();

        // Re-apply the persisted screen-capture setting once the main
        // window is alive. Skip the work entirely when the user hasn't
        // opted in (the default) — a no-op call would still pay for the
        // window-thread round-trip otherwise.
        let screenshot_task = if app.settings.allow_screenshots {
            Task::none()
        } else {
            crate::services::screenshot_protection::apply(main_id, true).discard()
        };

        (
            app,
            Task::batch([
                open_task.map(|id| Message::Window(WindowMessage::Opened(id))),
                load_task,
                screenshot_task,
                agent_task,
            ]),
        )
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let close_sub =
            iced::window::close_events().map(|id| Message::Window(WindowMessage::Closed(id)));

        // Routed through `event::listen_with` (not `keyboard::listen`) so the
        // `window::Id` is provided natively. `Subscription::map` requires a
        // non-capturing closure, and we need the id bound into the emitted
        // message for daemon multi-window dispatch.
        let event_sub = iced::event::listen_with(|event, status, id| match event {
            iced::Event::Keyboard(ev) => {
                Some(Message::Window(WindowMessage::KeyPressed(id, ev, status)))
            }
            // Mouse button presses count as session-timeout activity. Cursor
            // moves are intentionally skipped — they fire ~per-frame while
            // the cursor is in the window and would prevent any lock from
            // ever firing.
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_)) => {
                Some(Message::Window(WindowMessage::MouseInput(id)))
            }
            iced::Event::Window(iced::window::Event::Resized(size)) => {
                Some(Message::Window(WindowMessage::Resized(id, size)))
            }
            iced::Event::Window(iced::window::Event::CloseRequested) => {
                Some(Message::Window(WindowMessage::CloseRequested(id)))
            }
            iced::Event::Window(iced::window::Event::Focused) => {
                Some(Message::Window(WindowMessage::Focused(id)))
            }
            iced::Event::Window(iced::window::Event::Unfocused) => {
                Some(Message::Window(WindowMessage::Unfocused(id)))
            }
            _ => None,
        });

        // muda's `MenuEvent::receiver()` is global — both the native app menu
        // and the tray context menu dispatch to it. The handler filters by
        // `MenuId` to decide which one fired.
        let muda_sub = if self.native_menu.is_some() || self.tray.is_some() {
            Subscription::run(crate::services::menu::muda_event_stream)
                .map(|ev| Message::System(SystemMessage::MudaEvent(ev)))
        } else {
            Subscription::none()
        };

        let tray_sub = if self.tray.is_some() {
            Subscription::run(crate::services::tray::click_stream)
                .map(|action| Message::System(SystemMessage::TrayClick(action)))
        } else {
            Subscription::none()
        };

        let theme_sub = match &self.theme.system {
            Some(s) => Subscription::run_with(s.clone(), |st| st.subscribe())
                .map(|_| Message::System(SystemMessage::ThemeChanged)),
            None => Subscription::none(),
        };

        // Second-launch wake-up: listener bound once inside this stream, kept
        // alive across `update()` cycles because iced hashes the subscription
        // identity from the `fn` pointer.
        let wake_sub = Subscription::run(crate::services::instance_lock::wake_stream)
            .map(|_| Message::System(SystemMessage::InstanceWakeRequested));

        // OS session state (screen lock, suspend). Bound once; the platform
        // module owns the OS-side observer registration.
        let session_sub = Subscription::run(session_events::event_stream)
            .map(|ev| Message::System(SystemMessage::SessionEvent(ev)));

        // Session-timeout driver. Sleeps until the soonest enrolled deadline
        // and pushes a tick when it elapses; the App handler re-checks each
        // user and applies lock/log-out as needed. Idle (parked on
        // `pending()`) when no user has a finite timeout.
        let session_timeout_sub = crate::services::broadcast_stream::subscription_from_sender(
            "session-timeout",
            self.session_timeout.tick_sender(),
        )
        .map(|_| Message::System(SystemMessage::SessionTimeoutCheck));

        // Idle when OS-side hotkey registration failed (Wayland, missing
        // permissions) — the stream terminates and the subscription stays
        // dormant.
        let magnify_sub = Subscription::run(crate::services::global_hotkey::event_stream)
            .map(|_| Message::Magnify(crate::views::magnify::MagnifyMessage::HotkeyPressed));

        // Favicon fetch completions. Each message flips one row from globe
        // to real icon by virtue of arriving.
        let favicon_sub = crate::services::broadcast_stream::subscription_from_sender(
            "favicon",
            self.favicon.event_sender(),
        )
        .map(Into::into);

        let ssh_sub = crate::services::broadcast_stream::subscription_from_sender(
            "ssh-agent",
            self.ssh_agent.events().clone(),
        )
        .map(Message::SshAgent);
        Subscription::batch([
            ssh_sub,
            close_sub,
            event_sub,
            muda_sub,
            tray_sub,
            theme_sub,
            wake_sub,
            session_sub,
            session_timeout_sub,
            favicon_sub,
            magnify_sub,
        ])
    }
}
