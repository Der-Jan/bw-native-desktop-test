//! Private atomic storage and AEAD secrets protected by the OS credential store.
use base64::{Engine, engine::general_purpose::STANDARD};
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

// ASCII avoids KWallet 5 corrupting binary secrets advertised as text/plain.
const KEY_PREFIX: &str = "bw-desktop-key:v1:";
const INVALID_KEY: &str = "Invalid credential-store key. Sign in again for saved account credentials, or re-import the affected client certificate in server settings.";

fn encode_key(key: &[u8]) -> Result<Zeroizing<String>, String> {
    if key.len() != 32 {
        return Err(INVALID_KEY.into());
    }
    let mut text = Zeroizing::new(String::from(KEY_PREFIX));
    STANDARD.encode_string(key, &mut text);
    Ok(text)
}

fn decode_key(bytes: Zeroizing<Vec<u8>>) -> Result<Zeroizing<Vec<u8>>, String> {
    // Legacy entries are exactly 32 raw bytes. Never rewrite on read: a damaged
    // legacy key must first pass the envelope's AEAD authentication.
    if bytes.len() == 32 {
        return Ok(bytes);
    }
    let encoded = bytes
        .strip_prefix(KEY_PREFIX.as_bytes())
        .ok_or(INVALID_KEY)?;
    // Fixed size also bounds decoding allocations. STANDARD requires canonical padding.
    if encoded.len() != 44 {
        return Err(INVALID_KEY.into());
    }
    let decoded = Zeroizing::new(STANDARD.decode(encoded).map_err(|_| INVALID_KEY)?);
    if decoded.len() != 32 {
        return Err(INVALID_KEY.into());
    }
    Ok(decoded)
}

// Match only known diagnostic markers; never emit arbitrary backend payloads.
fn platform_failure(
    source: &(dyn std::error::Error + Send + Sync),
) -> (&'static str, &'static str) {
    let detail = source.to_string();
    for (marker, cause, message) in [
        (
            "org.freedesktop.DBus.Error.NoReply",
            "dbus_no_reply",
            "The credential-store service did not reply. Check whether a wallet prompt is waiting, then retry.",
        ),
        (
            "org.freedesktop.DBus.Error.Timeout",
            "dbus_timeout",
            "The credential-store request timed out. Complete any wallet prompt and retry.",
        ),
        (
            "org.freedesktop.DBus.Error.ServiceUnknown",
            "dbus_service_missing",
            "The Secret Service provider is not running in this desktop session.",
        ),
        (
            "org.freedesktop.DBus.Error.Disconnected",
            "dbus_disconnected",
            "The credential store lost its D-Bus session connection. Restart the app in the desktop session.",
        ),
        (
            "org.freedesktop.DBus.Error.AccessDenied",
            "dbus_denied",
            "The desktop session denied access to the credential-store service.",
        ),
        (
            "org.freedesktop.DBus.Error.UnknownObject",
            "dbus_stale_object",
            "The credential-store object disappeared. Retry after the wallet service is available.",
        ),
        (
            "org.freedesktop.DBus.Error.InvalidArgs",
            "dbus_invalid_args",
            "The credential-store service rejected the Secret Service request.",
        ),
        (
            "No DBus session or Secret Service provider found",
            "service_unavailable",
            "No D-Bus user session or Secret Service provider is available.",
        ),
        (
            "Crypto error:",
            "session_crypto",
            "The encrypted credential-store session could not be decoded.",
        ),
    ] {
        if detail.contains(marker) {
            return (cause, message);
        }
    }
    (
        "platform_unknown",
        "The OS credential-store service failed with an unclassified backend error.",
    )
}

