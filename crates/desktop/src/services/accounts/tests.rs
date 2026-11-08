//! Disposable TLS endpoint: requires a client identity, records actual requests.
use super::{
    certificate,
    login::{Attempt, Factor, PendingLogin, Step},
    profile::{ProfileStore, ServerProfile},
    storage::{SecretStore, test_support::MemoryKeys},
    transport::Transport,
};
use openssl::{
    asn1::Asn1Time,
    bn::BigNum,
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    ssl::{SslAcceptor, SslMethod, SslVerifyMode},
    stack::Stack,
    x509::{
        X509, X509NameBuilder,
        extension::{BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAlternativeName},
    },
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;
use zeroize::Zeroizing;
fn cert(cn: &str, issuer: Option<(&X509, &PKey<Private>)>, days: u32) -> (PKey<Private>, X509) {
    let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_text("CN", cn).unwrap();
    let name = name.build();
    let mut b = X509::builder().unwrap();
    b.set_version(2).unwrap();
    b.set_serial_number(
        &BigNum::from_u32(if cn == "localhost" { 2 } else { 3 })
            .unwrap()
            .to_asn1_integer()
            .unwrap(),
    )
    .unwrap();
    b.set_subject_name(&name).unwrap();
    b.set_issuer_name(issuer.map(|(c, _)| c.subject_name()).unwrap_or(&name))
        .unwrap();
    b.set_pubkey(&key).unwrap();
    b.set_not_before(&Asn1Time::days_from_now(0).unwrap())
        .unwrap();
    b.set_not_after(&Asn1Time::days_from_now(days).unwrap())
        .unwrap();
    if days == 0 {
        let now = chrono::Utc::now().timestamp();
        b.set_not_before(&Asn1Time::from_unix(now - 172800).unwrap())
            .unwrap();
        b.set_not_after(&Asn1Time::from_unix(now - 86400).unwrap())
            .unwrap();
    }
    if issuer.is_none() {
        b.append_extension(BasicConstraints::new().critical().ca().build().unwrap())
            .unwrap();
        b.append_extension(KeyUsage::new().key_cert_sign().crl_sign().build().unwrap())
            .unwrap();
    } else {
        b.append_extension(BasicConstraints::new().build().unwrap())
            .unwrap();
        b.append_extension(
            ExtendedKeyUsage::new()
                .server_auth()
                .client_auth()
                .build()
                .unwrap(),
        )
        .unwrap();
        b.append_extension(
            KeyUsage::new()
                .digital_signature()
                .key_encipherment()
                .build()
                .unwrap(),
        )
        .unwrap();
    }
    if cn == "localhost" {
        let san = SubjectAlternativeName::new()
            .dns("localhost")
            .build(&b.x509v3_context(issuer.map(|(c, _)| c.as_ref()), None))
            .unwrap();
        b.append_extension(san).unwrap();
    }
    b.sign(
        issuer.map(|(_, k)| k).unwrap_or(&key),
        MessageDigest::sha256(),
    )
    .unwrap();
    (key, b.build())
}
struct Fixture {
    root: PathBuf,
    store: ProfileStore,
    profile: ServerProfile,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    requests: Arc<Mutex<Vec<(String, String)>>>,
    client: PKey<Private>,
    leaf: X509,
    ca: X509,
    ca_key: PKey<Private>,
    sync_response: Arc<Mutex<(u16, serde_json::Value)>>,
    sync_delay: Arc<std::sync::atomic::AtomicU64>,
}
impl Fixture {
    fn new() -> Self {
        Self::with_server_days(3)
    }
    fn with_server_days(server_days: u32) -> Self {
        let root = std::env::temp_dir().join(format!("bw-mtls-test-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let keys = Arc::new(MemoryKeys::default());
        let store = ProfileStore::open(&root, SecretStore::with_keys(&root, keys)).unwrap();
        let (cakey, ca) = cert("Test CA", None, 7);
        let (serverkey, server) = cert("localhost", Some((&ca, &cakey)), server_days);
        let (client, leaf) = cert("Client A", Some((&ca, &cakey)), 3);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let mut profile = ServerProfile::new(&format!("https://localhost:{port}/vault/")).unwrap();
        profile.server_ca_pem = Some(String::from_utf8(ca.to_pem().unwrap()).unwrap());
        let profile = store.save(profile, None).unwrap();
        let mut tls = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        tls.set_session_id_context(b"bw-native-test").unwrap();
        tls.set_private_key(&serverkey).unwrap();
        tls.set_certificate(&server).unwrap();
        tls.cert_store_mut().add_cert(ca.clone()).unwrap();
        tls.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);
        let tls = tls.build();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let received = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let raw = crate::services::sdk::test_support::raw_client();
        let registration = raw
            .0
            .auth()
            .make_register_keys(
                "test@example.invalid".into(),
                "synthetic-password".into(),
                bitwarden_crypto::Kdf::default_pbkdf2(),
            )
            .unwrap();
        let response=serde_json::json!({"access_token":"synthetic-access-token","refresh_token":"synthetic-refresh-token","expires_in":3600,"key":registration.encrypted_user_key.to_string(),"privateKey":registration.keys.private.to_string()}).to_string();
        let sync_response = Arc::new(Mutex::new((500, serde_json::json!({}))));
        let sync_payload = sync_response.clone();
        let sync_delay = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let server_delay = sync_delay.clone();
        let thread = std::thread::spawn(move || {
            while !done.load(Ordering::SeqCst) {
                let (stream, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                        continue;
                    }
                    Err(_) => break,
                };
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let mut stream = match tls.accept(stream) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = e;
                        continue;
                    }
                };
                let fingerprint = stream
                    .ssl()
                    .peer_certificate()
                    .unwrap()
                    .digest(MessageDigest::sha256())
                    .unwrap()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                let mut bytes = Vec::new();
                let mut buf = [0; 4096];
                let mut expected = None;
                loop {
                    let n = match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    bytes.extend_from_slice(&buf[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let len = headers
                            .lines()
                            .find_map(|l| {
                                l.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|n| n.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        expected = Some(end + 4 + len);
                    }
                    if expected.is_some_and(|n| bytes.len() >= n) {
                        break;
                    }
                }
                let req = String::from_utf8_lossy(&bytes).to_string();
                let first = req.lines().next().unwrap_or("").to_string();
                received.lock().unwrap().push((req.clone(), fingerprint));
                let (status, body, extra) = if first.contains("/redirect ") {
                    (
                        "302 Found",
                        "{}".into(),
                        "Location: https://untrusted.invalid/\r\n",
                    )
                } else if first.contains("/sync?") {
                    let (status, mut value) = sync_payload.lock().unwrap().clone();
                    // Match Vaultwarden: an absent/old compatibility version
                    // silently excludes SSH ciphers from an otherwise valid sync.
                    let version = req
                        .split("\r\n\r\n")
                        .next()
                        .unwrap()
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("bitwarden-client-version")
                                .then(|| {
                                    value
                                        .trim()
                                        .split('.')
                                        .map(str::parse::<u32>)
                                        .collect::<Result<Vec<_>, _>>()
                                        .ok()
                                })
                                .flatten()
                        });
                    let ssh_supported =
                        version.is_some_and(|v| v.as_slice() >= [2024, 12, 0].as_slice());
                    if !ssh_supported
                        && let Some(ciphers) = value
                            .get_mut("ciphers")
                            .and_then(serde_json::Value::as_array_mut)
                    {
                        ciphers.retain(|c| {
                            c.get("type").and_then(serde_json::Value::as_u64) != Some(5)
                        });
                    }
                    std::thread::sleep(std::time::Duration::from_millis(
                        server_delay.load(Ordering::SeqCst),
                    ));
                    (
                        if status == 200 {
                            "200 OK"
                        } else {
                            "500 Internal Server Error"
                        },
                        value.to_string(),
                        "",
                    )
                } else if first.contains("/config ") {
                    ("200 OK", "{}".into(), "")
                } else if first.contains("/prelogin") {
                    ("200 OK", "{\"kdf\":0,\"kdfIterations\":600000}".into(), "")
                } else if first.contains("/connect/token ") {
                    if req.contains("twoFactorToken=123456")
                        || req.contains("grant_type=refresh_token")
                    {
                        ("200 OK", response.clone(), "")
                    } else {
                        (
                            "400 Bad Request",
                            "{\"TwoFactorProviders2\":{\"0\":null,\"1\":{}}}".into(),
                            "",
                        )
                    }
                } else if first.contains("/accounts/profile ") {
                    ("200 OK","{\"id\":\"00000000-0000-0000-0000-000000000042\",\"email\":\"test@example.invalid\",\"name\":\"Test\"}".into(),"")
                } else {
                    ("200 OK", "{}".into(), "")
                };
                let out = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(out.as_bytes());
            }
        });
        Self {
            root,
            store,
            profile,
            stop,
            thread: Some(thread),
            requests,
            client,
            leaf,
            ca,
            ca_key: cakey,
            sync_response,
            sync_delay,
        }
    }
    fn bundle(&self, name: &str, key: &PKey<Private>, leaf: &X509) -> PathBuf {
        let mut chain = Stack::new().unwrap();
        chain.push(self.ca.clone()).unwrap();
        let mut b = openssl::pkcs12::Pkcs12::builder();
        b.name("test").pkey(key).cert(leaf).ca(chain);
        let der = b.build2("test-password").unwrap().to_der().unwrap();
        let path = self.root.join(name);
        std::fs::write(&path, der).unwrap();
        path
    }
    fn import(&self) -> ServerProfile {
        let path = self.bundle("client.p12", &self.client, &self.leaf);
        certificate::import(
            &self.store,
            self.profile.clone(),
            &path,
            Zeroizing::new("test-password".into()),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            t.join().unwrap()
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
#[tokio::test]
async fn tls_requires_identity_and_scopes_sdk_and_login_through_two_factor() {
    let fixture = Fixture::new();
    let anonymous = Transport::new(&fixture.store, fixture.profile.clone()).unwrap();
    let url = fixture.profile.endpoint("api/config").unwrap();
    let req = anonymous
        .request(reqwest_account::Method::GET, url)
        .unwrap()
        .build()
        .unwrap();
    assert!(anonymous.execute(req).await.is_err());
    let profile = fixture.import();
    let transport = Transport::new(&fixture.store, profile.clone()).unwrap();
    let pending = PendingLogin::prepare(
        Attempt::new(),
        transport.clone(),
        "test@example.invalid".into(),
        Zeroizing::new("synthetic-password".into()),
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    let Step::Challenge(pending, factors) = pending.submit(None).await.unwrap() else {
        panic!("Expected challenge")
    };
    assert_eq!(factors, vec![Factor::Authenticator, Factor::Email]);
    pending.send_email().await.unwrap();
    let attempt = pending.attempt.clone();
    let Step::Authenticated(success) = pending
        .submit(Some((
            Factor::Authenticator,
            Zeroizing::new("123456".into()),
        )))
        .await
        .unwrap()
    else {
        panic!("Expected success")
    };
    let candidate = crate::services::sdk::production::prepare(
        fixture.root.clone(),
        fixture.store.clone(),
        attempt.clone(),
        *success,
    )
    .await
    .unwrap();
    let mut manager = crate::services::sdk::ClientManager::empty();
    let uid = manager.accept_login(candidate, &attempt).unwrap();
    assert!(manager.is_unlocked(&uid));
    assert!(manager.is_remote(&uid));
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .create_folder("blocked".into())
            .await
            .is_err()
    );
    // Force expiry and race two probes: exactly one refresh is committed.
    let record_path = fixture.root.join("accounts").join(format!("{uid}.json"));
    let mut record: super::login::AccountRecord =
        serde_json::from_slice(&std::fs::read(&record_path).unwrap()).unwrap();
    let old = record.credentials;
    let mut credentials: super::login::Credentials =
        serde_json::from_slice(&fixture.store.secrets.get(old).unwrap()).unwrap();
    credentials.expires_at = 0;
    record.credentials = fixture
        .store
        .secrets
        .put(&serde_json::to_vec(&credentials).unwrap())
        .unwrap();
    std::fs::write(&record_path, serde_json::to_vec(&record).unwrap()).unwrap();
    fixture.store.secrets.delete(old).unwrap();
    let one = crate::services::sdk::refresh::probe(
        manager.client_for(&uid).unwrap(),
        fixture.root.clone(),
        fixture.store.clone(),
    );
    let two = crate::services::sdk::refresh::probe(
        manager.client_for(&uid).unwrap(),
        fixture.root.clone(),
        fixture.store.clone(),
    );
    let (a, b) = tokio::join!(one, two);
    a.unwrap();
    b.unwrap();
    assert_eq!(
        fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(r, _)| r.contains("grant_type=refresh_token"))
            .count(),
        1
    );
    assert!(
        fixture.store.secrets.get(record.credentials).is_err(),
        "Old refresh credential key is deleted"
    );
    let metadata = std::fs::read_to_string(&record_path).unwrap();
    assert!(!metadata.contains("synthetic-access-token"));
    assert!(!metadata.contains("synthetic-refresh-token"));
    manager.lock(&uid).unwrap().finish().await;
    drop(manager);
    let mut restored =
        crate::services::sdk::production::test_load(&fixture.root, &fixture.store).await;
    assert!(!restored.is_unlocked(&uid));
    restored
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    assert!(restored.is_unlocked(&uid));
    let request = transport
        .request(
            reqwest_account::Method::GET,
            profile.endpoint("redirect").unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    assert!(transport.execute(request).await.is_err());
    let request = transport.request(
        reqwest_account::Method::GET,
        url::Url::parse("https://localhost:1/").unwrap(),
    );
    assert!(request.is_err());
    let client = transport
        .configure_sdk(bitwarden_core::ClientBuilder::new().with_settings(
            bitwarden_core::ClientSettings {
                identity_url: profile.endpoint("identity").unwrap().to_string(),
                ..Default::default()
            },
        ))
        .build();
    client
        .auth()
        .prelogin("test@example.invalid".into())
        .await
        .unwrap();
    {
        let requests = fixture.requests.lock().unwrap();
        assert!(
            requests.iter().all(|(_, fingerprint)| fingerprint
                == &profile.certificate.as_ref().unwrap().fingerprint)
        );
        assert!(
            requests
                .iter()
                .all(|(r, _)| !r.contains("synthetic-password"))
        );
        assert!(
            requests
                .iter()
                .any(|(r, _)| r.contains("Bearer synthetic-access-token"))
        );
        assert!(
            requests
                .iter()
                .filter(|(r, _)| !r.contains("/accounts/profile "))
                .all(|(r, _)| !r.to_lowercase().contains("authorization:"))
        );
    }
    let current: super::login::AccountRecord =
        serde_json::from_slice(&std::fs::read(&record_path).unwrap()).unwrap();
    restored.lock(&uid).unwrap().finish().await;
    drop(restored);
    crate::services::sdk::production::remove_record_with_secrets(
        &fixture.root,
        uid,
        &fixture.store.secrets,
    )
    .unwrap();
    assert!(fixture.store.secrets.get(current.credentials).is_err());
    assert!(!record_path.exists());
    let revision = profile.revision;
    fixture.store.save(profile, Some(revision)).unwrap();
    assert!(
        transport
            .check(&transport.profile.default_api().unwrap())
            .is_err()
    );
    use crate::services::sdk::ClientExt;
}
#[test]
fn wrong_password_and_expired_identity_never_replace_saved_profile() {
    let f = Fixture::new();
    let p = f.import();
    let path = f.bundle("new.p12", &f.client, &f.leaf);
    assert!(
        certificate::import(&f.store, p.clone(), &path, Zeroizing::new("wrong".into())).is_err()
    );
    assert_eq!(f.store.get(p.id).unwrap().revision, p.revision);
    let (key, expired) = cert("Expired", Some((&f.ca, &f.client)), 0);
    let path = f.bundle("expired.p12", &key, &expired);
    assert!(
        certificate::import(
            &f.store,
            p.clone(),
            &path,
            Zeroizing::new("test-password".into())
        )
        .is_err()
    );
    assert_eq!(f.store.get(p.id).unwrap().revision, p.revision);
}

#[tokio::test]
async fn same_host_profiles_keep_distinct_client_identities_and_ca_trust_is_explicit() {
    let f = Fixture::new();
    let a = f.import();
    let ta = Transport::new(&f.store, a.clone()).unwrap();
    let (key, leaf) = cert("Client B", Some((&f.ca, &f.ca_key)), 2);
    let path = f.bundle("b.p12", &key, &leaf);
    let mut b = f.profile.clone();
    b.id = Uuid::new_v4();
    b.revision = 0;
    let b = f.store.save(b, None).unwrap();
    let b =
        certificate::import(&f.store, b, &path, Zeroizing::new("test-password".into())).unwrap();
    let tb = Transport::new(&f.store, b.clone()).unwrap();
    for transport in [&ta, &tb, &ta] {
        let req = transport
            .request(
                reqwest_account::Method::GET,
                transport.profile.endpoint("api/config").unwrap(),
            )
            .unwrap()
            .build()
            .unwrap();
        transport.execute(req).await.unwrap();
    }
    let fingerprints = f
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|(_, fp)| fp.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        fingerprints,
        vec![
            a.certificate.as_ref().unwrap().fingerprint.clone(),
            b.certificate.as_ref().unwrap().fingerprint.clone(),
            a.certificate.as_ref().unwrap().fingerprint.clone()
        ]
    );
    let mut untrusted = a.clone();
    untrusted.id = Uuid::new_v4();
    untrusted.revision = 0;
    untrusted.server_ca_pem = None;
    let untrusted = f.store.save(untrusted, None).unwrap();
    let t = Transport::new(&f.store, untrusted).unwrap();
    let req = t
        .request(
            reqwest_account::Method::GET,
            t.profile.endpoint("api/config").unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    assert!(
        t.execute(req).await.is_err(),
        "Importing client chain does not trust the server CA"
    );
    let mut expired = ta.clone();
    expired.profile.certificate.as_mut().unwrap().not_after = 0;
    assert!(
        expired
            .request(
                reqwest_account::Method::GET,
                ta.profile.default_api().unwrap()
            )
            .is_err()
    );
    assert_eq!(
        f.requests.lock().unwrap().len(),
        3,
        "Expired certificate cannot reuse an existing connection"
    );
}

#[tokio::test]
async fn expired_server_certificate_has_specific_error_and_never_sends_http() {
    let f = Fixture::with_server_days(0);
    let profile = f.import();
    let t = Transport::new(&f.store, profile).unwrap();
    let req = t
        .request(
            reqwest_account::Method::GET,
            t.profile.endpoint("api/config").unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let message = t.execute(req).await.unwrap_err();
    assert!(
        message.contains("Servers oder seiner Zertifikatskette ist abgelaufen"),
        "{message}"
    );
    assert!(
        f.requests.lock().unwrap().is_empty(),
        "No HTTP credentials are sent after a failed TLS validation"
    );
    let client = t
        .configure_sdk(bitwarden_core::ClientBuilder::new().with_settings(
            bitwarden_core::ClientSettings {
                identity_url: t.profile.endpoint("identity").unwrap().to_string(),
                ..Default::default()
            },
        ))
        .build();
    let error = client
        .auth()
        .prelogin("test@example.invalid".into())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("Servers oder seiner Zertifikatskette ist abgelaufen"),
        "{error}"
    );
}

impl Fixture {
    async fn account(&self) -> (crate::services::sdk::ClientManager, crate::domain::UserId) {
        let profile = self
            .store
            .get(self.profile.id)
            .filter(|p| p.certificate.is_some())
            .unwrap_or_else(|| self.import());
        let t = Transport::new(&self.store, profile).unwrap();
        let attempt = Attempt::new();
        let pending = PendingLogin::prepare(
            attempt.clone(),
            t,
            "test@example.invalid".into(),
            Zeroizing::new("synthetic-password".into()),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
        let Step::Authenticated(success) = pending
            .submit(Some((
                Factor::Authenticator,
                Zeroizing::new("123456".into()),
            )))
            .await
            .unwrap()
        else {
            panic!("Expected authenticated test account")
        };
        let candidate = crate::services::sdk::production::prepare(
            self.root.clone(),
            self.store.clone(),
            attempt.clone(),
            *success,
        )
        .await
        .unwrap();
        let mut manager = crate::services::sdk::ClientManager::empty();
        let uid = manager.accept_login(candidate, &attempt).unwrap();
        (manager, uid)
    }
    fn sync_requests(&self) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(req, _)| req.lines().next().is_some_and(|l| l.contains("/sync?")))
            .count()
    }
}
#[tokio::test]
async fn mtls_read_sync_is_atomic_coalesced_read_only_and_survives_offline_restart() {
    use crate::services::sdk::{ClientExt, test_support::encrypted_sync_response, vault_sync};
    let f = Fixture::new();
    let (mut manager, uid) = f.account().await;
    let payload = encrypted_sync_response(&manager, uid, "Synthetic sync first item").await;
    *f.sync_response.lock().unwrap() = (200, payload.clone());
    f.sync_delay.store(40, Ordering::SeqCst);
    let stale = manager.client_for(&uid).unwrap();
    let a = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    );
    let b = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    );
    let (a, b) = tokio::join!(a, b);
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.ciphers, 1);
    assert_eq!(a.downloaded_at, b.downloaded_at);
    assert_eq!(f.sync_requests(), 1);
    assert!(
        stale.list_ciphers().await.is_err(),
        "Snapshot commit revokes queued work from the old generation"
    );
    let rows = manager
        .client_for(&uid)
        .unwrap()
        .list_ciphers()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Synthetic sync first item");
    let id = rows[0].id.unwrap();
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .login
            .unwrap()
            .password
            .as_deref(),
        Some("synthetic sync vault secret")
    );
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .list_folders()
            .await
            .unwrap()[0]
            .name,
        "Synthetic encrypted folder"
    );
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .create_folder("Blocked server write".into())
            .await
            .is_err()
    );
    let first_time = manager.last_sync(&uid).unwrap();
    crate::services::sdk::test_support::retain_sync_gui_fixture(&f.root);
    *f.sync_response.lock().unwrap() = (
        500,
        serde_json::json!({"message":"Never expose server message"}),
    );
    assert!(
        vault_sync::synchronize(
            manager.client_for(&uid).unwrap(),
            f.root.clone(),
            f.store.clone()
        )
        .await
        .is_err()
    );
    assert_eq!(manager.last_sync(&uid), Some(first_time));
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .list_ciphers()
            .await
            .unwrap()
            .len(),
        1
    );
    let mut malformed = payload.clone();
    malformed.as_object_mut().unwrap().remove("ciphers");
    *f.sync_response.lock().unwrap() = (200, malformed);
    assert!(
        vault_sync::synchronize(
            manager.client_for(&uid).unwrap(),
            f.root.clone(),
            f.store.clone()
        )
        .await
        .is_err()
    );
    assert_eq!(manager.last_sync(&uid), Some(first_time));
    manager.lock(&uid).unwrap().finish().await;
    drop(manager);
    // Restart while all network operations are blocked: local SDK unlock still reads the committed snapshot.
    let requests_before = f.requests.lock().unwrap().len();
    let restored = crate::services::sdk::production::test_load(&f.root, &f.store).await;
    assert!(!restored.is_unlocked(&uid));
    restored
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    assert_eq!(
        restored
            .client_for(&uid)
            .unwrap()
            .list_ciphers()
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(restored.last_sync(&uid), Some(first_time));
    assert_eq!(f.requests.lock().unwrap().len(), requests_before);

    let mut empty = payload;
    empty["ciphers"] = serde_json::json!([]);
    empty["folders"] = serde_json::json!([]);
    *f.sync_response.lock().unwrap() = (200, empty);
    vault_sync::synchronize(
        restored.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(
        restored
            .client_for(&uid)
            .unwrap()
            .list_ciphers()
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        restored
            .client_for(&uid)
            .unwrap()
            .list_folders()
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        f.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(r, _)| r.contains("/sync?"))
            .all(|(r, _)| r.contains("Bearer synthetic-access-token"))
    );
    for suffix in [".sqlite", ".sqlite-wal"] {
        let path = f.root.join(format!("{uid}{suffix}"));
        if let Ok(bytes) = std::fs::read(path) {
            for secret in [
                "Synthetic sync first item",
                "synthetic sync vault secret",
                "synthetic-password",
                "synthetic-access-token",
            ] {
                assert!(
                    !bytes.windows(secret.len()).any(|w| w == secret.as_bytes()),
                    "Plaintext persisted: {secret}"
                );
            }
        }
    }
}

