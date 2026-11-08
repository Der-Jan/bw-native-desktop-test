//! Opt-in browser bridge. Native peers are process-checked and each key release
//! requires a current, account-bound foreground approval. No OS biometrics here.
mod legacy;
mod registration;
mod sdk_ipc;
mod server;
pub mod wire;
use crate::services::sdk::BrowserSession;
pub use registration::{Browser, install as register, restore as restore_registration};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Semaphore, broadcast, oneshot, watch};
use uuid::Uuid;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Deny,
    Once,
    UntilLock,
}
#[derive(Clone)]
pub enum Event {
    Approval(Arc<Approval>),
    Dismiss(Uuid),
    Changed,
}
pub struct Approval {
    pub id: Uuid,
    pub browser: String,
    pub email: String,
    pub server: String,
    pub extension: String,
    connection: Arc<Connection>,
    response: Mutex<Option<oneshot::Sender<Decision>>>,
    deadline: Instant,
}
impl Approval {
    pub fn current(&self) -> bool {
        self.deadline > Instant::now()
            && self.connection.current()
            && self.connection.session.unlocked()
            && self
                .response
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|r| !r.is_closed())
    }
    pub fn respond(&self, decision: Decision) {
        let decision = if self.current() {
            decision
        } else {
            Decision::Deny
        };
        if let Some(response) = self.response.lock().unwrap().take() {
            let _ = response.send(decision);
        }
    }
}
struct State {
    enabled: bool,
    epoch: u64,
    session: Option<BrowserSession>,
    status: String,
}
struct Shared {
    state: Mutex<State>,
    changes: watch::Sender<u64>,
    events: broadcast::Sender<Event>,
    prompt: Arc<Semaphore>,
}
pub struct BrowserRuntime {
    shared: Arc<Shared>,
    path: PathBuf,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl BrowserRuntime {
    pub fn new() -> Self {
        let (changes, _) = watch::channel(0);
        let (events, _) = broadcast::channel(16);
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    enabled: false,
                    epoch: 0,
                    session: None,
                    status: "Disabled".into(),
                }),
                changes,
                events,
                prompt: Arc::new(Semaphore::new(1)),
            }),
            path: socket_path(),
            task: None,
        }
    }
    pub fn events(&self) -> &broadcast::Sender<Event> {
        &self.shared.events
    }
    pub fn status(&self) -> String {
        self.shared.state.lock().unwrap().status.clone()
    }
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    pub fn configure(&mut self, enabled: bool) {
        let mut state = self.shared.state.lock().unwrap();
        if state.enabled != enabled {
            state.enabled = enabled;
            state.epoch += 1;
            state.status = if enabled {
                "Starting browser connection…"
            } else {
                "Disabled"
            }
            .into();
            self.shared.changes.send_replace(state.epoch);
        }
        drop(state);
        if enabled && self.task.as_ref().is_none_or(|t| t.is_finished()) {
            let shared = self.shared.clone();
            let path = self.path.clone();
            self.task = tokio::runtime::Handle::try_current()
                .ok()
                .map(|rt| rt.spawn(server::run(shared, path)));
        }
    }
    pub fn set_session(&self, session: Option<BrowserSession>) {
        let mut state = self.shared.state.lock().unwrap();
        let same = match (&state.session, &session) {
            (Some(a), Some(b)) => a.same_authority(b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            state.session = session;
            state.epoch += 1;
            self.shared.changes.send_replace(state.epoch);
        }
    }
    pub fn stop(&mut self) {
        self.configure(false);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
impl Drop for BrowserRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}
struct Connection {
    shared: Arc<Shared>,
    session: BrowserSession,
    epoch: u64,
    browser: String,
    origin: String,
    alive: AtomicBool,
    remembered: AtomicBool,
}
impl Connection {
    fn current(&self) -> bool {
        let state = self.shared.state.lock().unwrap();
        self.alive.load(Ordering::SeqCst)
            && state.enabled
            && state.epoch == self.epoch
            && self.session.current()
    }
    fn status(&self, id: Option<bitwarden_core::UserId>) -> u8 {
        if !self.current() || id.is_some_and(|id| id != self.session.server_user_id) {
            8
        } else if self.session.unlocked() {
            0
        } else {
            1
        }
    }
    async fn approve(self: &Arc<Self>, id: bitwarden_core::UserId) -> bool {
        if self.status(Some(id)) != 0 {
            return false;
        }
        if self.remembered.load(Ordering::SeqCst) {
            return self.current() && self.session.unlocked();
        }
        let Ok(_slot) = self.shared.prompt.clone().try_acquire_owned() else {
            return false;
        };
        let (tx, rx) = oneshot::channel();
        let approval = Arc::new(Approval {
            id: Uuid::new_v4(),
            browser: self.browser.clone(),
            email: self.session.email.clone(),
            server: self.session.server.clone(),
            extension: self.origin.clone(),
            connection: self.clone(),
            response: Mutex::new(Some(tx)),
            deadline: Instant::now() + Duration::from_secs(50),
        });
        struct Dismiss {
            shared: Arc<Shared>,
            id: Uuid,
        }
        impl Drop for Dismiss {
            fn drop(&mut self) {
                let _ = self.shared.events.send(Event::Dismiss(self.id));
            }
        }
        let _dismiss = Dismiss {
            shared: self.shared.clone(),
            id: approval.id,
        };
        if self.shared.events.send(Event::Approval(approval)).is_err() {
            return false;
        }
        let decision = tokio::time::timeout(Duration::from_secs(50), rx)
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or(Decision::Deny);
        if !self.current() || !self.session.unlocked() {
            return false;
        }
        if decision == Decision::UntilLock {
            self.remembered.store(true, Ordering::SeqCst);
        }
        decision != Decision::Deny
    }
    async fn key(
        self: &Arc<Self>,
        id: bitwarden_core::UserId,
    ) -> Option<bitwarden_crypto::SymmetricCryptoKey> {
        if !self.approve(id).await {
            return None;
        }
        let key = self.session.approved_browser_key().await.ok()?;
        (self.current() && self.session.unlocked()).then_some(key)
    }
}
#[cfg(all(unix, not(test)))]
fn socket_path() -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    let hash = openssl::sha::sha256(crate::paths::data_dir().as_os_str().as_bytes());
    let suffix: String = hash[..8].iter().map(|b| format!("{b:02x}")).collect();
    let directory = std::env::var_os("XDG_RUNTIME_DIR")
        .map(|path| PathBuf::from(path).join("bitwarden-desktop-next"))
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("bitwarden-desktop-next-{}", unsafe {
                libc::geteuid()
            }))
        });
    directory.join(format!("browser-{suffix}.sock"))
}

