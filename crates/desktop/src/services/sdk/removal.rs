//! Durable offline removal for the current local account store.
//!
//! Marker presence denies loading even if cleanup was interrupted. Markers stay
//! after cleanup so restoring an old database cannot silently resurrect an account.

use bitwarden_core::UserId;
use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use super::LockOperation;

#[derive(Debug, Clone, Copy)]
pub enum StorageWarning {
    StorageUnavailable,
    RemovalIncomplete,
    LogoutNotStored,
}

#[derive(Debug, Clone)]
pub enum LogoutFailure {
    NotCommitted,
    CleanupPending,
}

pub(super) struct RemovalStore {
    root: PathBuf,
    demo: bool,
    mutation: Mutex<()>,
}

impl RemovalStore {
    pub(super) fn new(root: PathBuf, demo: bool) -> Self {
        Self {
            root,
            demo,
            mutation: Mutex::new(()),
        }
    }

    fn markers(&self) -> PathBuf {
        self.root.join("removed-accounts")
    }

    /// Commit intent before deleting anything. Existing markers are idempotent.
    pub(super) fn persist(&self, uid: UserId) -> io::Result<()> {
        let _guard = self.mutation.lock().expect("removal store poisoned");
        let markers = self.markers();
        match fs::symlink_metadata(&markers) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Ok(_) => return Err(io::Error::other("Removal journal is not a directory")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let mut builder = fs::DirBuilder::new();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                builder.create(&markers)?;
            }
            Err(e) => return Err(e),
        }
        sync_directory(&self.root)?;
        let marker = markers.join(uid.to_string());
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = match options.open(&marker) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let meta = fs::symlink_metadata(&marker)?;
                if !meta.is_file() || meta.file_type().is_symlink() {
                    return Err(io::Error::other("Invalid removal marker"));
                }
                fs::OpenOptions::new().write(true).open(&marker)?
            }
            Err(e) => return Err(e),
        };
        file.sync_all()?;
        sync_directory(&markers)
    }

    /// Called only after the revoked runtime has closed its SDK owner.
    pub(super) fn complete(&self, uid: UserId) -> io::Result<()> {
        let _guard = self.mutation.lock().expect("removal store poisoned");
        self.require_marker(uid)?;
        self.cleanup(uid)
    }

    fn require_marker(&self, uid: UserId) -> io::Result<()> {
        let meta = fs::symlink_metadata(self.markers().join(uid.to_string()))?;
        if meta.is_file() && !meta.file_type().is_symlink() {
            Ok(())
        } else {
            Err(io::Error::other("Invalid removal marker"))
        }
    }

    fn cleanup(&self, uid: UserId) -> io::Result<()> {
        if self.demo {
            self.remove_mock_metadata(uid)?;
        } else {
            super::production::remove_record(&self.root, uid).map_err(std::io::Error::other)?;
        }
        // UUID-derived names only; never delete a caller-supplied path or recurse.
        for suffix in [".sqlite", ".sqlite-wal", ".sqlite-shm", ".sqlite-journal"] {
            match fs::remove_file(self.root.join(format!("{uid}{suffix}"))) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        sync_directory(&self.root)
    }

    fn remove_mock_metadata(&self, uid: UserId) -> io::Result<()> {
        let path = self.root.join("mock.json");
        let meta = fs::symlink_metadata(&path)?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(io::Error::other("Mock metadata is not a regular file"));
        }
        let mut value: serde_json::Value = serde_json::from_reader(fs::File::open(&path)?)?;
        let users = value
            .get_mut("users")
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| io::Error::other("Invalid mock account metadata"))?;
        let before = users.len();
        users.retain(|u| {
            u.get("user_id")
                .and_then(|v| v.as_str())
                .and_then(|id| id.parse::<UserId>().ok())
                != Some(uid)
        });
        if users.len() == before {
            return Ok(());
        }
        let temp = TemporaryMetadata(
            self.root
                .join(format!(".account-removal-{}.tmp", uuid::Uuid::new_v4())),
        );
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp.0)?;
        serde_json::to_writer_pretty(&mut file, &value)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp.0, path)?;
        sync_directory(&self.root)
    }

    /// Startup runs this before opening any SDK database. Cleanup failure stays
    /// pending, but the affected account remains blocked by its marker.
    pub(super) fn recover(&self) -> io::Result<(HashSet<UserId>, bool)> {
        let _guard = self.mutation.lock().expect("removal store poisoned");
        let markers = self.markers();
        match fs::symlink_metadata(&markers) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((HashSet::new(), false)),
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Err(e) => return Err(e),
            Ok(_) => return Err(io::Error::other("Invalid removal journal")),
        }
        let mut removed = HashSet::new();
        for entry in fs::read_dir(markers)? {
            let entry = entry?;
            let uid = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<UserId>().ok())
                .ok_or_else(|| io::Error::other("Invalid removal journal entry"))?;
            self.require_marker(uid)?;
            removed.insert(uid);
        }
        let mut pending = false;
        for uid in &removed {
            if let Err(error) = self.cleanup(*uid) {
                pending = true;
                tracing::warn!(%error, "Account removal cleanup remains pending");
            }
        }
        Ok((removed, pending))
    }
}