fn keyring_error(operation: &str, error: keyring::Error) -> String {
    // Never log Debug/Display of arbitrary backend errors: BadEncoding contains
    // secret bytes and Ambiguous contains credentials. Emit only classified causes.
    let (cause, message) = match error {
        keyring::Error::NoEntry => (
            "missing",
            "The required OS credential-store key is missing. Sign in again or re-import the affected client certificate.",
        ),
        keyring::Error::NoStorageAccess(_) => (
            "access",
            "The OS credential store is locked or access was denied. Unlock it and retry.",
        ),
        keyring::Error::PlatformFailure(ref source)
            if source.to_string().contains("createDLGroup") =>
        {
            (
                "qca",
                "KWallet could not create an encrypted session. On Ubuntu 24.04/KWallet 5, install libqca-qt5-2-plugins and restart KWallet or sign out and back in.",
            )
        }
        keyring::Error::PlatformFailure(ref source) => platform_failure(source.as_ref()),
        keyring::Error::BadEncoding(_) => ("encoding", INVALID_KEY),
        keyring::Error::Ambiguous(_) => (
            "ambiguous",
            "Multiple OS credential-store entries match this key. Resolve the duplicate entries before retrying.",
        ),
        _ => ("request", "The OS credential store rejected the request."),
    };
    tracing::warn!(operation, cause, "OS credential-store operation failed");
    format!("{message} [{operation}/{cause}]")
}

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
        let text = encode_key(key)?;
        keyring::Entry::new(SERVICE, &format!("secret-{id}"))
            // Use the byte API symmetrically: Windows set_password encodes UTF-16.
            // The payload itself is ASCII, so KWallet text/plain conversion is safe.
            .and_then(|e| e.set_secret(text.as_bytes()))
            .map_err(|error| keyring_error("write", error))
    }
    fn get(&self, id: Uuid) -> Result<Zeroizing<Vec<u8>>, String> {
        let _guard = KEYRING_LOCK
            .lock()
            .map_err(|_| "Credential store unavailable")?;
        keyring::Entry::new(SERVICE, &format!("secret-{id}"))
            .and_then(|e| e.get_secret())
            .map(Zeroizing::new)
            .map_err(|error| keyring_error("read", error))
            .and_then(decode_key)
    }
    fn delete(&self, id: Uuid) -> Result<(), String> {
        let _guard = KEYRING_LOCK
            .lock()
            .map_err(|_| "Credential store unavailable")?;
        match keyring::Entry::new(SERVICE, &format!("secret-{id}"))
            .and_then(|e| e.delete_credential())
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(keyring_error("delete", error)),
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
            return Err(INVALID_KEY.into());
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
        .map_err(|_| "Encrypted secret was changed or its key is invalid. Sign in again for saved account credentials, or re-import the affected client certificate in server settings.".into())
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
    fn text_keys_preserve_all_byte_values_and_legacy_keys() {
        for offset in 0..=255u8 {
            let key: Vec<u8> = (0..32u8).map(|i| offset.wrapping_add(i)).collect();
            let encoded = encode_key(&key).unwrap();
            assert!(encoded.is_ascii());
            assert_eq!(
                *decode_key(Zeroizing::new(encoded.as_bytes().to_vec())).unwrap(),
                key
            );
            assert_eq!(*decode_key(Zeroizing::new(key.clone())).unwrap(), key);
        }
        // Even a legacy key beginning with the new marker remains readable.
        let mut legacy = vec![0; 32];
        legacy[..KEY_PREFIX.len()].copy_from_slice(KEY_PREFIX.as_bytes());
        assert_eq!(*decode_key(Zeroizing::new(legacy.clone())).unwrap(), legacy);
    }

    #[test]
    fn malformed_keys_are_rejected() {
        for value in [
            String::new(),
            "raw damaged key".into(),
            format!("bw-desktop-key:v2:{}", STANDARD.encode([0; 32])),
            format!("{KEY_PREFIX}{}", STANDARD.encode([0; 31])),
            format!("{KEY_PREFIX}{}", STANDARD.encode([0; 33])),
            format!("{KEY_PREFIX}{}", "!".repeat(44)),
            format!(
                "{KEY_PREFIX}{}",
                STANDARD.encode([0; 32]).trim_end_matches('=')
            ),
            format!("{KEY_PREFIX}{}", "A".repeat(43) + "B"),
            format!("{KEY_PREFIX}{}B=", "A".repeat(42)),
        ] {
            assert!(decode_key(Zeroizing::new(value.into_bytes())).is_err());
        }
        assert!(encode_key(&[0; 31]).is_err());
        assert!(encode_key(&[0; 33]).is_err());
    }

    #[test]
    fn credential_errors_are_distinct_and_do_not_expose_backend_payloads() {
        let missing = keyring_error("test", keyring::Error::NoEntry);
        let locked = keyring_error(
            "test",
            keyring::Error::NoStorageAccess(Box::new(io::Error::other("private payload"))),
        );
        let platform = keyring_error(
            "test",
            keyring::Error::PlatformFailure(Box::new(io::Error::other("private payload"))),
        );
        let qca = keyring_error(
            "test",
            keyring::Error::PlatformFailure(Box::new(io::Error::other(
                "createDLGroup failed private payload",
            ))),
        );
        assert_ne!(missing, locked);
        assert_ne!(locked, platform);
        assert!(qca.contains("libqca-qt5-2-plugins"));
        for message in [
            missing,
            locked,
            platform,
            qca,
            keyring_error(
                "test",
                keyring::Error::BadEncoding(b"private payload".to_vec()),
            ),
        ] {
            assert!(!message.contains("private payload"));
        }
    }

    /// Run separately in fresh KWallet 5 and GNOME Keyring desktop sessions.
    /// Uses synthetic keys and a random identifier; never reads existing entries.
    #[test]
    #[ignore = "requires an interactive OS credential store; see docs/testing.md"]
    fn os_key_store_roundtrip() {
        let id = Uuid::new_v4();
        let store = OsKeyStore;
        let result = (|| -> Result<(), String> {
            for key in [
                vec![0; 32],
                vec![255; 32],
                (0..32).map(|i| (i * 17) as u8).collect(),
            ] {
                store.put(id, &key)?;
                if *store.get(id)? != key {
                    return Err("OS credential-store roundtrip changed the key".into());
                }
            }
            Ok(())
        })();
        let cleanup = store.delete(id);
        result.unwrap();
        cleanup.unwrap();
        assert!(store.get(id).is_err());
    }

    #[test]
    fn damaged_legacy_key_fails_authentication_without_deleting_the_secret() {
        let root = std::env::temp_dir().join(format!("bw-secret-test-{}", Uuid::new_v4()));
        let keys = Arc::new(MemoryKeys::default());
        let store = SecretStore::with_keys(&root, keys.clone());
        let id = store.put(b"synthetic private material").unwrap();
        let original = keys.get(id).unwrap();
        let mut damaged = original.clone();
        damaged[0] ^= 1;
        keys.put(id, &damaged).unwrap();
        assert!(store.get(id).unwrap_err().contains("Sign in again"));
        assert!(store.root.join(format!("{id}.json")).exists());
        keys.put(id, &original).unwrap();
        assert_eq!(&*store.get(id).unwrap(), b"synthetic private material");
        store.delete(id).unwrap();
        fs::remove_dir_all(root).unwrap();
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
