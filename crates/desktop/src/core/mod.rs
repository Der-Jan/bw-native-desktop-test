//! Single-threaded application model. Workers return bounded, scoped completions.
//! No renderer or window toolkit participates in account authority decisions.
mod account;
mod desktop;
pub mod generator;
pub mod model;
mod profile;
#[cfg(test)]
mod tests;
mod transfer;
pub mod vault;

use crate::{
    domain::{Screen, UserId},
    services::{
        self,
        accounts::{
            login::{Attempt, Factor, PendingLogin},
            profile::ProfileStore,
        },
        sdk::{ClientManager, SessionToken},
        settings::Settings,
    },
};
use model::{Editor, Generator, Prompt, Request, VaultCache};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

pub type Completion = Box<dyn FnOnce(&mut Core) + Send>;
#[derive(Clone)]
pub struct Dispatcher {
    sender: mpsc::SyncSender<Completion>,
    wake: Arc<dyn Fn() + Send + Sync>,
    runtime: tokio::runtime::Handle,
}
impl Dispatcher {
    pub fn post(&self, callback: impl FnOnce(&mut Core) + Send + 'static) {
        // All callbacks are bounded. An event-loop wake precedes backpressure,
        // including while the main native surface is hidden in the tray.
        (self.wake)();
        if self.sender.send(Box::new(callback)).is_ok() {
            (self.wake)();
        }
    }
    pub fn spawn<T: Send + 'static>(
        &self,
        future: impl Future<Output = T> + Send + 'static,
        apply: impl FnOnce(&mut Core, T) + Send + 'static,
    ) {
        let events = self.clone();
        self.runtime.spawn(async move {
            let result = future.await;
            events.post(move |core| apply(core, result));
        });
    }
}
#[derive(Clone)]
pub struct Scope {
    pub uid: UserId,
    pub generation: u64,
    pub session: SessionToken,
}
impl Scope {
    pub fn current(&self, core: &Core) -> bool {
        core.active == Some(self.uid)
            && core.generation == self.generation
            && core.manager.accepts_session(&self.session)
            && core.manager.is_unlocked(&self.uid)
    }
}
pub struct Notice {
    pub text: String,
    pub error: bool,
    pub expires: Instant,
}
pub struct SyncState {
    pub request: uuid::Uuid,
    pub error: Option<String>,
    pub busy: bool,
}
pub struct Login {
    pub email: String,
    pub busy: bool,
    pub attempt: Option<Attempt>,
    pub pending: Option<Box<PendingLogin>>,
    pub factors: Vec<Factor>,
    pub factor: Factor,
    pub error: Option<String>,
}
impl Default for Login {
    fn default() -> Self {
        Self {
            email: String::new(),
            busy: false,
            attempt: None,
            pending: None,
            factors: Vec::new(),
            factor: Factor::Authenticator,
            error: None,
        }
    }
}
impl Login {
    pub fn cancel(&mut self) {
        if let Some(a) = self.attempt.take() {
            a.cancel();
        }
        self.pending = None;
        self.busy = false;
        self.factors.clear();
        self.error = None;
    }
}
pub struct Core {
    pub manager: ClientManager,
    pub active: Option<UserId>,
    pub settings: Settings,
    pub appearance: services::desktop_appearance::Appearance,
    pub screen: Screen,
    pub generation: u64,
    /// UI must discard input/undo/layout state whenever this epoch changes.
    pub clear_epoch: u64,
    pub caches: HashMap<UserId, VaultCache>,
    pub selected: Option<bitwarden_vault::CipherId>,
    pub detail: Option<bitwarden_vault::CipherView>,
    pub detail_open: bool,
    pub detail_request: Option<uuid::Uuid>,
    pub editor: Option<Editor>,
    pub query: String,
    pub filter: services::vault_navigation::VaultFilter,
    pub scroll_to: Option<usize>,
    pub login: Login,
    pub profiles: Option<ProfileStore>,
    pub profile: Option<services::accounts::profile::ServerProfile>,
    pub profile_request: Option<uuid::Uuid>,
    pub profile_busy: bool,
    pub profile_error: Option<String>,
    pub prompt: Option<Prompt>,
    pub generator: Generator,
    pub sync: HashMap<UserId, SyncState>,
    pub notice: Option<Notice>,
    pub loading: bool,
    pub locking: HashSet<UserId>,
    pub removing: HashSet<UserId>,
    pub quit_requested: bool,
    pub focused: bool,
    pub hidden: bool,
    pub restore_requested: bool,
    pub magnify_requested: bool,
    pub settings_open: bool,
    pub generator_open: bool,
    pub export_open: bool,
    pub export_request: Option<uuid::Uuid>,
    pub import_open: bool,
    pub about_open: bool,
    pub fingerprint_open: bool,
    pub new_folder_open: bool,
    pub ssh_agent: services::ssh_agent::AgentRuntime,
    pub ssh_approval: Option<Arc<services::ssh_agent::Approval>>,
    pub clipboard: services::clipboard::ClipboardManager,
    pub favicon: services::favicon::FaviconService,
    pub timeout: services::session_timeout::SessionTimeout,
    events: Dispatcher,
    receiver: mpsc::Receiver<Completion>,
    pub window_action: Option<services::menu::MenuAction>,
    pub native_menu: Option<services::menu::NativeMenuHandle>,
    pub tray: Option<services::tray::TrayHandle>,
    pub tray_starting: bool,
}
impl Core {
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self::build(wake, Settings::load(), true)
    }
    #[cfg(test)]
    pub(crate) fn test(manager: ClientManager) -> Self {
        let mut core = Self::build(Arc::new(|| {}), Settings::default(), false);
        core.manager = manager;
        core.loading = false;
        core.screen = Screen::Vault;
        core.active = core.manager.user_ids().first().copied();
        core
    }
    fn build(wake: Arc<dyn Fn() + Send + Sync>, settings: Settings, boot: bool) -> Self {
        let (sender, receiver) = mpsc::sync_channel(128);
        let events = Dispatcher {
            sender,
            wake,
            runtime: tokio::runtime::Handle::current(),
        };
        if !settings.language.is_empty()
            && let Ok(tag) = settings.language.parse()
        {
            services::i18n::set_language(tag);
        }
        let mut core = Self {
            manager: ClientManager::empty(),
            active: None,
            settings,
            appearance: services::desktop_appearance::Appearance::default(),
            screen: Screen::Loading,
            generation: 0,
            clear_epoch: 0,
            caches: HashMap::new(),
            selected: None,
            detail: None,
            detail_open: false,
            detail_request: None,
            editor: None,
            query: String::new(),
            filter: Default::default(),
            scroll_to: None,
            login: Login::default(),
            profiles: None,
            profile: None,
            profile_request: None,
            profile_busy: false,
            profile_error: None,
            prompt: None,
            generator: Generator::default(),
            sync: HashMap::new(),
            notice: None,
            loading: true,
            locking: HashSet::new(),
            removing: HashSet::new(),
            quit_requested: false,
            focused: true,
            hidden: false,
            restore_requested: false,
            magnify_requested: false,
            settings_open: false,
            generator_open: false,
            export_open: false,
            export_request: None,
            import_open: false,
            about_open: false,
            fingerprint_open: false,
            new_folder_open: false,
            ssh_agent: services::ssh_agent::AgentRuntime::new(),
            ssh_approval: None,
            clipboard: services::clipboard::ClipboardManager::new(),
            favicon: services::favicon::FaviconService::new(Arc::new(|_| {
                "https://icons.bitwarden.net".into()
            })),
            timeout: services::session_timeout::SessionTimeout::new(),
            events,
            receiver,
            window_action: None,
            native_menu: None,
            tray: None,
            tray_starting: false,
        };
        if boot {
            core.install_desktop_events();
            core.events
                .spawn(ClientManager::load(), |core, mut manager| {
                    if let Some(warning) = manager.take_startup_warning() {
                        core.notify(format!("{warning:?}"), true);
                    }
                    core.profiles = manager.profiles();
                    if let Some(store) = core.profiles.clone() {
                        core.restore_profile(&store);
                    }
                    core.manager = manager;
                    core.loading = false;
                    core.screen = Screen::Login;
                    if let Some(uid) = core.manager.user_ids().first().copied() {
                        core.switch_user(uid);
                    }
                    core.refresh_agent();
                });
        }
        core
    }
    pub fn poll(&mut self) {
        // Bounded per-pass work prevents a burst of results from starving input.
        for _ in 0..128 {
            match self.receiver.try_recv() {
                Ok(apply) => apply(self),
                Err(_) => break,
            }
        }
        if self
            .notice
            .as_ref()
            .is_some_and(|n| n.expires <= Instant::now())
        {
            self.notice = None;
        }
        self.refresh_agent();
    }
    pub fn scope(&self) -> Option<Scope> {
        let uid = self.active?;
        if !self.manager.is_unlocked(&uid) {
            return None;
        }
        Some(Scope {
            uid,
            generation: self.generation,
            session: self.manager.session_for(&uid)?,
        })
    }
    pub fn spawn_scoped<T: Send + 'static>(
        &self,
        scope: Scope,
        future: impl Future<Output = T> + Send + 'static,
        apply: impl FnOnce(&mut Core, T) + Send + 'static,
    ) {
        self.events.spawn(future, move |core, result| {
            if scope.current(core) {
                apply(core, result);
            }
        });
    }
    pub fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.notice = Some(Notice {
            text: text.into(),
            error,
            expires: Instant::now() + Duration::from_secs(6),
        });
    }
    pub fn remote(&self) -> bool {
        self.active.is_some_and(|uid| self.manager.is_remote(&uid))
    }
    pub fn unlocked(&self) -> bool {
        self.active
            .is_some_and(|uid| self.manager.is_unlocked(&uid))
    }
    pub fn current_cache(&self) -> Option<&VaultCache> {
        self.active.and_then(|uid| self.caches.get(&uid))
    }
    pub fn clear_presentation(&mut self) {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("presentation generation");
        self.clear_epoch = self.clear_epoch.checked_add(1).expect("UI cleanup epoch");
        self.cancel_prompt();
        self.query.zeroize();
        self.notice = None;
        self.selected = None;
        self.detail = None;
        self.detail_open = false;
        self.detail_request = None;
        self.editor = None;
        self.generator.clear();
        if let Some(uid) = self.active {
            self.favicon.evict_user(&uid);
        }
        self.generator_open = false;
        self.export_open = false;
        self.export_request = None;
        self.import_open = false;
        self.new_folder_open = false;
        self.fingerprint_open = false;
        self.clipboard.clear_owned();
        self.magnify_requested = false;
    }
    pub fn cancel_prompt(&mut self) {
        self.prompt = None;
        self.manager.clear_grants();
    }
    pub fn request(&mut self, request: Request) {
        self.request_action(request);
    }
}