#[tokio::test]
async fn sync_org_removal_clears_keys_and_failed_key_update_restores_the_previous_snapshot() {
    use crate::services::sdk::{
        ClientExt,
        test_support::{encrypted_org_sync_response, has_org_key},
        vault_sync,
    };
    let f = Fixture::new();
    let (mut manager, uid) = f.account().await;
    let (payload, org, id) = encrypted_org_sync_response(&manager, uid).await;
    *f.sync_response.lock().unwrap() = (200, payload.clone());
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(manager.list_organizations(&uid).len(), 1);
    assert_eq!(
        manager.list_collections(&uid)[0].name,
        "Synthetic secret collection"
    );
    assert!(has_org_key(&manager, uid, org));
    let first = manager.last_sync(&uid);
    let mut bad = payload.clone();
    bad["profile"]["organizations"][0]["key"] = serde_json::json!("4.AA==");
    *f.sync_response.lock().unwrap() = (200, bad);
    assert!(
        vault_sync::synchronize(
            manager.client_for(&uid).unwrap(),
            f.root.clone(),
            f.store.clone()
        )
        .await
        .is_err()
    );
    assert_eq!(manager.last_sync(&uid), first);
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .login
            .unwrap()
            .password
            .as_deref(),
        Some("synthetic organization secret")
    );
    manager.lock(&uid).unwrap().finish().await;
    assert!(manager.list_collections(&uid).is_empty());
    manager
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    assert_eq!(manager.list_collections(&uid).len(), 1);
    let mut empty = payload;
    empty["profile"]["organizations"] = serde_json::json!([]);
    empty["ciphers"] = serde_json::json!([]);
    empty["collections"] = serde_json::json!([]);
    *f.sync_response.lock().unwrap() = (200, empty);
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(manager.list_organizations(&uid).is_empty());
    assert!(manager.list_collections(&uid).is_empty());
    assert!(!has_org_key(&manager, uid, org));
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn locking_during_sync_fetch_discards_the_response_without_committing() {
    use crate::services::sdk::{ClientExt, test_support::encrypted_sync_response, vault_sync};
    let f = Fixture::new();
    let (mut manager, uid) = f.account().await;
    let payload = encrypted_sync_response(&manager, uid, "Before lock").await;
    *f.sync_response.lock().unwrap() = (200, payload.clone());
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    let old = manager.last_sync(&uid);
    let next = encrypted_sync_response(&manager, uid, "Must never commit after revocation").await;
    *f.sync_response.lock().unwrap() = (200, next);
    f.sync_delay.store(150, Ordering::SeqCst);
    let before = f.sync_requests();
    let task = tokio::spawn(vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    ));
    for _ in 0..100 {
        if f.sync_requests() > before {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(f.sync_requests() > before);
    let lock = manager.lock(&uid).unwrap();
    assert!(task.await.unwrap().is_err());
    lock.finish().await;
    assert_eq!(manager.last_sync(&uid), old);
    manager
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .list_ciphers()
            .await
            .unwrap()[0]
            .name,
        "Before lock"
    );
}

