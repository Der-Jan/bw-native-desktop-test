//! Revocable, account-specific icon transport. Never sends OAuth credentials.
mod cache;
use super::{ClientManager, SessionToken};
use crate::services::accounts::{
    profile::{ProfileStore, ServerProfile},
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone)]
pub struct FaviconSource {
    session: SessionToken,
    store: ProfileStore,
    profile: ServerProfile,
    icons: url::Url,
    epoch: Arc<AtomicU64>,
    transport: Arc<tokio::sync::OnceCell<Transport>>,
    cache: Arc<tokio::sync::OnceCell<tokio::sync::Mutex<cache::Catalog>>>,
}
impl FaviconSource {
    pub fn current(&self) -> bool {
        self.session.ensure_unlocked().is_ok()
            && self.session.state.network_authorized.load(Ordering::SeqCst)
            && self.epoch.load(Ordering::SeqCst) == self.profile.revision
            && self
                .profile
                .certificate
                .as_ref()
                .is_none_or(|cert| cert.check_time().is_ok())
    }
    pub fn same_authority(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.session.state, &other.session.state)
            && self.session.generation == other.session.generation
            && self.profile.id == other.profile.id
            && self.profile.revision == other.profile.revision
    }
    pub async fn fetch(&self, hostname: &str) -> Result<Vec<u8>, String> {
        if !self.current() {
            return Err("Icon session changed".into());
        }
        if crate::services::favicon::hostname_for_fetch(hostname).as_deref() != Some(hostname) {
            return Err("Invalid icon hostname".into());
        }
        let mut url = self.icons.clone();
        url.path_segments_mut()
            .map_err(|_| "Invalid icon endpoint")?
            .pop_if_empty()
            .push(hostname)
            .push("icon.png");
        let transport = self
            .transport
            .get_or_try_init(|| async {
                let store = self.store.clone();
                let profile = self.profile.clone();
                tokio::task::spawn_blocking(move || Transport::new(&store, profile))
                    .await
                    .map_err(|_| "Could not prepare icon transport")?
            })
            .await?;
        if !self.current() {
            return Err("Icon session changed".into());
        }
        let request = transport
            .request(reqwest_account::Method::GET, url.clone())?
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|_| "Could not prepare icon request")?;
        let mut response = transport.execute(request).await?;
        if !response.status().is_success() {
            return Err(format!(
                "Icon server returned HTTP {}",
                response.status().as_u16()
            ));
        }
        const LIMIT: usize = 2 * 1024 * 1024;
        if response.content_length().is_some_and(|n| n > LIMIT as u64) {
            return Err("Icon too large".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| "Could not read icon")? {
            if !self.current() {
                return Err("Icon session changed".into());
            }
            if bytes.len().saturating_add(chunk.len()) > LIMIT {
                return Err("Icon too large".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        transport.check(&url)?;
        if !self.current() {
            return Err("Icon session changed".into());
        }
        Ok(bytes)
    }
}
impl ClientManager {
    pub fn favicon_source_for(&self, uid: &bitwarden_core::UserId) -> Option<FaviconSource> {
        let entry = self.users.get(uid)?;
        let store = self.profiles.clone()?;
        let profile = store.get(entry.profile?)?;
        let source = FaviconSource {
            session: entry.session.token(),
            epoch: store.epoch(profile.id).ok()?,
            icons: profile.endpoint("icons/").ok()?,
            store,
            profile,
            transport: Arc::new(tokio::sync::OnceCell::new()),
            cache: Arc::new(tokio::sync::OnceCell::new()),
        };
        source.current().then_some(source)
    }
}
