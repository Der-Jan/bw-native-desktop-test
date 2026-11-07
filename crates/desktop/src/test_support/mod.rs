//! Shared helpers for UI tests — snapshots (Layer C/D), view-update unit
//! tests (Layer B), Simulator interaction tests (Layer C). Layer A (pure
//! helpers) needs nothing from here.
//!
//! ## Setting up state
//!
//! Tests build an [`App`][crate::app::App] via [`App::test`] (private,
//! `#[cfg(test)]`-gated) and mutate its fields directly to set up scenario
//! state. Both [`App::render_ctx`] and [`App::update_ctx`] are then called to
//! obtain a `RenderCtx` / `UpdateCtx` for the view being tested.
//!
//! ```ignore
//! let mut app = App::test();
//! app.active_user = Some(uid);
//! app.sidebar.active_vault_filter = VaultFilter::default().with_section(VaultSection::Trash);
//! let out = view.update(msg, app.update_ctx());
//! ```
//!
//! ## Snapshot tests
//!
//! Use [`assert_themed_snapshots`] for the standard light + dark pair:
//!
//! ```ignore
//! #[tokio::test(flavor = "current_thread")]
//! async fn fingerprint_modal() {
//!     let mut state = FingerprintModal::default();
//!     state.open_with("apple banana carrot dolphin eagle".to_owned());
//!     test_support::settle_animations();
//!     test_support::assert_themed_snapshots(
//!         "fingerprint_modal",
//!         |rctx| state.view(rctx),
//!     );
//! }
//! ```
//!
//! Baselines land at `tests/snapshots/{name}-{theme}-tiny-skia.png` —
//! `iced_test` appends `-{renderer_name}` before the extension. First run
//! writes the baseline, subsequent runs do an exact-byte compare. Delete the
//! PNG to re-baseline a view.
//!
//! ## View-update unit tests
//!
//! All update tests need a tokio runtime because [`App::test`] spawns
//! [`crate::services::session_timeout::SessionTimeout`] and captures a
//! runtime handle in [`crate::services::favicon::FaviconService`]. Use
//! `#[tokio::test(flavor = "current_thread")]`:
//!
//! ```ignore
//! #[tokio::test(flavor = "current_thread")]
//! async fn submit_with_empty_name_is_ignored() {
//!     let mut view = NewFolderView::new();
//!     test_support::run_update(&mut view, NewFolderMessage::Submit).expect_none();
//! }
//! ```
//!
//! ## Simulator interaction tests
//!
//! Simulator runs no runtime — Tasks don't execute, subscriptions don't
//! fire. Pattern is: render, drive widget events, drain produced messages,
//! apply them to your state by hand.
//!
//! ```ignore
//! #[tokio::test(flavor = "current_thread")]
//! async fn close_button_emits_close() {
//!     let mut state = FingerprintModal::default();
//!     state.open_with("phrase".into());
//!     test_support::settle_animations();
//!     let mut app = App::test();
//!     let element = state.view(&app.render_ctx_main());
//!     let messages = test_support::drive_element(element, |ui| {
//!         ui.click("Close").unwrap();
//!     });
//!     test_support::assert_emitted(&messages, "Close",
//!         |m| matches!(m, FingerprintMessage::Close));
//! }
//! ```

pub mod fixtures;

use std::{path::Path, sync::OnceLock};

use iced::{Element, Settings};
use iced_test::simulator::Simulator;

use crate::{
    APP_FONT,
    app::{App, Outcome, RenderCtx, View},
    assets,
    components::{icons, toast::Toast},
    theme::AppTheme,
};

/// Select the same `tiny-skia` backend as the application for headless
/// snapshots. Idempotent and parallel-safe via `OnceLock`.
pub fn init() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        // SAFETY: `set_var` is unsafe under the 2024 edition because other
        // threads may be reading the environment. Running this inside
        // `OnceLock::get_or_init` keeps the mutation single-shot and
        // ordered-before any test that reads `ICED_TEST_BACKEND`.
        unsafe {
            std::env::set_var("ICED_TEST_BACKEND", "tiny-skia");
        }
    });
}