#[cfg(any(test, not(unix)))]
fn socket_path() -> PathBuf {
    std::env::temp_dir().join(format!("bw-browser-test-{}.sock", Uuid::new_v4()))
}

fn encode_key(key: &bitwarden_crypto::SymmetricCryptoKey) -> zeroize::Zeroizing<String> {
    use base64::Engine;
    let bytes = zeroize::Zeroizing::new(key.to_base64().into_bytes());
    zeroize::Zeroizing::new(base64::engine::general_purpose::STANDARD.encode(&*bytes))
}

#[cfg(test)]
fn test_fixture() -> (
    BrowserRuntime,
    crate::services::sdk::ClientManager,
    Arc<Connection>,
) {
    let uid = bitwarden_core::UserId::new_v4();
    let manager = crate::services::sdk::test_support::unlocked_manager(uid);
    let runtime = BrowserRuntime::new();
    let session = manager.browser_session_for(&uid).unwrap();
    runtime.set_session(Some(session.clone()));
    let epoch = {
        let mut state = runtime.shared.state.lock().unwrap();
        state.enabled = true;
        state.epoch
    };
    let connection = Arc::new(Connection {
        shared: runtime.shared.clone(),
        session,
        epoch,
        browser: "Helium".into(),
        origin: format!("chrome-extension://{}", wire::CHROMIUM_IDS[0]),
        alive: AtomicBool::new(true),
        remembered: AtomicBool::new(false),
    });
    (runtime, manager, connection)
}
#[cfg(test)]
mod tests;