#[tokio::test]
async fn same_server_account_id_and_email_have_independent_local_sync_snapshots() {
    use crate::services::sdk::{ClientExt, test_support::encrypted_sync_response, vault_sync};
    let f = Fixture::new();
    let (mut a, first) = f.account().await;
    let (b, second) = f.account().await;
    assert_ne!(first, second);
    *f.sync_response.lock().unwrap() = (
        200,
        encrypted_sync_response(&a, first, "First local account snapshot").await,
    );
    vault_sync::synchronize(
        a.client_for(&first).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    *f.sync_response.lock().unwrap() = (
        200,
        encrypted_sync_response(&b, second, "Second local account snapshot").await,
    );
    vault_sync::synchronize(
        b.client_for(&second).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        a.client_for(&first).unwrap().list_ciphers().await.unwrap()[0].name,
        "First local account snapshot"
    );
    assert_eq!(
        b.client_for(&second).unwrap().list_ciphers().await.unwrap()[0].name,
        "Second local account snapshot"
    );
    a.lock(&first).unwrap().finish().await;
    assert!(b.is_unlocked(&second));
    assert_eq!(
        b.client_for(&second)
            .unwrap()
            .list_ciphers()
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn version_gated_ssh_sync_reaches_vault_list_and_agent_through_mtls() {
    use crate::services::sdk::{ClientExt, test_support::encrypted_ssh_sync_response, vault_sync};
    let f = Fixture::new();
    let (manager, uid) = f.account().await;
    let (payload, public) = encrypted_ssh_sync_response(&manager, uid).await;
    *f.sync_response.lock().unwrap() = (200, payload);
    // An explicitly old client still receives a successful but incomplete sync.
    let transport = Transport::new(&f.store, f.store.get(f.profile.id).unwrap()).unwrap();
    let url = transport
        .profile
        .default_api()
        .unwrap()
        .join("sync?excludeDomains=true")
        .unwrap();
    let old = transport
        .request(reqwest_account::Method::GET, url)
        .unwrap()
        .header("bitwarden-client-version", "0.1.0")
        .bearer_auth("synthetic-access-token")
        .build()
        .unwrap();
    let old = super::transport::json_body(transport.execute(old).await.unwrap())
        .await
        .unwrap();
    assert_eq!(old["ciphers"].as_array().unwrap().len(), 1);
    let result = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        result.ciphers, 2,
        "server must not suppress the SSH item as an old client"
    );
    let items = manager
        .client_for(&uid)
        .unwrap()
        .list_ciphers()
        .await
        .unwrap();
    assert_eq!(
        items
            .iter()
            .filter(|c| matches!(&c.r#type, bitwarden_vault::CipherListViewType::SshKey))
            .count(),
        1
    );
    let keys = manager
        .agent_session_for(&uid)
        .unwrap()
        .identities()
        .await
        .unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].public_key, public);
    let requests = f.requests.lock().unwrap();
    let request = requests
        .iter()
        .rev()
        .find(|(req, _)| req.lines().next().unwrap_or("").contains("/sync?"))
        .unwrap();
    assert!(request.0.to_ascii_lowercase().contains(&format!(
        "bitwarden-client-version: {}",
        super::transport::BITWARDEN_PROTOCOL_VERSION
    )));
}

#[tokio::test]
async fn local_edits_are_encrypted_durable_revision_checked_and_never_uploaded() {
    use crate::services::sdk::{ClientExt, test_support::encrypted_sync_response, vault_sync};
    let f = Fixture::new();
    let (mut manager, uid) = f.account().await;
    *f.sync_response.lock().unwrap() = (
        200,
        encrypted_sync_response(&manager, uid, "Original server entry").await,
    );
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    let id = manager
        .client_for(&uid)
        .unwrap()
        .list_ciphers()
        .await
        .unwrap()[0]
        .id
        .unwrap();
    let mut draft = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    let stale = draft.clone();
    let mut tampered = draft.clone();
    tampered.edit = false;
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .save_cipher(tampered)
            .await
            .is_err()
    );
    assert_eq!(manager.local_edit_count(&uid), 0);
    draft.name = "Only local edited title".into();
    draft.login.as_mut().unwrap().username = Some("local-user-only".into());
    draft.login.as_mut().unwrap().password = Some("local-password-only".into());
    let requests = f.requests.lock().unwrap().len();
    let saved = manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(draft)
        .await
        .unwrap();
    assert!(saved.revision_date > stale.revision_date);
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .save_cipher(stale)
            .await
            .is_err()
    );
    assert_eq!(manager.local_edit_count(&uid), 1);
    assert!(manager.has_local_edit(&uid, id));
    assert!(
        vault_sync::synchronize(
            manager.client_for(&uid).unwrap(),
            f.root.clone(),
            f.store.clone()
        )
        .await
        .unwrap_err()
        .contains("local changes")
    );
    assert_eq!(
        f.requests.lock().unwrap().len(),
        requests,
        "Local save and paused sync must send no HTTP requests"
    );
    for suffix in [".sqlite", ".sqlite-wal"] {
        if let Ok(bytes) = std::fs::read(f.root.join(format!("{uid}{suffix}"))) {
            for secret in [
                "Only local edited title",
                "local-user-only",
                "local-password-only",
            ] {
                assert!(
                    !bytes.windows(secret.len()).any(|w| w == secret.as_bytes()),
                    "Local plaintext persisted"
                );
            }
        }
    }
    manager.lock(&uid).unwrap().finish().await;
    drop(manager);
    let restored = crate::services::sdk::production::test_load(&f.root, &f.store).await;
    restored
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    let edited = restored
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    assert_eq!(edited.name, "Only local edited title");
    assert_eq!(
        edited.login.unwrap().password.as_deref(),
        Some("local-password-only")
    );
    assert_eq!(restored.local_edit_count(&uid), 1);
    assert_eq!(f.requests.lock().unwrap().len(), requests);
    restored
        .client_for(&uid)
        .unwrap()
        .discard_local(id)
        .await
        .unwrap();
    assert_eq!(restored.local_edit_count(&uid), 0);
    assert_eq!(
        restored
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .name,
        "Original server entry"
    );
    vault_sync::synchronize(
        restored.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(
        f.requests
            .lock()
            .unwrap()
            .iter()
            .all(|(r, _)| !r.starts_with("PUT ") && !r.starts_with("DELETE "))
    );
}

#[tokio::test]
async fn local_edit_requires_current_permissions_and_one_shot_password_authorization() {
    use crate::services::sdk::{
        Action, Authorization, ClientExt, test_support::encrypted_sync_response, vault_sync,
    };
    let f = Fixture::new();
    let (manager, uid) = f.account().await;
    let mut response = encrypted_sync_response(&manager, uid, "Protected entry").await;
    response["ciphers"][0]["reprompt"] = serde_json::json!(1);
    *f.sync_response.lock().unwrap() = (200, response.clone());
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    let id = manager
        .client_for(&uid)
        .unwrap()
        .list_ciphers()
        .await
        .unwrap()[0]
        .id
        .unwrap();
    // A protected read requires its own, distinct grant.
    let (client, envelope) = manager.validation_data_for(&uid).unwrap();
    let revision = manager
        .client_for(&uid)
        .unwrap()
        .list_ciphers()
        .await
        .unwrap()[0]
        .revision_date;
    client
        .authorize_password(
            envelope.clone(),
            "synthetic-password".into(),
            Authorization {
                cipher: Some(id),
                revision: Some(revision),
                action: Action::Read,
            },
        )
        .await
        .unwrap();
    let mut draft = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    draft.name = "Protected local edit".into();
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .save_cipher(draft.clone())
            .await
            .is_err()
    );
    manager
        .client_for(&uid)
        .unwrap()
        .authorize_password(
            envelope,
            "synthetic-password".into(),
            Authorization {
                cipher: Some(id),
                revision: Some(revision),
                action: Action::Write,
            },
        )
        .await
        .unwrap();
    manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(draft.clone())
        .await
        .unwrap();
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .save_cipher(draft)
            .await
            .is_err()
    );
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .discard_local(id)
            .await
            .is_err(),
        "Discard also requires a fresh password grant"
    );
    let revision = manager
        .client_for(&uid)
        .unwrap()
        .list_ciphers()
        .await
        .unwrap()[0]
        .revision_date;
    let (client, envelope) = manager.validation_data_for(&uid).unwrap();
    client
        .authorize_password(
            envelope,
            "synthetic-password".into(),
            Authorization {
                cipher: Some(id),
                revision: Some(revision),
                action: Action::Write,
            },
        )
        .await
        .unwrap();
    manager
        .client_for(&uid)
        .unwrap()
        .discard_local(id)
        .await
        .unwrap();
    response["ciphers"][0]["edit"] = serde_json::json!(false);
    *f.sync_response.lock().unwrap() = (200, response);
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher_for(id, Action::Edit)
            .await
            .is_err()
    );
    assert_eq!(manager.local_edit_count(&uid), 0);
}