/// Compatibility hook for snapshots; dialogs now open immediately.
pub fn settle_animations() {}

/// Build a [`Simulator`] with the app's production fonts loaded so text and
/// icon glyphs render identically to the running binary.
pub fn simulator<'a, M>(
    element: impl Into<Element<'a, M, AppTheme>>,
) -> Simulator<'a, M, AppTheme> {
    let settings = Settings {
        fonts: vec![
            assets::FONT_MEDIUM.into(),
            assets::FONT_BOLD.into(),
            assets::BWI_FONT.into(),
            icons::FONT_BYTES.into(),
        ],
        default_font: APP_FONT,
        ..Settings::default()
    };
    Simulator::with_settings(settings, element)
}

/// Render the element with the given theme and compare against the PNG
/// baseline at `tests/snapshots/{path}-{renderer}.png`. Writes the baseline
/// on first run; panics on later runs if the bytes drift.
pub fn assert_snapshot<'a, M>(
    path: impl AsRef<Path>,
    theme: &AppTheme,
    element: impl Into<Element<'a, M, AppTheme>>,
) {
    let path = path.as_ref();
    let mut ui = simulator(element);
    let snapshot = ui.snapshot(theme).expect("rendered snapshot");
    let matched = snapshot.matches_image(path).expect("snapshot comparison");
    assert!(
        matched,
        "snapshot drift at {}; delete the .png to re-baseline",
        path.display(),
    );
}

/// Accept either a single message or an array of messages for
/// [`ViewTestExt::run`]. `T` and `[T; N]` both implement it; if a test
/// needs a `Vec<T>`, add an impl or collect into an array first.
///
/// Coherence holds because `M ≢ [M; N]` for any actual Rust type — there
/// is no recursive type that equals an array of itself.
pub trait IntoMessages<M> {
    fn into_messages(self) -> Vec<M>;
}

impl<M> IntoMessages<M> for M {
    fn into_messages(self) -> Vec<M> {
        vec![self]
    }
}

impl<M, const N: usize> IntoMessages<M> for [M; N] {
    fn into_messages(self) -> Vec<M> {
        self.into_iter().collect()
    }
}

/// Test-only extension on [`View`]. `view.run(...)` drives one or more
/// messages through `update()` with a fresh [`App::test`] context per
/// call. Returns the [`Outcome`] of the last message — chain
/// `.expect_*()` for single-message assertions; drop the result for
/// multi-message setup. The body is sync; `async` purely as a
/// compile-time signal that the test belongs in a
/// `#[tokio::test(flavor = "current_thread")]`. See the module
/// docstring for the full failure-mode rationale.
///
/// ```ignore
/// view.run(NewFolderMessage::NameChanged("Social".into()))
///     .await
///     .expect_none();
///
/// view.run([
///     NewFolderMessage::NameChanged("Social".into()),
///     NewFolderMessage::Submit,
/// ]).await;
/// ```
pub trait ViewTestExt: View {
    fn run(
        &mut self,
        msgs: impl IntoMessages<Self::Message>,
    ) -> impl std::future::Future<Output = Outcome<Self>>;

    /// Snapshot-test the view across the standard light + dark themes in
    /// one call. Calls [`init`], builds a fresh [`App::test`] per
    /// iteration with the iteration's theme installed, renders via the
    /// trait `view()` method, and writes baselines under
    /// `tests/snapshots/{name}_{light|dark}-tiny-skia.png`. `async` for
    /// the same compile-time-signal reason as [`Self::run`].
    ///
    /// Tests with non-default App state (active user, pre-loaded
    /// fixtures, etc.) or non-trait render entry points should inline the
    /// theme loop; this method covers the standard "render the View
    /// directly" case.
    fn assert_themed_snapshots(&self, name: &str) -> impl std::future::Future<Output = ()>;
}

