//! Opt-in SSH agent. Only the active unlocked SDK session can serve keys.
//! Approvals/grants are transient and bound to authority generation, revision,
//! key, SSH username and the verified host-key path. No private keys are cached.
mod protocol;
#[cfg(unix)]
mod transport;

use crate::services::{
    preferences::SshPromptBehavior,
    sdk::ssh_agent::{AgentIdentity, AgentSession},
};
use protocol::{Bindings, MAX_PACKET, push_string};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::{Semaphore, broadcast, oneshot, watch};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Deny,
    Once,
    UntilLock,
}
#[derive(Clone, Debug)]
pub enum UiEvent {
    Approval(Arc<Approval>),
    Dismiss(Uuid),
}
pub struct Approval {
    pub id: Uuid,
    pub key_name: String,
    pub fingerprint: String,
    pub username: String,
    pub process: String,
    pub hosts: Vec<String>,
    pub can_remember: bool,
    pub default_decision: Decision,
    epoch: u64,
    shared: Weak<Shared>,
    reply: Mutex<Option<oneshot::Sender<Decision>>>,
}
impl std::fmt::Debug for Approval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SSHApproval")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
impl Approval {
    pub fn current(&self) -> bool {
        self.shared.upgrade().is_some_and(|s| s.current(self.epoch))
    }
    pub fn respond(&self, decision: Decision) {
        if let Some(reply) = self.reply.lock().unwrap().take() {
            let _ = reply.send(if self.current() {
                decision
            } else {
                Decision::Deny
            });
        }
    }
}
#[derive(Hash, PartialEq, Eq)]
struct Grant {
    id: bitwarden_vault::CipherId,
    revision: chrono::DateTime<chrono::Utc>,
    key: [u8; 32],
    user: String,
    hosts: Vec<[u8; 32]>,
}
struct State {
    enabled: bool,
    epoch: u64,
    session: Option<AgentSession>,
    mode: SshPromptBehavior,
    grants: HashSet<Grant>,
}
struct Shared {
    state: Mutex<State>,
    changes: watch::Sender<u64>,
    events: broadcast::Sender<UiEvent>,
    prompt: Arc<Semaphore>,
}
impl Shared {
    fn current(&self, epoch: u64) -> bool {
        let state = self.state.lock().unwrap();
        state.enabled
            && state.epoch == epoch
            && state.session.as_ref().is_some_and(AgentSession::valid)
    }
    fn invalidate(state: &mut State, changes: &watch::Sender<u64>) {
        state.epoch = state.epoch.checked_add(1).expect("SSH epoch exhausted");
        state.grants.clear();
        changes.send_replace(state.epoch);
    }
    async fn handle(
        self: &Arc<Self>,
        packet: Vec<u8>,
        bindings: &mut Bindings,
        epoch: u64,
        process: &str,
    ) -> Vec<u8> {
        let (session, mode) = {
            let state = self.state.lock().unwrap();
            if !state.enabled || state.epoch != epoch {
                return vec![5];
            };
            (state.session.clone(), state.mode)
        };
        if packet.as_slice() == [11] {
            let mut output = vec![12, 0, 0, 0, 0];
            let Some(session) = session else {
                return output;
            };
            let Ok(keys) = session.identities().await else {
                return output;
            };
            if !self.current(epoch) {
                return vec![12, 0, 0, 0, 0];
            }
            let mut count = 0u32;
            for key in keys {
                let size = 8 + key.public_key.len() + key.name.len();
                if output.len() + size > MAX_PACKET {
                    break;
                }
                push_string(&mut output, &key.public_key);
                push_string(&mut output, key.name.as_bytes());
                count += 1;
            }
            output[1..5].copy_from_slice(&count.to_be_bytes());
            return output;
        }
        let Some(session) = session else {
            return vec![5];
        };
        if !session.valid() {
            return vec![5];
        }
        if packet.first() == Some(&27) {
            return bindings.extension(&packet);
        }
        let Ok(request) = protocol::parse_sign(&packet, bindings) else {
            return vec![5];
        };
        let Ok(keys) = session.identities().await else {
            return vec![5];
        };
        if !self.current(epoch) {
            return vec![5];
        }
        let Some(identity) = keys
            .into_iter()
            .find(|k| k.public_key == request.public_key)
        else {
            return vec![5];
        };
        let grant = Grant {
            id: identity.id,
            revision: identity.revision,
            key: openssl::sha::sha256(&identity.public_key),
            user: request.authentication.username.clone(),
            hosts: request.authentication.hosts.clone(),
        };
        let remembered = request.authentication.rememberable
            && !grant.hosts.is_empty()
            && self.state.lock().unwrap().grants.contains(&grant);
        let decision = if mode == SshPromptBehavior::Never || remembered {
            Decision::Once
        } else {
            self.approve(&identity, &request.authentication, epoch, process, mode)
                .await
        };
        if matches!(decision, Decision::Deny) || !self.current(epoch) {
            return vec![5];
        }
        // SDK lease re-reads permissions, reprompt/deletion status, revision and
        // private/public correspondence after the user has approved.
        let Ok(signature) = session.sign(identity, request.data, request.flags).await else {
            return vec![5];
        };
        if !self.current(epoch) {
            return vec![5];
        }
        if matches!(decision, Decision::UntilLock)
            && !grant.hosts.is_empty()
            && request.authentication.rememberable
            && mode != SshPromptBehavior::Never
        {
            let mut state = self.state.lock().unwrap();
            if state.epoch == epoch && state.grants.len() < 128 {
                state.grants.insert(grant);
            }
        }
        let mut output = vec![14];
        push_string(&mut output, &signature);
        output
    }
    async fn approve(
        self: &Arc<Self>,
        identity: &AgentIdentity,
        auth: &protocol::Authentication,
        epoch: u64,
        process: &str,
        mode: SshPromptBehavior,
    ) -> Decision {
        let Ok(_permit) = self.prompt.clone().try_acquire_owned() else {
            return Decision::Deny;
        };
        let (tx, rx) = oneshot::channel();
        let approval = Arc::new(Approval {
            id: Uuid::new_v4(),
            key_name: identity.name.clone(),
            fingerprint: identity.fingerprint.clone(),
            username: auth.username.clone(),
            process: process.into(),
            hosts: auth.fingerprints.clone(),
            can_remember: auth.rememberable,
            default_decision: if auth.rememberable && mode == SshPromptBehavior::RememberUntilLock {
                Decision::UntilLock
            } else {
                Decision::Once
            },
            epoch,
            shared: Arc::downgrade(self),
            reply: Mutex::new(Some(tx)),
        });
        let id = approval.id;
        if self.events.send(UiEvent::Approval(approval)).is_err() {
            return Decision::Deny;
        }
        // A guard dismisses the dialog on timeout, disconnect, disable or task abort.
        struct Dismiss {
            id: Uuid,
            events: broadcast::Sender<UiEvent>,
        }
        impl Drop for Dismiss {
            fn drop(&mut self) {
                let _ = self.events.send(UiEvent::Dismiss(self.id));
            }
        }
        let _dismiss = Dismiss {
            id,
            events: self.events.clone(),
        };
        match tokio::time::timeout(std::time::Duration::from_secs(45), rx).await {
            Ok(Ok(decision)) => decision,
            _ => Decision::Deny,
        }
    }
}

