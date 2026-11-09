//! Best-effort encrypted icon cache in the account's existing SDK SQLite store.
use super::FaviconSource;
use base64::{Engine, engine::general_purpose::STANDARD};
use bitwarden_core::{UserId, key_management::SymmetricKeySlotId};
use bitwarden_crypto::{Decryptable, EncString, PrimitiveEncryptable};
use bitwarden_pm::PasswordManagerClient;
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use zeroize::{Zeroize, Zeroizing};

bitwarden_state::register_setting_key!(pub(super) const CACHE:String="native_encrypted_icon_cache_v1");
const MAX_ENTRIES: usize = 512;
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_STORED: usize = 4 * 1024 * 1024;
const MAX_ICON: usize = 8 * 1024;
const TTL: i64 = 7 * 24 * 60 * 60;
#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    hostname: String,
    png: String,
    fetched: i64,
}
impl Drop for Entry {
    fn drop(&mut self) {
        self.hostname.zeroize();
        self.png.zeroize();
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Catalog {
    version: u8,
    account: UserId,
    profile: uuid::Uuid,
    revision: u64,
    entries: Vec<Entry>,
}
impl Catalog {
    fn empty(source: &FaviconSource) -> Self {
        Self {
            version: 1,
            account: source.session.user_id(),
            profile: source.profile.id,
            revision: source.profile.revision,
            entries: Vec::new(),
        }
    }
    fn prune(&mut self, now: i64) {
        self.entries.retain(|e| {
            e.fetched <= now.saturating_add(300)
                && now.saturating_sub(e.fetched) <= TTL
                && e.hostname.len() <= 253
                && e.png.len() <= MAX_ICON * 4 / 3 + 4
                && crate::services::favicon::hostname_for_fetch(&e.hostname).as_deref()
                    == Some(e.hostname.as_str())
        });
        self.entries.sort_by_key(|e| e.fetched);
        let mut bytes: usize = self.entries.iter().map(|e| e.png.len()).sum();
        let mut removed = 0;
        while self.entries.len() - removed > MAX_ENTRIES || bytes > MAX_BYTES {
            bytes -= self.entries[removed].png.len();
            removed += 1;
        }
        self.entries.drain(..removed);
    }
}
fn decode(client: &PasswordManagerClient, stored: Option<String>, expected: Catalog) -> Catalog {
    let decode = || -> Option<Catalog> {
        let stored = stored?;
        if stored.len() > MAX_STORED {
            return None;
        }
        let encrypted: EncString = stored.parse().ok()?;
        let keys = client.0.internal.get_key_store();
        let plaintext: String = encrypted
            .decrypt(&mut keys.context(), SymmetricKeySlotId::User)
            .ok()?;
        let plaintext = Zeroizing::new(plaintext);
        if plaintext.len() > MAX_STORED {
            return None;
        }
        let mut catalog: Catalog = serde_json::from_str(&plaintext).ok()?;
        if catalog.version != 1
            || catalog.account != expected.account
            || catalog.profile != expected.profile
            || catalog.revision != expected.revision
        {
            return None;
        }
        catalog.prune(chrono::Utc::now().timestamp());
        Some(catalog)
    };
    decode().unwrap_or(expected)
}
fn encode(client: &PasswordManagerClient, catalog: &Catalog) -> Result<String, String> {
    let plaintext =
        Zeroizing::new(serde_json::to_string(catalog).map_err(|_| "Icon cache unavailable")?);
    let keys = client.0.internal.get_key_store();
    let encrypted: EncString = plaintext
        .as_str()
        .encrypt(&mut keys.context_mut(), SymmetricKeySlotId::User)
        .map_err(|_| "Icon cache unavailable")?;
    Ok(encrypted.to_string())
}
impl FaviconSource {
    async fn load_catalog(&self) -> Result<&tokio::sync::Mutex<Catalog>, String> {
        self.cache
            .get_or_try_init(|| async {
                let _lease = self.session.state.gate.read().await;
                if !self.current() {
                    return Err("Icon session changed".into());
                }
                let client = self.session.state.client()?;
                let stored = client
                    .platform()
                    .state()
                    .setting(CACHE)
                    .map_err(|_| "Icon cache unavailable")?
                    .get()
                    .await
                    .map_err(|_| "Icon cache unavailable")?;
                let expected = Catalog::empty(self);
                let catalog =
                    tokio::task::spawn_blocking(move || decode(&client, stored, expected))
                        .await
                        .map_err(|_| "Icon cache unavailable")?;
                if !self.current() {
                    return Err("Icon session changed".into());
                }
                Ok(tokio::sync::Mutex::new(catalog))
            })
            .await
    }
    pub(crate) async fn cached_icon(&self, hostname: &str) -> Option<Vec<u8>> {
        if !self.current() {
            return None;
        }
        let mut catalog = self.load_catalog().await.ok()?.lock().await;
        catalog.prune(chrono::Utc::now().timestamp());
        let bytes = STANDARD
            .decode(
                &catalog
                    .entries
                    .iter()
                    .rev()
                    .find(|e| e.hostname == hostname)?
                    .png,
            )
            .ok()?;
        (bytes.len() <= MAX_ICON && self.current()).then_some(bytes)
    }
    pub(crate) async fn cache_icon(
        &self,
        hostname: &str,
        png: Vec<u8>,
        active: Arc<AtomicBool>,
    ) -> Result<(), String> {
        if png.len() > MAX_ICON
            || hostname.len() > 253
            || crate::services::favicon::hostname_for_fetch(hostname).as_deref() != Some(hostname)
        {
            return Err("Invalid cached icon".into());
        }
        let mut memory = self.load_catalog().await?.lock().await;
        // Serialize read/modify/write with every source incarnation for this
        // account, while allowing unrelated vault reads. Never hold the lease
        // during an HTTP request or image decode.
        let _write = self.session.state.icon_gate.lock().await;
        let _lease = self.session.state.gate.read().await;
        if !self.current() || !active.load(Ordering::SeqCst) {
            return Err("Icon session changed".into());
        }
        let client = self.session.state.client()?;
        let setting = client
            .platform()
            .state()
            .setting(CACHE)
            .map_err(|_| "Icon cache unavailable")?;
        let stored = setting.get().await.map_err(|_| "Icon cache unavailable")?;
        let expected = Catalog::empty(self);
        let hostname = hostname.to_owned();
        let worker = PasswordManagerClient(client.0.clone());
        let (catalog, encrypted) = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let mut catalog = decode(&worker, stored, expected);
            catalog.entries.retain(|e| e.hostname != hostname);
            catalog.entries.push(Entry {
                hostname,
                png: STANDARD.encode(png),
                fetched: chrono::Utc::now().timestamp(),
            });
            catalog.prune(chrono::Utc::now().timestamp());
            let encrypted = encode(&worker, &catalog)?;
            Ok((catalog, encrypted))
        })
        .await
        .map_err(|_| "Icon cache unavailable")??;
        if !self.current() || !active.load(Ordering::SeqCst) {
            return Err("Icon session changed".into());
        }
        setting
            .update(encrypted)
            .await
            .map_err(|_| "Icon cache unavailable")?;
        *memory = catalog;
        if !self.current() {
            return Err("Icon session changed".into());
        }
        Ok(())
    }
}

