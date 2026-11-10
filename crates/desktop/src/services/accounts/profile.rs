use super::storage::{SecretStore, atomic_write, read_bounded};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    pub host: String,
    pub port: u16,
}
impl Origin {
    pub fn of(url: &Url) -> Result<Self, String> {
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err("Use an HTTPS address without credentials, query or fragment".into());
        }
        Ok(Self {
            host: url.host_str().ok_or("Server host is missing")?.to_owned(),
            port: url
                .port_or_known_default()
                .ok_or("Server port is missing")?,
        })
    }
    pub fn parse(s: &str) -> Result<Self, String> {
        let u = Url::parse(s.trim()).map_err(|_| "Invalid endpoint address")?;
        if u.path() != "/" || u.query().is_some() {
            return Err(
                "Additional endpoint permissions must contain only an HTTPS host and optional port"
                    .into(),
            );
        }
        Self::of(&u)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CertificateInfo {
    pub fingerprint: String,
    pub subject: String,
    pub not_before: i64,
    pub not_after: i64,
    pub secret: Uuid,
}
impl CertificateInfo {
    pub fn check_time(&self) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp();
        if now < self.not_before {
            Err("The client certificate is not yet valid".into())
        } else if now >= self.not_after {
            Err("The client certificate has expired; replace it in server settings".into())
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerProfile {
    pub id: Uuid,
    pub revision: u64,
    #[serde(default)]
    pub account_bound: bool,
    pub name: String,
    pub base: String,
    pub allowed: Vec<Origin>,
    pub certificate: Option<CertificateInfo>,
    pub server_ca_pem: Option<String>,
}
impl ServerProfile {
    pub fn new(base: &str) -> Result<Self, String> {
        let mut url = Url::parse(base.trim()).map_err(|_| "Invalid server address")?;
        let origin = Origin::of(&url)?;
        if url.query().is_some() {
            return Err("Server address must not contain a query".into());
        }
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        Ok(Self {
            id: Uuid::new_v4(),
            revision: 0,
            account_bound: false,
            name: origin.host.clone(),
            base: url.to_string(),
            allowed: vec![origin],
            certificate: None,
            server_ca_pem: None,
        })
    }
    pub fn cloud(eu: bool) -> Self {
        let mut p = Self::new(if eu {
            "https://vault.bitwarden.eu/"
        } else {
            "https://vault.bitwarden.com/"
        })
        .unwrap();
        for host in if eu {
            ["api.bitwarden.eu", "identity.bitwarden.eu"]
        } else {
            ["api.bitwarden.com", "identity.bitwarden.com"]
        } {
            p.allowed.push(Origin {
                host: host.into(),
                port: 443,
            });
        }
        p
    }
    pub fn endpoint(&self, path: &str) -> Result<Url, String> {
        let u = Url::parse(&self.base)
            .map_err(|_| "Invalid saved server")?
            .join(path)
            .map_err(|_| "Invalid endpoint")?;
        self.check(&u)?;
        Ok(u)
    }
    pub fn check(&self, url: &Url) -> Result<(), String> {
        if !self.allowed.contains(&Origin::of(url)?) {
            return Err(
                "The server requested an endpoint which has not been explicitly allowed".into(),
            );
        }
        if let Some(cert) = &self.certificate {
            cert.check_time()?;
        }
        Ok(())
    }
    pub fn default_api(&self) -> Result<Url, String> {
        match Url::parse(&self.base)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .as_deref()
            .unwrap_or("")
        {
            "vault.bitwarden.com" => Ok(Url::parse("https://api.bitwarden.com/").unwrap()),
            "vault.bitwarden.eu" => Ok(Url::parse("https://api.bitwarden.eu/").unwrap()),
            _ => self.endpoint("api/"),
        }
    }
    pub fn default_identity(&self) -> Result<Url, String> {
        match Url::parse(&self.base)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .as_deref()
            .unwrap_or("")
        {
            "vault.bitwarden.com" => Ok(Url::parse("https://identity.bitwarden.com/").unwrap()),
            "vault.bitwarden.eu" => Ok(Url::parse("https://identity.bitwarden.eu/").unwrap()),
            _ => self.endpoint("identity/"),
        }
    }
}
struct Registry {
    profiles: HashMap<Uuid, ServerProfile>,
    epochs: HashMap<Uuid, Arc<AtomicU64>>,
}
#[derive(Clone)]
pub struct ProfileStore {
    root: PathBuf,
    inner: Arc<Mutex<Registry>>,
    pub secrets: SecretStore,
    pub proxy: super::proxy::Broker,
}
impl ProfileStore {
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn system(root: &Path) -> Result<Self, String> {
        static STORES: std::sync::OnceLock<Mutex<HashMap<PathBuf, ProfileStore>>> =
            std::sync::OnceLock::new();
        let mut stores = STORES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .map_err(|_| "Server profile registry unavailable")?;
        if let Some(store) = stores.get(root) {
            return Ok(store.clone());
        }
        let store = Self::open(root, SecretStore::system(root))?;
        stores.insert(root.to_path_buf(), store.clone());
        Ok(store)
    }
    pub fn open(root: &Path, secrets: SecretStore) -> Result<Self, String> {
        let path = root.join("server-profiles.json");
        let profiles: HashMap<Uuid, ServerProfile> = if path.exists() {
            serde_json::from_slice(&read_bounded(&path, 2 * 1024 * 1024)?)
                .map_err(|_| "Saved server profiles are invalid")?
        } else {
            HashMap::new()
        };
        for (id, p) in &profiles {
            if *id != p.id || p.allowed.is_empty() {
                return Err("Saved server profile is invalid".into());
            }
            let base = Url::parse(&p.base).map_err(|_| "Saved server address is invalid")?;
            if base.query().is_some() || !p.allowed.contains(&Origin::of(&base)?) {
                return Err("Saved server origin is not allowed".into());
            }
            // Expired identities remain editable offline; Transport checks their clock.
        }
        let epochs = profiles
            .iter()
            .map(|(id, p)| (*id, Arc::new(AtomicU64::new(p.revision))))
            .collect();
        Ok(Self {
            root: root.to_path_buf(),
            proxy: super::proxy::Broker::environment(),
            inner: Arc::new(Mutex::new(Registry { profiles, epochs })),
            secrets,
        })
    }
    pub fn list(&self) -> Vec<ServerProfile> {
        let mut p: Vec<_> = self
            .inner
            .lock()
            .unwrap()
            .profiles
            .values()
            .cloned()
            .collect();
        p.sort_by(|a, b| a.name.cmp(&b.name));
        p
    }
    pub fn get(&self, id: Uuid) -> Option<ServerProfile> {
        self.inner.lock().unwrap().profiles.get(&id).cloned()
    }
    /// Persist the exact prelogin binding, never just an email or server URL.
    pub fn select_login_profile(&self, id: Uuid) -> Result<(), String> {
        let state = self
            .inner
            .lock()
            .map_err(|_| "Server settings unavailable")?;
        let profile = state
            .profiles
            .get(&id)
            .ok_or("Saved login profile is missing")?;
        if profile.account_bound {
            return Err("Account-specific profiles cannot become a prelogin default".into());
        }
        atomic_write(
            &self.root,
            "login-profile.json",
            &serde_json::to_vec(&id).map_err(|_| "Could not encode login profile selection")?,
        )
        .map_err(|_| "Could not save login profile selection".into())
    }
    pub fn selected_login_profile(&self) -> Result<Option<ServerProfile>, String> {
        let path = self.root.join("login-profile.json");
        if path
            .try_exists()
            .map_err(|_| "Login profile selection unavailable")?
        {
            let id: Uuid = serde_json::from_slice(&read_bounded(&path, 1024)?)
                .map_err(|_| "Invalid saved login profile selection")?;
            let profile = self.get(id).filter(|p| !p.account_bound).ok_or(
                "The selected login profile is missing; select a saved server in settings",
            )?;
            return Ok(Some(profile));
        }
        // Upgrade older previews without deleting any profile or certificate. Only
        // recover an unambiguous custom endpoint, preferring its sole mTLS binding.
        let custom: Vec<_> = self
            .list()
            .into_iter()
            .filter(|p| {
                !p.account_bound
                    && !matches!(
                        Url::parse(&p.base)
                            .ok()
                            .and_then(|u| u.host_str().map(str::to_owned))
                            .as_deref(),
                        Some("vault.bitwarden.com" | "vault.bitwarden.eu")
                    )
            })
            .collect();
        if custom.is_empty() {
            return Ok(None);
        }
        if custom.iter().any(|p| p.base != custom[0].base) {
            return Err("Mehrere gespeicherte Server vorhanden. Bitte das gewünschte Profil in den Server-/mTLS-Einstellungen auswählen.".into());
        }
        let profile = Self::preferred_login_profile(custom)?;
        self.select_login_profile(profile.id)?;
        Ok(Some(profile))
    }
    pub fn preferred_login_profile(
        mut profiles: Vec<ServerProfile>,
    ) -> Result<ServerProfile, String> {
        let with_identity: Vec<_> = profiles
            .iter()
            .filter(|p| p.certificate.is_some())
            .collect();
        if with_identity.len() > 1 {
            return Err("Mehrere Zertifikatsprofile für diesen Server vorhanden. Bitte ein gespeichertes Profil ausdrücklich auswählen.".into());
        }
        if let Some(profile) = with_identity.first() {
            return Ok((*profile).clone());
        }
        profiles.sort_by_key(|p| (p.revision, p.id));
        profiles.pop().ok_or("Server profile is missing".into())
    }
    pub fn epoch(&self, id: Uuid) -> Result<Arc<AtomicU64>, String> {
        self.inner
            .lock()
            .unwrap()
            .epochs
            .get(&id)
            .cloned()
            .ok_or("Server profile no longer exists".into())
    }
    pub fn save(
        &self,
        mut profile: ServerProfile,
        expected: Option<u64>,
    ) -> Result<ServerProfile, String> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| "Server settings unavailable")?;
        if state.profiles.get(&profile.id).map(|p| p.revision) != expected {
            return Err("Server settings changed while this operation was running".into());
        }
        // Validate all scope entries without rejecting an expired certificate's offline replacement.
        let base = Url::parse(&profile.base).map_err(|_| "Invalid server address")?;
        if !profile.allowed.contains(&Origin::of(&base)?) {
            return Err("Server origin is not allowed".into());
        }
        profile.revision = expected
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Profile revision exhausted")?;
        let mut next = state.profiles.clone();
        next.insert(profile.id, profile.clone());
        atomic_write(
            &self.root,
            "server-profiles.json",
            &serde_json::to_vec_pretty(&next).map_err(|_| "Could not encode server settings")?,
        )
        .map_err(|_| "Could not save server settings")?;
        let epoch = state
            .epochs
            .entry(profile.id)
            .or_insert_with(|| Arc::new(AtomicU64::new(0)));
        epoch.store(profile.revision, Ordering::SeqCst);
        state.profiles = next;
        Ok(profile)
    }
    pub fn remove(&self, id: Uuid) -> Result<(), String> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| "Server settings unavailable")?;
        let mut next = state.profiles.clone();
        next.remove(&id);
        atomic_write(
            &self.root,
            "server-profiles.json",
            &serde_json::to_vec_pretty(&next).map_err(|_| "Could not encode server settings")?,
        )
        .map_err(|_| "Could not remove server settings")?;
        if let Some(epoch) = state.epochs.get(&id) {
            epoch.store(u64::MAX, Ordering::SeqCst);
        }
        state.profiles = next;
        Ok(())
    }
    pub fn clone_for_account(&self, profile: &ServerProfile) -> Result<ServerProfile, String> {
        if self.epoch(profile.id)?.load(Ordering::SeqCst) != profile.revision {
            return Err("Server settings changed during login".into());
        }
        let mut clone = profile.clone();
        clone.account_bound = true;
        clone.id = Uuid::new_v4();
        clone.revision = 0;
        self.save(clone, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origins_keep_ports_and_paths_and_reject_credentials() {
        let p = ServerProfile::new("https://example.test:8443/vault").unwrap();
        assert_eq!(
            p.endpoint("identity/").unwrap().as_str(),
            "https://example.test:8443/vault/identity/"
        );
        assert!(
            p.check(&Url::parse("https://example.test/vault/").unwrap())
                .is_err()
        );
        assert!(ServerProfile::new("https://user:secret@example.test/").is_err());
        assert!(ServerProfile::new("http://example.test/").is_err());
        assert!(ServerProfile::new("https://example.test/#fragment").is_err());
    }
}