pub struct AgentRuntime {
    shared: Arc<Shared>,
    requested: bool,
    path: PathBuf,
    error: Option<&'static str>,
    task: Option<tokio::task::JoinHandle<()>>,
    #[cfg(unix)]
    socket: Option<transport::SocketGuard>,
}
impl AgentRuntime {
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(16);
        let (changes, _) = watch::channel(0);
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    enabled: false,
                    epoch: 0,
                    session: None,
                    mode: SshPromptBehavior::Always,
                    grants: HashSet::new(),
                }),
                changes,
                events,
                prompt: Arc::new(Semaphore::new(1)),
            }),
            requested: false,
            path: socket_path(),
            error: None,
            task: None,
            #[cfg(unix)]
            socket: None,
        }
    }
    pub fn events(&self) -> &broadcast::Sender<UiEvent> {
        &self.shared.events
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// A desktop process cannot set the environment of an existing shell.
    /// Provide a correctly quoted command for the user's shell startup file.
    pub fn shell_setup(&self) -> String {
        format!(
            "export SSH_AUTH_SOCK='{}'",
            self.path.to_string_lossy().replace('\'', "'\\''")
        )
    }
    pub fn status(&self) -> &str {
        if let Some(error) = self.error {
            return error;
        }
        if !self.requested {
            return "Deaktiviert";
        }
        if self.task.as_ref().is_none_or(|t| t.is_finished()) {
            return "Nicht gestartet";
        }
        let state = self.shared.state.lock().unwrap();
        if state.session.as_ref().is_some_and(AgentSession::valid) {
            "Bereit · aktives Konto entsperrt"
        } else {
            "Bereit · Tresor zum Verwenden der Schlüssel entsperren"
        }
    }
    pub fn configure(&mut self, enabled: bool) {
        if self.requested == enabled {
            return;
        }
        self.stop();
        self.requested = enabled;
        self.error = None;
        if !enabled {
            return;
        }
        #[cfg(unix)]
        {
            match transport::bind(&self.path) {
                Ok((listener, guard)) => {
                    self.socket = Some(guard);
                    self.shared.state.lock().unwrap().enabled = true;
                    self.task = Some(tokio::spawn(transport::serve(
                        listener,
                        self.shared.clone(),
                    )));
                }
                Err(error) => self.error = Some(error),
            }
        }
        #[cfg(not(unix))]
        {
            self.error = Some("SSH-Agent ist auf dieser Plattform noch nicht verfügbar");
        }
    }
    pub fn set_context(&mut self, session: Option<AgentSession>, mode: SshPromptBehavior) {
        let mut state = self.shared.state.lock().unwrap();
        let same = match (&state.session, &session) {
            (None, None) => true,
            (Some(a), Some(b)) => a.same_authority(b),
            _ => false,
        };
        if same && state.mode == mode {
            return;
        }
        state.session = session;
        state.mode = mode;
        Shared::invalidate(&mut state, &self.shared.changes);
    }
    pub fn stop(&mut self) {
        {
            let mut state = self.shared.state.lock().unwrap();
            state.enabled = false;
            state.session = None;
            Shared::invalidate(&mut state, &self.shared.changes);
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
        #[cfg(unix)]
        {
            self.socket.take();
        }
        self.requested = false;
    }
}
impl Drop for AgentRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}
fn socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("BITWARDEN_SSH_AUTH_SOCK") {
        return PathBuf::from(path);
    }
    #[cfg(unix)]
    let uid = unsafe { libc::geteuid() };
    #[cfg(not(unix))]
    let uid = 0;
    stable_socket_path(
        std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .as_deref(),
        &std::env::temp_dir(),
        uid,
    )
}
fn stable_socket_path(runtime: Option<&Path>, temporary: &Path, uid: u32) -> PathBuf {
    let directory = match runtime.filter(|p| !p.as_os_str().is_empty()) {
        Some(runtime) => runtime.join("bitwarden-desktop-next"),
        None => temporary.join(format!("bw-native-ssh-{uid}")),
    };
    directory.join("ssh-agent.sock")
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) async fn test_ui_approval(
    can_remember: bool,
    default_until_lock: bool,
) -> (crate::services::sdk::ClientManager, Arc<Approval>) {
    let (manager, uid, identity) = crate::services::sdk::ssh_agent::test_fixture().await;
    let mut agent = AgentRuntime::new();
    agent.shared.state.lock().unwrap().enabled = true;
    agent.set_context(
        manager.agent_session_for(&uid),
        if default_until_lock {
            SshPromptBehavior::RememberUntilLock
        } else {
            SshPromptBehavior::Always
        },
    );
    let auth = protocol::Authentication {
        username: "test-user".into(),
        hosts: if can_remember { vec![[7; 32]] } else { vec![] },
        fingerprints: if can_remember {
            vec!["SHA256:synthetic-test-host".into()]
        } else {
            vec![]
        },
        rememberable: can_remember,
    };
    let epoch = agent.shared.state.lock().unwrap().epoch;
    let mut receiver = agent.events().subscribe();
    let shared = agent.shared.clone();
    let task = tokio::spawn(async move {
        let _keep_alive = agent;
        shared
            .approve(
                &identity,
                &auth,
                epoch,
                "synthetic test",
                if default_until_lock {
                    SshPromptBehavior::RememberUntilLock
                } else {
                    SshPromptBehavior::Always
                },
            )
            .await;
    });
    let UiEvent::Approval(approval) = receiver.recv().await.unwrap() else {
        panic!("approval")
    };
    // The broker task owns the runtime and times out normally; tests resolve it.
    drop(task);
    (manager, approval)
}
