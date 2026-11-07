//! Private atomic storage and AEAD secrets protected by the OS credential store.
use openssl::{
    rand::rand_bytes,
    symm::{Cipher, decrypt_aead, encrypt_aead},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use uuid::Uuid;
use zeroize::Zeroizing;

static KEYRING_LOCK: Mutex<()> = Mutex::new(());
const SERVICE: &str = "com.bitwarden.desktop.next";

pub trait KeyStore: Send + Sync {
    fn put(&self, id: Uuid, key: &[u8]) -> Result<(), String>;
    fn get(&self, id: Uuid) -> Result<Zeroizing<Vec<u8>>, String>;
    fn delete(&self, id: Uuid) -> Result<(), String>;
}
struct OsKeyStore;
impl KeyStore for OsKeyStore {
    fn put(&self, id: Uuid, key: &[u8]) -> Result<(), String> {
        let _guard = KEYRING_LOCK
            .lock()
            .map_err(|_| "Credential store unavailable")?;
        keyring::Entry::new(SERVICE, &format!("secret-{id}"))
            .and_then(|e| e.set_secret(key))
            .map_err(|_| "The OS credential store is locked or unavailable".into())
    }
    fn get(&self, id: Uuid) -> Result<Zeroizing<Vec<u8>>, String> {
        let _guard = KEYRING_LOCK
            .lock()
            .map_err(|_| "Credential store unavailable")?;
        keyring::Entry::new(SERVICE, &format!("secret-{id}"))
            .and_then(|e| e.get_secret())
            .map(Zeroizing::new)
            .map_err(|_| "The OS credential store is locked or the required key is missing".into())
    }
    fn delete(&self, id: Uuid) -> Result<(), String> {
        let _guard = KEYRING_LOCK
            .lock()
            .map_err(|_| "Credential store unavailable")?;
        match keyring::Entry::new(SERVICE, &format!("secret-{id}"))
            .and_then(|e| e.delete_credential())
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("Could not remove a key from the OS credential store".into()),
        }
    }
}

#[derive(Clone)]
pub struct SecretStore {
    root: PathBuf,
    keys: Arc<dyn KeyStore>,
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u8,
    nonce: Vec<u8>,
    tag: Vec<u8>,
    ciphertext: Vec<u8>,
}
impl SecretStore {
    pub fn system(root: &Path) -> Self {
        Self {
            root: root.join("secrets"),
            keys: Arc::new(OsKeyStore),
        }
    }
    #[cfg(test)]
    pub fn with_keys(root: &Path, keys: Arc<dyn KeyStore>) -> Self {
        Self {
            root: root.join("secrets"),
            keys,
        }
    }
    pub fn put(&self, plaintext: &[u8]) -> Result<Uuid, String> {
        let id = Uuid::new_v4();
        let mut key = Zeroizing::new(vec![0; 32]);
        let mut nonce = vec![0; 12];
        let mut tag = vec![0; 16];
        rand_bytes(&mut key).map_err(|_| "Secure randomness unavailable")?;
        rand_bytes(&mut nonce).map_err(|_| "Secure randomness unavailable")?;
        let ciphertext = encrypt_aead(
            Cipher::aes_256_gcm(),
            &key,
            Some(&nonce),
            id.as_bytes(),
            plaintext,
            &mut tag,
        )
        .map_err(|_| "Secret encryption failed")?;
        self.keys.put(id, &key)?;
        let envelope = Envelope {
            version: 1,
            nonce,
            tag,
            ciphertext,
        };
        let result = serde_json::to_vec(&envelope)
            .map_err(|_| "Secret encoding failed".to_string())
            .and_then(|bytes| {
                atomic_write(&self.root, &format!("{id}.json"), &bytes)
                    .map_err(|_| "Secret storage failed".to_string())
            });
        if let Err(error) = result {
            let _ = self.keys.delete(id);
            return Err(error);
        }
        Ok(id)
    }
    pub fn get(&self, id: Uuid) -> Result<Zeroizing<Vec<u8>>, String> {
        let bytes = read_bounded(&self.root.join(format!("{id}.json")), 16 * 1024 * 1024)?;
        let e: Envelope = serde_json::from_slice(&bytes).map_err(|_| "Invalid encrypted secret")?;
        if e.version != 1 || e.nonce.len() != 12 || e.tag.len() != 16 {
            return Err("Invalid encrypted secret".into());
        }
        let key = self.keys.get(id)?;
        if key.len() != 32 {
            return Err("Invalid credential-store key".into());
        }
        decrypt_aead(
            Cipher::aes_256_gcm(),
            &key,
            Some(&e.nonce),
            id.as_bytes(),
            &e.ciphertext,
            &e.tag,
        )
        .map(Zeroizing::new)
        .map_err(|_| "Encrypted secret was changed or its key is invalid".into())
    }
    /// Startup-only garbage collection; keys are destroyed before orphan ciphertext.
    pub fn prune(&self, keep: &std::collections::HashSet<Uuid>) -> Result<(), String> {
        let entries = match fs::read_dir(&self.root) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err("Secret storage unavailable".into()),
        };
        for entry in entries {
            let path = entry.map_err(|_| "Secret storage unavailable")?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse().ok())
                .ok_or("Invalid encrypted secret filename")?;
            if !keep.contains(&id) {
                self.delete(id)?;
            }
        }
        Ok(())
    }
    pub fn delete(&self, id: Uuid) -> Result<(), String> {
        self.keys.delete(id)?; // Destroy authority first; retry file cleanup after a failure.
        match fs::remove_file(self.root.join(format!("{id}.json"))) {
            Ok(()) => sync_dir(&self.root).map_err(|_| "Could not commit secret removal".into()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("Could not remove encrypted secret file".into()),
        }
    }
}