#[cfg(test)]
impl FaviconSource {
    pub(crate) async fn encrypted_icons_for_test(&self) -> String {
        self.session
            .state
            .client()
            .unwrap()
            .platform()
            .state()
            .setting(CACHE)
            .unwrap()
            .get()
            .await
            .unwrap()
            .unwrap()
    }
    pub(crate) async fn replace_icons_for_test(&self, stored: String) {
        self.session
            .state
            .client()
            .unwrap()
            .platform()
            .state()
            .setting(CACHE)
            .unwrap()
            .update(stored)
            .await
            .unwrap();
    }
    pub(crate) async fn alter_icons_for_test(&self, age: bool, invalid_png: bool) {
        let client = self.session.state.client().unwrap();
        let mut catalog = decode(
            &client,
            Some(self.encrypted_icons_for_test().await),
            Catalog::empty(self),
        );
        assert!(!catalog.entries.is_empty());
        for entry in &mut catalog.entries {
            if age {
                entry.fetched = chrono::Utc::now().timestamp() - TTL - 1;
            }
            if invalid_png {
                entry.png = STANDARD.encode(b"invalid cached PNG");
            }
        }
        self.replace_icons_for_test(encode(&client, &catalog).unwrap())
            .await;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn catalog() -> Catalog {
        Catalog {
            version: 1,
            account: UserId::new_v4(),
            profile: uuid::Uuid::new_v4(),
            revision: 1,
            entries: Vec::new(),
        }
    }
    #[test]
    fn cache_count_and_byte_limits_evict_oldest_entries() {
        let now = chrono::Utc::now().timestamp();
        for bytes in [10, 8000] {
            let mut cache = catalog();
            for index in 0..600 {
                cache.entries.push(Entry {
                    hostname: format!("{index}.example.test"),
                    png: "a".repeat(bytes),
                    fetched: now - index,
                });
            }
            cache.prune(now);
            assert!(cache.entries.len() <= MAX_ENTRIES);
            assert!(cache.entries.iter().map(|e| e.png.len()).sum::<usize>() <= MAX_BYTES);
            assert_eq!(cache.entries.last().unwrap().hostname, "0.example.test");
            assert!(
                !cache
                    .entries
                    .iter()
                    .any(|e| e.hostname == "599.example.test")
            );
        }
    }
    #[test]
    fn cache_rejects_expired_future_and_noncanonical_hosts() {
        let now = chrono::Utc::now().timestamp();
        let mut cache = catalog();
        for (hostname, fetched) in [
            ("old.test", now - TTL - 1),
            ("future.test", now + 301),
            ("localhost", now),
            ("valid.test", now),
        ] {
            cache.entries.push(Entry {
                hostname: hostname.into(),
                png: "".into(),
                fetched,
            });
        }
        cache.prune(now);
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.entries[0].hostname, "valid.test");
    }
}