impl<V: View> ViewTestExt for V {
    async fn run(&mut self, msgs: impl IntoMessages<V::Message>) -> Outcome<V> {
        let mut app = App::test();
        let mut last: Outcome<V> = Outcome::None;
        for msg in msgs.into_messages() {
            last = self.update(msg, app.update_ctx());
        }
        last
    }

    async fn assert_themed_snapshots(&self, name: &str) {
        init();
        for (theme, suffix) in [(AppTheme::light(), "light"), (AppTheme::dark(), "dark")] {
            let mut app = App::test();
            app.theme.current = theme.clone();
            let element = self.view(&app.render_ctx_main());
            assert_snapshot(format!("tests/snapshots/{name}_{suffix}"), &theme, element);
        }
    }
}

/// Test-only extension methods for asserting on a view's [`Outcome`].
///
/// `Outcome` can't derive `PartialEq` (Task is opaque) and the contained
/// types vary per view, so most tests fall back to `match` blocks with a
/// `panic!` arm. These helpers compress that to a single method call:
///
/// ```ignore
/// let ev = run_update(&mut view, msg).expect_event();
/// assert!(matches!(ev, NewFolderEvent::Run(s) if s == "Social"));
/// ```
pub trait OutcomeExt<V: View> {
    /// Panic unless the outcome is [`Outcome::None`].
    fn expect_none(self);
    /// Panic unless the outcome is [`Outcome::Event`]; return the event.
    fn expect_event(self) -> V::Event;
    /// Panic unless the outcome is [`Outcome::Toast`]; return the toast.
    fn expect_toast(self) -> Toast;
}

impl<V: View> OutcomeExt<V> for Outcome<V> {
    fn expect_none(self) {
        if !matches!(self, Outcome::None) {
            panic!("expected Outcome::None, got {}", outcome_kind(&self));
        }
    }

    fn expect_event(self) -> V::Event {
        match self {
            Outcome::Event(e) => e,
            other => panic!("expected Outcome::Event, got {}", outcome_kind(&other)),
        }
    }

    fn expect_toast(self) -> Toast {
        match self {
            Outcome::Toast(t) => t,
            other => panic!("expected Outcome::Toast, got {}", outcome_kind(&other)),
        }
    }
}

fn outcome_kind<V: View>(out: &Outcome<V>) -> &'static str {
    match out {
        Outcome::None => "Outcome::None",
        Outcome::Task(_) => "Outcome::Task(_)",
        Outcome::Event(_) => "Outcome::Event(_)",
        Outcome::Toast(_) => "Outcome::Toast(_)",
    }
}

/// Layer-C harness: wrap an element in a `Simulator`, drive interactions
/// against it, drain the produced messages. Build the element inline at
/// the call site — that's where Rust can reason about element-borrows-state
/// + element-borrows-context lifetimes naturally.
///
/// ```ignore
/// let element = view.view(&app.render_ctx_main());
/// let messages = drive_element(element, |ui| {
///     ui.click("Save").expect("Save button");
/// });
/// ```
pub fn drive_element<'a, M>(
    element: impl Into<Element<'a, M, AppTheme>>,
    interact: impl FnOnce(&mut Simulator<'a, M, AppTheme>),
) -> Vec<M>
where
    M: 'static,
{
    let mut ui = simulator(element);
    interact(&mut ui);
    ui.into_messages().collect()
}

/// Assert that at least one message in `messages` satisfies `matcher`,
/// with a debug-formatted error including the description and all messages
/// when not. Replaces the pattern:
///
/// ```ignore
/// assert!(
///     messages.iter().any(|m| matches!(m, FingerprintMessage::Close)),
///     "expected Close in {messages:?}",
/// );
/// ```
pub fn assert_emitted<M: std::fmt::Debug>(
    messages: &[M],
    description: &str,
    matcher: impl Fn(&M) -> bool,
) {
    assert!(
        messages.iter().any(matcher),
        "expected {description} in {messages:?}",
    );
}