pub fn private_dir(root: &Path) -> io::Result<()> {
    if let Ok(meta) = fs::symlink_metadata(root)
        && (!meta.is_dir() || meta.file_type().is_symlink())
    {
        return Err(io::Error::other(
            "Storage directory is not a regular directory",
        ));
    }
    let mut b = fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
pub fn atomic_write(root: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    private_dir(root)?;
    let target = root.join(name);
    if fs::symlink_metadata(&target).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
        return Err(io::Error::other("Storage target is not a regular file"));
    }
    let tmp = root.join(format!(".write-{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, target)?;
        sync_dir(root)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
pub fn sync_dir(root: &Path) -> io::Result<()> {
    #[cfg(unix)]
    fs::File::open(root)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = root;
    Ok(())
}
pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let meta = fs::symlink_metadata(path).map_err(|_| "Local file is unavailable")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > limit {
        return Err("Local file has an invalid type or is too large".into());
    }
    let mut out = Vec::new();
    fs::File::open(path)
        .map_err(|_| "Local file is unavailable")?
        .take(limit + 1)
        .read_to_end(&mut out)
        .map_err(|_| "Could not read local file")?;
    if out.len() as u64 > limit {
        return Err("Local file is too large".into());
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    #[derive(Default)]
    pub struct MemoryKeys(pub Mutex<std::collections::HashMap<Uuid, Vec<u8>>>);
    impl KeyStore for MemoryKeys {
        fn put(&self, id: Uuid, key: &[u8]) -> Result<(), String> {
            self.0.lock().unwrap().insert(id, key.to_vec());
            Ok(())
        }
        fn get(&self, id: Uuid) -> Result<Zeroizing<Vec<u8>>, String> {
            self.0
                .lock()
                .unwrap()
                .get(&id)
                .cloned()
                .map(Zeroizing::new)
                .ok_or("Missing test key".into())
        }
        fn delete(&self, id: Uuid) -> Result<(), String> {
            self.0.lock().unwrap().remove(&id);
            Ok(())
        }
    }
    #[test]
    fn secrets_are_authenticated_and_bound_to_their_identifier() {
        let root = std::env::temp_dir().join(format!("bw-secret-test-{}", Uuid::new_v4()));
        let store = SecretStore::with_keys(&root, Arc::new(MemoryKeys::default()));
        let id = store.put(b"synthetic private material").unwrap();
        assert_eq!(&*store.get(id).unwrap(), b"synthetic private material");
        let path = store.root.join(format!("{id}.json"));
        assert!(
            !String::from_utf8_lossy(&fs::read(&path).unwrap())
                .contains("synthetic private material")
        );
        let second = Uuid::new_v4();
        fs::copy(&path, store.root.join(format!("{second}.json"))).unwrap();
        let key = store.keys.get(id).unwrap();
        store.keys.put(second, &key).unwrap();
        assert!(store.get(second).is_err());
        store.delete(id).unwrap();
        assert!(store.get(id).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
