//! Persist approval metadata only. No vault keys or browser credentials.
use super::Connection;
use crate::services::accounts::storage::{atomic_write, read_bounded};
use std::{collections::BTreeSet, path::PathBuf};

pub(super) struct Store {
    path: Option<PathBuf>,
    grants: BTreeSet<String>,
}
impl Store {
    pub fn load(path: Option<PathBuf>) -> Self {
        let grants = path
            .as_ref()
            .and_then(|p| read_bounded(p, 128 * 1024).ok())
            .and_then(|b| serde_json::from_slice::<BTreeSet<String>>(&b).ok())
            .filter(|g| {
                g.len() <= 1024
                    && g.iter()
                        .all(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            })
            .unwrap_or_default();
        Self { path, grants }
    }
    fn identity(connection: &Connection) -> String {
        let identity = serde_json::to_vec(&(
            connection.session.uid,
            connection.session.server_user_id,
            &connection.session.server,
            connection.browser_executable.as_os_str().as_encoded_bytes(),
            &connection.origin,
        ))
        .expect("Serializable browser identity");
        openssl::sha::sha256(&identity)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
    pub fn contains(&self, connection: &Connection) -> bool {
        self.grants.contains(&Self::identity(connection))
    }
    pub fn len(&self) -> usize {
        self.grants.len()
    }
    fn save(&self, grants: &BTreeSet<String>) -> Result<(), String> {
        if let Some(path) = &self.path {
            let data =
                serde_json::to_vec(grants).map_err(|_| "Could not encode browser approvals")?;
            atomic_write(
                path.parent().ok_or("Invalid approval path")?,
                path.file_name()
                    .and_then(|p| p.to_str())
                    .ok_or("Invalid approval path")?,
                &data,
            )
            .map_err(|_| "Could not save browser approvals")?;
        }
        Ok(())
    }
    pub fn approve(&mut self, connection: &Connection) -> Result<(), String> {
        if !connection.current() || !connection.session.unlocked() {
            return Err("Browser session changed".into());
        }
        if self.grants.len() >= 1024 {
            return Err("Too many browser approvals; clear saved approvals in Settings".into());
        }
        let mut next = self.grants.clone();
        next.insert(Self::identity(connection));
        self.save(&next)?;
        self.grants = next;
        Ok(())
    }
    pub fn clear(&mut self) -> Result<(), String> {
        // Revoke in memory even when durable storage fails. The caller reports
        // that failure instead of claiming it will survive a restart.
        self.grants.clear();
        self.save(&self.grants)
    }
}