/// Test-only constructors and accessors on [`App`]. Defined here (rather
/// than in `app/lifecycle.rs` / `app/view.rs`) so test-only methods stay
/// colocated with the rest of the test scaffolding.
impl App {
    /// Build a minimal `App` suitable for unit tests. Skips window creation,
    /// OS-service registration (menu / tray / hotkey), autostart wiring, and
    /// the SDK load — everything that needs a windowing system or filesystem.
    /// Tests construct one, optionally mutate fields, then call
    /// [`App::render_ctx`] / [`App::update_ctx`] to obtain contexts.
    pub(crate) fn test() -> Self {
        use std::collections::HashMap;
        use std::sync::Arc;

        use crate::app::Views;
        use crate::app::window::{WindowInfo, WindowKind};
        use crate::components::sidebar::SidebarState;
        use crate::services::sdk::ClientManager;
        use crate::services::settings::Settings;
        use crate::theme::ThemePreference;

        let main_window = iced::window::Id::unique();
        let magnify_id = iced::window::Id::unique();
        let mut windows = HashMap::new();
        windows.insert(
            main_window,
            WindowInfo::new(WindowKind::Main, crate::app::MAIN_WINDOW_SIZE),
        );

        Self {
            active_user: None,
            presentation_generation: 0,
            storage_warning: None,
            pending_logouts: Default::default(),
            sync_status: Default::default(),
            exit_after_logout: false,
            client_manager: ClientManager::empty(),
            settings: Settings::default(),

            screen: crate::domain::Screen::Loading,
            sidebar: SidebarState::default(),

            views: Views::new(),

            windows,
            main_window,
            main_window_focused: true,
            main_window_hidden: false,
            main_window_suspended: false,
            magnify: crate::views::magnify::MagnifyView::new(magnify_id),

            // Tests use a fixed theme and must not start the OS observer's
            // blocking D-Bus runtime from inside their Tokio runtime.
            theme: crate::app::ThemeState {
                preference: ThemePreference::Light,
                current: AppTheme::light(),
                system: None,
            },
            menu: crate::services::menu::menu_tree(),
            native_menu: None,
            main_window_raw_id: None,
            tray: None,

            ssh_agent: crate::services::ssh_agent::AgentRuntime::new(),
            ssh_approval: None,
            ssh_approval_window: None,
            shutting_down: false,
            clipboard: crate::services::clipboard::ClipboardManager::new(),
            favicon: crate::services::favicon::FaviconService::new(Arc::new(|_uid| String::new())),
            session_timeout: crate::services::session_timeout::SessionTimeout::new(),

            open_overlay: None,
            toasts: Vec::new(),

            cache: crate::app::ViewCache::default(),
        }
    }

    /// Build an `UpdateCtx` from the current App state. Test-only —
    /// production [`App::update`] inlines the equivalent construction
    /// because returning `UpdateCtx<'_>` from a `&mut self` method would
    /// block the simultaneous `&mut self.views.<view>` borrow the dispatch
    /// arms need.
    pub(crate) fn update_ctx(&mut self) -> crate::app::UpdateCtx<'_> {
        let active_vault_filter = self.sidebar.active_vault_filter;
        let active_send_filter = self.sidebar.active_send_filter;
        crate::app::UpdateCtx {
            vault_sort: self.settings.vault_sort,
            client_manager: &mut self.client_manager,
            active_user: self.active_user.as_ref(),
            active_vault_filter,
            active_send_filter,
            open_overlay: &mut self.open_overlay,
        }
    }

    /// Build a `RenderCtx` keyed to the main window — the standard render
    /// target for unit tests. Production renders go through per-window
    /// dispatch in [`App::view`] / [`App::render_ctx`].
    pub(crate) fn render_ctx_main(&self) -> RenderCtx<'_> {
        self.render_ctx(self.main_window)
    }
}