#[tokio::test]
async fn local_org_edits_keep_ownership_and_survive_key_reload() {
    use crate::services::sdk::{ClientExt, test_support::encrypted_org_sync_response, vault_sync};
    let f = Fixture::new();
    let (mut manager, uid) = f.account().await;
    let (mut response, org, id) = encrypted_org_sync_response(&manager, uid).await;
    response["ciphers"][0]["edit"] = serde_json::json!(true);
    response["collections"][0]["readOnly"] = serde_json::json!(false);
    *f.sync_response.lock().unwrap() = (200, response);
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    let mut draft = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    let mut moved = draft.clone();
    moved.organization_id = None;
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .save_cipher(moved)
            .await
            .is_err()
    );
    draft.name = "Locally edited organization entry".into();
    manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(draft)
        .await
        .unwrap();
    manager.lock(&uid).unwrap().finish().await;
    drop(manager);
    let restored = crate::services::sdk::production::test_load(&f.root, &f.store).await;
    restored
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    let view = restored
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    assert_eq!(view.organization_id, Some(org));
    assert_eq!(view.name, "Locally edited organization entry");
    restored
        .client_for(&uid)
        .unwrap()
        .discard_local(id)
        .await
        .unwrap();
    assert_eq!(restored.local_edit_count(&uid), 0);
}