struct TemporaryMetadata(PathBuf);
impl Drop for TemporaryMetadata {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(path)?.sync_all()?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub struct LogoutOperation {
    pub(super) cleanup: LockOperation,
    pub(super) storage: Option<Arc<RemovalStore>>,
}

impl LogoutOperation {
    pub async fn finish(self) -> Result<(), LogoutFailure> {
        let uid = self.cleanup.session.user_id();
        let committed = match &self.storage {
            Some(storage) => {
                let storage = Arc::clone(storage);
                tokio::task::spawn_blocking(move || storage.persist(uid))
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r.map_err(|e| e.to_string()))
            }
            None => Ok(()), // Disk-free test fixture; real loaded managers have storage.
        };
        // Even persistence failure must clear keys and close the SDK runtime.
        self.cleanup.finish().await;
        if let Err(error) = committed {
            tracing::error!(%error, "Could not commit logout intent");
            return Err(LogoutFailure::NotCommitted);
        }
        if let Some(storage) = self.storage {
            let result = tokio::task::spawn_blocking(move || storage.complete(uid))
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r.map_err(|e| e.to_string()));
            if let Err(error) = result {
                tracing::warn!(%error, "Logout committed; local cleanup remains pending");
                return Err(LogoutFailure::CleanupPending);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::sdk::test_support::unlocked_manager;

    struct Fixture {
        root: PathBuf,
        store: Arc<RemovalStore>,
        uid: UserId,
        other: UserId,
    }
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("bw-removal-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let uid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
            let other = "00000000-0000-0000-0000-000000000002".parse().unwrap();
            let f = Self {
                store: Arc::new(RemovalStore::new(root.clone(), true)),
                root,
                uid,
                other,
            };
            f.seed();
            f
        }
        fn seed(&self) {
            fs::write(
                self.root.join("mock.json"),
                serde_json::to_vec(&serde_json::json!({
                    "users": [
                        { "user_id": self.uid, "master_password_dev_only": "synthetic secret" },
                        { "user_id": self.other, "extra": "preserved" }
                    ], "schema_extension": "preserved"
                }))
                .unwrap(),
            )
            .unwrap();
            for uid in [self.uid, self.other] {
                for suffix in [".sqlite", ".sqlite-wal", ".sqlite-shm", ".sqlite-journal"] {
                    fs::write(
                        self.root.join(format!("{uid}{suffix}")),
                        b"synthetic encrypted cache",
                    )
                    .unwrap();
                }
            }
            fs::write(self.root.join("settings.json"), b"global settings").unwrap();
        }
        fn metadata(&self) -> serde_json::Value {
            serde_json::from_reader(fs::File::open(self.root.join("mock.json")).unwrap()).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn restart_recovers_committed_intent_and_never_resurrects_restored_accounts() {
        let f = Fixture::new();
        f.store.persist(f.uid).unwrap(); // Simulate a crash before any cleanup.
        assert_eq!(f.metadata()["users"].as_array().unwrap().len(), 2);
        let reopened = RemovalStore::new(f.root.clone(), true);
        let (removed, pending) = reopened.recover().unwrap();
        assert!(removed.contains(&f.uid));
        assert!(!pending);
        assert_eq!(f.metadata()["users"].as_array().unwrap().len(), 1);
        assert_eq!(f.metadata()["users"][0]["user_id"], f.other.to_string());
        assert_eq!(f.metadata()["schema_extension"], "preserved");
        assert!(
            !fs::read_to_string(f.root.join("mock.json"))
                .unwrap()
                .contains("synthetic secret")
        );
        for suffix in [".sqlite", ".sqlite-wal", ".sqlite-shm", ".sqlite-journal"] {
            assert!(!f.root.join(format!("{}{suffix}", f.uid)).exists());
            assert!(f.root.join(format!("{}{suffix}", f.other)).exists());
        }
        assert_eq!(
            fs::read(f.root.join("settings.json")).unwrap(),
            b"global settings"
        );
        f.seed(); // Restoring an old cache must still respect the retained marker.
        assert!(reopened.recover().unwrap().0.contains(&f.uid));
        assert!(!f.root.join(format!("{}.sqlite", f.uid)).exists());
    }

    #[test]
    fn failed_cleanup_remains_blocked_and_retries_on_restart() {
        let f = Fixture::new();
        f.store.persist(f.uid).unwrap();
        let database = f.root.join(format!("{}.sqlite", f.uid));
        fs::remove_file(&database).unwrap();
        fs::create_dir(&database).unwrap(); // Deterministic I/O failure, even as root.
        let (removed, pending) = f.store.recover().unwrap();
        assert!(pending);
        assert!(removed.contains(&f.uid));
        fs::remove_dir(database).unwrap();
        assert!(!f.store.recover().unwrap().1);
    }

    #[test]
    fn cleanup_without_a_committed_marker_is_rejected() {
        let f = Fixture::new();
        assert!(f.store.complete(f.uid).is_err());
        assert_eq!(f.metadata()["users"].as_array().unwrap().len(), 2);
        assert!(f.root.join(format!("{}.sqlite", f.uid)).exists());
    }

    #[test]
    fn concurrent_account_removals_preserve_each_others_metadata_changes() {
        let f = Fixture::new();
        std::thread::scope(|scope| {
            for uid in [f.uid, f.other] {
                let store = f.store.clone();
                scope.spawn(move || {
                    store.persist(uid).unwrap();
                    store.complete(uid).unwrap();
                });
            }
        });
        assert!(f.metadata()["users"].as_array().unwrap().is_empty());
        assert_eq!(f.store.recover().unwrap().0.len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn journal_is_private_and_links_cannot_redirect_recovery() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let f = Fixture::new();
        f.store.persist(f.uid).unwrap();
        assert_eq!(
            fs::metadata(f.store.markers())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let marker = f.store.markers().join(f.uid.to_string());
        assert_eq!(
            fs::metadata(&marker).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_file(&marker).unwrap();
        symlink(f.root.join("settings.json"), &marker).unwrap();
        assert!(f.store.recover().is_err());
        assert_eq!(
            fs::read(f.root.join("settings.json")).unwrap(),
            b"global settings"
        );
    }

    #[tokio::test]
    async fn logout_closes_sdk_owner_even_with_unpolled_handles_and_unlocks() {
        let f = Fixture::new();
        let mut manager = unlocked_manager(f.uid);
        manager.storage = Some(f.store.clone());
        let queued = manager.client_for(&f.uid).unwrap();
        let raw = queued.session.state.client().unwrap();
        let weak = Arc::downgrade(&raw.0.internal);
        manager.lock(&f.uid).unwrap().finish().await;
        let unlock = manager.unlock_data_for(&f.uid).unwrap();
        manager.log_out(&f.uid).unwrap().finish().await.unwrap();
        assert!(!raw.is_unlocked());
        drop(raw);
        assert!(
            weak.upgrade().is_none(),
            "queued handles must not retain the SDK/database owner"
        );
        assert!(
            queued
                .run(|_| async {
                    panic!("revoked work executed");
                    #[allow(unreachable_code)]
                    Ok(())
                })
                .await
                .is_err()
        );
        assert!(unlock.unlock("test".into()).await.is_err());
        assert!(f.store.recover().unwrap().0.contains(&f.uid));
    }

    #[tokio::test]
    async fn persistence_failure_still_clears_keys_and_reports_failure() {
        let f = Fixture::new();
        fs::write(f.store.markers(), b"not a directory").unwrap();
        let mut manager = unlocked_manager(f.uid);
        manager.storage = Some(f.store.clone());
        let raw = manager
            .client_for(&f.uid)
            .unwrap()
            .session
            .state
            .client()
            .unwrap();
        assert!(manager.log_out(&f.uid).unwrap().finish().await.is_err());
        assert!(!manager.is_unlocked(&f.uid));
        assert!(!raw.is_unlocked());
        assert_eq!(f.metadata()["users"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn removal_commits_intent_before_draining_but_deletes_only_after_work_finishes() {
        let f = Fixture::new();
        let mut manager = unlocked_manager(f.uid);
        manager.storage = Some(f.store.clone());
        let client = manager.client_for(&f.uid).unwrap();
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let work = tokio::spawn(client.run(move |_| async move {
            entered_tx.send(()).unwrap();
            release_rx.await.unwrap();
            Ok("synthetic plaintext".to_string())
        }));
        entered_rx.await.unwrap();
        let cleanup = tokio::spawn(manager.log_out(&f.uid).unwrap().finish());
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while !f.store.markers().join(f.uid.to_string()).exists() {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        assert!(!cleanup.is_finished());
        assert!(f.root.join(format!("{}.sqlite", f.uid)).exists());
        release_tx.send(()).unwrap();
        assert!(work.await.unwrap().is_err());
        cleanup.await.unwrap().unwrap();
        assert!(!f.root.join(format!("{}.sqlite", f.uid)).exists());
    }
}
