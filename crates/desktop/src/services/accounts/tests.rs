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
    login_policy: Arc<Mutex<Option<serde_json::Value>>>,
    write_mode: Arc<std::sync::atomic::AtomicU8>,
    folder_mode: Arc<std::sync::atomic::AtomicU8>,
    attachment_mode: Arc<std::sync::atomic::AtomicU8>,
    attachment_files: Arc<Mutex<std::collections::HashMap<String, Vec<u8>>>>,
    revision_override: Arc<Mutex<Option<i64>>>,
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
        let login_policy = Arc::new(Mutex::new(None::<serde_json::Value>));
        let server_login_policy = login_policy.clone();
        // 0 reject; 1 accept; 2 apply but lose reply; 3 lose before apply;
        // 4 concurrent server change; 7 slow reply; 8 accept one then reject.
        let write_mode = Arc::new(std::sync::atomic::AtomicU8::new(0));
        let server_write_mode = write_mode.clone();
        let folder_mode = Arc::new(std::sync::atomic::AtomicU8::new(1));
        let server_folder_mode = folder_mode.clone();
        let attachment_mode = Arc::new(std::sync::atomic::AtomicU8::new(1));
        let server_attachment_mode = attachment_mode.clone();
        let attachment_files = Arc::new(Mutex::new(
            std::collections::HashMap::<String, Vec<u8>>::new(),
        ));
        let server_files = attachment_files.clone();
        let revision_override = Arc::new(Mutex::new(None::<i64>));
        let server_revision_override = revision_override.clone();
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
                let mut parts = first.split_whitespace();
                let verb = parts.next().unwrap_or("");
                let path = parts.next().unwrap_or("");
                if path.starts_with("/vault/attachments/") {
                    let mode = server_attachment_mode.load(Ordering::SeqCst);
                    let file_path = path.split('?').next().unwrap();
                    let data = server_files.lock().unwrap().get(file_path).cloned();
                    if let Some(mut data) = data {
                        if mode == 7 && !data.is_empty() {
                            let last = data.len() - 1;
                            data[last] ^= 0x80;
                        }
                        if mode == 14 {
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        }
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            if mode == 13 {
                                500 * 1024 * 1024 + 2048
                            } else {
                                data.len()
                            }
                        );
                        let _ = stream.write_all(header.as_bytes());
                        if mode == 15 {
                            let n = data.len().min(4);
                            let _ = stream.write_all(&data[..n]);
                            std::thread::sleep(std::time::Duration::from_millis(500));
                            let _ = stream.write_all(&data[n..]);
                        } else {
                            let _ = stream.write_all(&data);
                        }
                    } else {
                        let _=stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    }
                    continue;
                }
                if let Some(rest) = path.strip_prefix("/vault/api/ciphers/")
                    && rest.contains("/attachment/")
                {
                    let (cid, aid) = rest.split_once("/attachment/").unwrap();
                    let mode = server_attachment_mode.load(Ordering::SeqCst);
                    let mut payload = sync_payload.lock().unwrap();
                    let cipher = payload.1["ciphers"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|c| c["id"].as_str() == Some(cid))
                        .unwrap();
                    let file_path = format!("/vault/attachments/{cid}/{aid}");
                    if verb == "POST" && aid != "v2" {
                        if mode == 4 {
                            continue;
                        }
                        let head_end = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
                        let header = String::from_utf8_lossy(&bytes[..head_end]);
                        let boundary = header
                            .lines()
                            .find_map(|line| {
                                line.split_once("boundary=")
                                    .map(|(_, v)| v.trim().trim_matches('"'))
                            })
                            .unwrap();
                        let body = &bytes[head_end + 4..];
                        let begin = body.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
                        let delimiter = format!("\r\n--{boundary}--");
                        let end = body
                            .windows(delimiter.len())
                            .position(|w| w == delimiter.as_bytes())
                            .unwrap();
                        server_files
                            .lock()
                            .unwrap()
                            .insert(file_path, body[begin..end].to_vec());
                        if mode == 3 {
                            continue;
                        }
                        if mode == 8 {
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        }
                        let _ = stream.write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                        continue;
                    }
                    let mut status = "200 OK";
                    let result = if verb == "POST" && aid == "v2" {
                        if mode == 5 {
                            status = "400 Bad Request";
                            serde_json::json!({})
                        } else {
                            let body: serde_json::Value =
                                serde_json::from_str(req.split_once("\r\n\r\n").unwrap().1)
                                    .unwrap();
                            let id = Uuid::new_v4().simple().to_string();
                            let attachment = serde_json::json!({"id":id,"fileName":body["fileName"],"key":body["key"],"size":body["fileSize"].to_string(),"sizeName":"fixture bytes","url":format!("https://localhost:{port}/vault/attachments/{cid}/{id}?token=synthetic-file-token")});
                            if !cipher["attachments"].is_array() {
                                cipher["attachments"] = serde_json::json!([]);
                            }
                            cipher["attachments"]
                                .as_array_mut()
                                .unwrap()
                                .push(attachment);
                            if mode == 2 {
                                continue;
                            }
                            serde_json::json!({"attachmentId":id,"fileUploadType":0,"url":if mode==11 {"https://untrusted.invalid/upload".to_owned()} else {format!("/ciphers/{cid}/attachment/{id}")},"cipherResponse":cipher})
                        }
                    } else if verb == "DELETE" {
                        cipher["attachments"]
                            .as_array_mut()
                            .unwrap()
                            .retain(|a| a["id"].as_str() != Some(aid));
                        server_files.lock().unwrap().remove(&file_path);
                        if mode == 9 {
                            continue;
                        }
                        serde_json::json!({})
                    } else if verb == "GET" {
                        let mut metadata = cipher["attachments"]
                            .as_array()
                            .and_then(|v| v.iter().find(|a| a["id"].as_str() == Some(aid)))
                            .cloned()
                            .unwrap_or(serde_json::json!({}));
                        if mode == 6 {
                            metadata["url"] =
                                serde_json::json!("https://untrusted.invalid/attachments/file");
                        }
                        if mode == 10 {
                            metadata["id"] = serde_json::json!("other-id");
                        }
                        metadata
                    } else {
                        status = "400 Bad Request";
                        serde_json::json!({})
                    };
                    let body = result.to_string();
                    let header = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(body.as_bytes());
                    continue;
                }
                if verb == "POST"
                    && (path == "/vault/api/ciphers" || path == "/vault/api/ciphers/create")
                {
                    let mode = server_write_mode.load(Ordering::SeqCst);
                    if mode == 3 {
                        continue;
                    }
                    let value: serde_json::Value =
                        serde_json::from_str(req.split_once("\r\n\r\n").unwrap().1).unwrap();
                    let mut body = if path.ends_with("/create") {
                        value["cipher"].clone()
                    } else {
                        value.clone()
                    };
                    if mode == 0 || mode == 4 {
                        let _=stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
                        continue;
                    }
                    body["id"] = serde_json::json!(Uuid::new_v4());
                    body["creationDate"] = serde_json::json!(chrono::Utc::now().to_rfc3339());
                    body["revisionDate"] = body["creationDate"].clone();
                    body["edit"] = serde_json::json!(true);
                    body["viewPassword"] = serde_json::json!(true);
                    body["organizationUseTotp"] = serde_json::json!(false);
                    body["collectionIds"] = value
                        .get("collectionIds")
                        .cloned()
                        .unwrap_or(serde_json::json!([]));
                    sync_payload.lock().unwrap().1["ciphers"]
                        .as_array_mut()
                        .unwrap()
                        .push(body.clone());
                    if mode == 2 {
                        continue;
                    }
                    if mode == 7 {
                        std::thread::sleep(std::time::Duration::from_millis(400));
                    }
                    let body = body.to_string();
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(body.as_bytes());
                    continue;
                }
                if path.starts_with("/vault/api/folders")
                    && matches!(verb, "POST" | "PUT" | "DELETE")
                {
                    // 1 accept; 2 apply/lose reply; 3 drop before applying;
                    // 4 reject; 5 redirect; 6 slow reply; 7 malformed reply; 8 expire auth.
                    let mode = server_folder_mode.load(Ordering::SeqCst);
                    if mode == 3 {
                        continue;
                    }
                    let mut status = "200 OK";
                    let mut body = serde_json::json!({});
                    let mut extra = "";
                    if mode == 4 {
                        status = "400 Bad Request";
                    } else if mode == 5 {
                        status = "302 Found";
                        extra = "Location: https://untrusted.invalid/folders\r\n";
                    } else if mode == 8 {
                        status = "401 Unauthorized";
                    } else {
                        let mut payload = sync_payload.lock().unwrap();
                        let id = path.strip_prefix("/vault/api/folders/");
                        if verb == "DELETE" {
                            let folders = payload.1["folders"].as_array_mut().unwrap();
                            if folders.iter().any(|f| f["id"].as_str() == id) {
                                folders.retain(|f| f["id"].as_str() != id);
                                for cipher in payload.1["ciphers"].as_array_mut().unwrap() {
                                    if cipher["folderId"].as_str() == id {
                                        cipher["folderId"] = serde_json::Value::Null;
                                    }
                                }
                            } else {
                                status = "400 Bad Request";
                            }
                        } else {
                            let request: serde_json::Value =
                                serde_json::from_str(req.split_once("\r\n\r\n").unwrap().1)
                                    .unwrap();
                            let folders = payload.1["folders"].as_array_mut().unwrap();
                            if verb == "POST" {
                                body = serde_json::json!({"id":Uuid::new_v4(),"name":request["name"],"revisionDate":chrono::Utc::now().to_rfc3339(),"object":"folder"});
                                folders.push(body.clone());
                            } else if let Some(folder) =
                                folders.iter_mut().find(|f| f["id"].as_str() == id)
                            {
                                folder["name"] = request["name"].clone();
                                folder["revisionDate"] = serde_json::json!(
                                    (chrono::Utc::now() + chrono::Duration::seconds(1))
                                        .to_rfc3339()
                                );
                                body = folder.clone();
                            } else {
                                status = "400 Bad Request";
                            }
                        }
                        if mode == 2 {
                            continue;
                        }
                        if mode == 6 {
                            std::thread::sleep(std::time::Duration::from_millis(400));
                        }
                        if mode == 7 {
                            body = serde_json::json!({});
                        }
                    }
                    let body = if verb == "DELETE" && status == "200 OK" {
                        String::new()
                    } else {
                        body.to_string()
                    };
                    let header = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(body.as_bytes());
                    continue;
                }
                if first.starts_with("PUT /vault/api/ciphers/") {
                    let mode = server_write_mode.load(Ordering::SeqCst);
                    let body: serde_json::Value =
                        serde_json::from_str(req.split_once("\r\n\r\n").unwrap().1).unwrap();
                    let id = first
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .rsplit('/')
                        .next()
                        .unwrap();
                    let mut sync = sync_payload.lock().unwrap();
                    let server_user_id = sync.1["profile"]["id"].clone();
                    let models = sync.1["ciphers"].as_array_mut().unwrap();
                    let original = models
                        .iter_mut()
                        .find(|m| m["id"].as_str() == Some(id))
                        .unwrap();
                    if mode == 4 {
                        original["revisionDate"] = serde_json::json!(
                            (chrono::Utc::now() + chrono::Duration::seconds(10)).to_rfc3339()
                        );
                    }
                    let revision = chrono::DateTime::parse_from_rfc3339(
                        original["revisionDate"].as_str().unwrap(),
                    )
                    .unwrap();
                    let known = chrono::DateTime::parse_from_rfc3339(
                        body["lastKnownRevisionDate"].as_str().unwrap(),
                    )
                    .unwrap();
                    let accept = matches!(mode, 1 | 2 | 7 | 8)
                        && revision == known
                        && body["encryptedFor"] == server_user_id;
                    if mode == 3 {
                        continue;
                    }
                    if accept {
                        for key in [
                            "name",
                            "notes",
                            "fields",
                            "login",
                            "card",
                            "identity",
                            "secureNote",
                            "sshKey",
                            "bankAccount",
                            "driversLicense",
                            "passport",
                            "favorite",
                            "reprompt",
                            "folderId",
                            "key",
                            "passwordHistory",
                            "type",
                            "organizationId",
                            "archivedDate",
                        ] {
                            original[key] = body[key].clone();
                        }
                        original["revisionDate"] = serde_json::json!(
                            (chrono::Utc::now().max(
                                revision.with_timezone(&chrono::Utc) + chrono::Duration::seconds(2)
                            ))
                            .to_rfc3339()
                        );
                        if mode == 2 {
                            continue;
                        }
                        if mode == 7 {
                            std::thread::sleep(std::time::Duration::from_millis(400));
                        }
                        if mode == 8 {
                            server_write_mode.store(0, Ordering::SeqCst);
                        }
                    }
                    let data = if accept {
                        original.to_string()
                    } else {
                        serde_json::json!({"message":"The client copy of this cipher is out of date. Resync the client and try again."}).to_string()
                    };
                    let status = if accept { "200 OK" } else { "400 Bad Request" };
                    let header = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        data.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(data.as_bytes());
                    continue;
                }
                if first.starts_with("GET /vault/icons/") {
                    if first.contains("redirect.test/") {
                        let _ = stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: https://untrusted.invalid/icon.png\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    } else if first.contains("oversized.test/") {
                        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2097153\r\nConnection: close\r\n\r\n");
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(
                            server_delay.load(Ordering::SeqCst),
                        ));
                        let mut png = std::io::Cursor::new(Vec::new());
                        image::DynamicImage::new_rgba8(4, 4)
                            .write_to(&mut png, image::ImageFormat::Png)
                            .unwrap();
                        let data = if first.contains("invalid.test/") {
                            b"not an image".to_vec()
                        } else {
                            png.into_inner()
                        };
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            data.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(&data);
                    }
                    continue;
                }
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
                        let policy = server_login_policy.lock().unwrap().clone();
                        let response = if let Some(policy) = policy {
                            let mut data: serde_json::Value =
                                serde_json::from_str(&response).unwrap();
                            data["MasterPasswordPolicy"] = policy;
                            data.to_string()
                        } else {
                            response.clone()
                        };
                        ("200 OK", response, "")
                    } else {
                        (
                            "400 Bad Request",
                            "{\"TwoFactorProviders2\":{\"0\":null,\"1\":{}}}".into(),
                            "",
                        )
                    }
                } else if first.contains("/accounts/revision-date ") {
                    use std::hash::{Hash, Hasher};
                    let mut hash = std::collections::hash_map::DefaultHasher::new();
                    sync_payload.lock().unwrap().1.to_string().hash(&mut hash);
                    (
                        "200 OK",
                        server_revision_override
                            .lock()
                            .unwrap()
                            .unwrap_or((hash.finish() & i64::MAX as u64) as i64)
                            .to_string(),
                        "",
                    )
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
            login_policy,
            write_mode,
            folder_mode,
            attachment_mode,
            attachment_files,
            revision_override,
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
        let mut manager =
            crate::services::sdk::test_support::manager_with_profiles(self.store.clone());
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
async fn local_edits_are_encrypted_durable_revision_checked_and_not_uploaded_on_save() {
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
    assert_eq!(
        f.requests.lock().unwrap().len(),
        requests,
        "Saving a local draft must send no HTTP requests"
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

#[tokio::test]
async fn browser_authority_uses_the_server_identity_and_revokes_on_lock() {
    let f = Fixture::new();
    let (mut manager, uid) = f.account().await;
    let browser = manager.browser_session_for(&uid).unwrap();
    assert_ne!(
        browser.uid, browser.server_user_id,
        "Local account IDs must not be used in browser messages"
    );
    assert!(browser.current());
    assert!(browser.unlocked());
    assert!(browser.approved_browser_key().await.is_ok());
    manager.lock(&uid).unwrap().finish().await;
    assert!(!browser.current());
    assert!(!browser.unlocked());
    assert!(browser.approved_browser_key().await.is_err());
}

#[tokio::test]
async fn favicon_uses_account_mtls_subpath_without_oauth_and_caches_once() {
    use crate::services::favicon::{FaviconService, IconState};
    let fixture = Fixture::new();
    let (mut manager, uid) = fixture.account().await;
    let source = manager.favicon_source_for(&uid).unwrap();
    let service = FaviconService::new();
    assert!(matches!(
        service.get(&uid, "example.test"),
        IconState::Missing
    ));
    service.set_source(uid, Some(source.clone()));
    let version = service.version();
    service.set_source(uid, manager.favicon_source_for(&uid));
    assert_eq!(
        version,
        service.version(),
        "equivalent sources retain their cache"
    );
    let mut updates = service.event_sender().subscribe();
    assert!(matches!(
        service.get(&uid, "example.test"),
        IconState::Pending
    ));
    tokio::time::timeout(std::time::Duration::from_secs(3), updates.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        service.get(&uid, "example.test"),
        IconState::Found(_)
    ));
    assert!(matches!(
        service.get(&uid, "example.test"),
        IconState::Found(_)
    ));
    {
        let requests = fixture.requests.lock().unwrap();
        let icons: Vec<_> = requests
            .iter()
            .filter(|(req, _)| req.starts_with("GET /vault/icons/"))
            .collect();
        assert_eq!(icons.len(), 1);
        assert!(
            icons[0]
                .0
                .starts_with("GET /vault/icons/example.test/icon.png HTTP/1.1\r\n")
        );
        assert!(!icons[0].0.to_ascii_lowercase().contains("authorization:"));
        assert!(!icons[0].0.to_ascii_lowercase().contains("cookie:"));
        assert_eq!(
            icons[0].1,
            fixture
                .store
                .get(fixture.profile.id)
                .unwrap()
                .certificate
                .unwrap()
                .fingerprint
        );
    }
    manager.lock(&uid).unwrap().finish().await;
    assert!(!source.current());
    assert!(source.fetch("example.test").await.is_err());
    assert!(matches!(
        service.get(&uid, "example.test"),
        IconState::Missing
    ));
}

#[tokio::test]
async fn favicon_rejects_redirects_oversized_invalid_and_revoked_sources() {
    use crate::services::favicon::{FaviconService, IconState};
    let fixture = Fixture::new();
    let (manager, uid) = fixture.account().await;
    let source = manager.favicon_source_for(&uid).unwrap();
    assert!(source.fetch("redirect.test").await.is_err());
    assert!(source.fetch("oversized.test").await.is_err());
    let before = fixture.requests.lock().unwrap().len();
    for host in [
        "127.0.0.1",
        "localhost",
        "hidden.onion",
        "example.test/../../api/sync",
        "example.test?x=y",
    ] {
        assert!(source.fetch(host).await.is_err());
    }
    assert_eq!(before, fixture.requests.lock().unwrap().len());
    let service = FaviconService::new();
    service.set_source(uid, Some(source.clone()));
    let mut updates = service.event_sender().subscribe();
    service.get(&uid, "invalid.test");
    tokio::time::timeout(std::time::Duration::from_secs(3), updates.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        service.get(&uid, "invalid.test"),
        IconState::Missing
    ));
    let mut profile = fixture
        .store
        .get(manager.profile_for(&uid).unwrap())
        .unwrap();
    profile.server_ca_pem = None;
    fixture
        .store
        .save(profile.clone(), Some(profile.revision))
        .unwrap();
    let before = fixture.requests.lock().unwrap().len();
    assert!(!source.current());
    assert!(source.fetch("example.test").await.is_err());
    assert_eq!(before, fixture.requests.lock().unwrap().len());
    assert!(matches!(
        service.get(&uid, "invalid.test"),
        IconState::Missing
    ));
}

#[tokio::test]
async fn favicon_late_response_cannot_repopulate_a_recreated_cache() {
    use crate::services::favicon::{FaviconService, IconState};
    let fixture = Fixture::new();
    let (manager, uid) = fixture.account().await;
    let source = manager.favicon_source_for(&uid).unwrap();
    let service = FaviconService::new();
    service.set_source(uid, Some(source.clone()));
    let mut updates = service.event_sender().subscribe();
    fixture.sync_delay.store(200, Ordering::SeqCst);
    service.get(&uid, "example.test");
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|(r, _)| r.starts_with("GET /vault/icons/"))
        {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    service.evict_user(&uid);
    service.set_source(uid, Some(source));
    // A late request from before disable/re-enable must not populate the new
    // cache even when the account session and hostname are still identical.
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(350), updates.recv())
            .await
            .is_err()
    );
    fixture.sync_delay.store(0, Ordering::SeqCst);
    assert!(matches!(
        service.get(&uid, "example.test"),
        IconState::Pending
    ));
    tokio::time::timeout(std::time::Duration::from_secs(3), updates.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        service.get(&uid, "example.test"),
        IconState::Found(_)
    ));
}

fn org_policy(payload: &mut serde_json::Value, org: Uuid, kind: i64, data: serde_json::Value) {
    let orgs = payload["profile"]["organizations"].as_array_mut().unwrap();
    if !orgs
        .iter()
        .any(|o| o["id"].as_str() == Some(&org.to_string()))
    {
        orgs.push(serde_json::json!({"id":org,"name":"Synthetic policy organization","enabled":true,"usePolicies":true,"type":2,"status":2}));
    }
    payload["policies"].as_array_mut().unwrap().push(serde_json::json!({"id":Uuid::new_v4(),"organizationId":org,"type":kind,"enabled":true,"data":data}));
}

#[tokio::test]
async fn organization_generator_and_uri_rules_survive_encrypted_offline_restart() {
    use crate::services::sdk::{ClientExt, test_support::encrypted_sync_response, vault_sync};
    use bitwarden_generators::{PassphraseGeneratorRequest, PasswordGeneratorRequest};
    let f = Fixture::new();
    let (manager, uid) = f.account().await;
    let stale = manager.client_for(&uid).unwrap();
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .generate_password(PasswordGeneratorRequest::default())
            .await
            .is_err(),
        "Before first server sync the policy is unknown"
    );
    let mut payload = encrypted_sync_response(&manager, uid, "Policy-bound login").await;
    let id: bitwarden_vault::CipherId = payload["ciphers"][0]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let org = Uuid::new_v4();
    org_policy(
        &mut payload,
        org,
        2,
        serde_json::json!({"minLength":24,"useUppercase":true,"useLowercase":true,"numberCount":4,"specialCount":3,"minNumberWords":6,"capitalize":true,"includeNumber":true}),
    );
    org_policy(
        &mut payload,
        org,
        16,
        serde_json::json!({"uriMatchDetection":3}),
    );
    *f.sync_response.lock().unwrap() = (200, payload);
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(
        stale
            .generate_password(PasswordGeneratorRequest::default())
            .await
            .is_err()
    );
    let weak = PasswordGeneratorRequest {
        length: 5,
        lowercase: false,
        uppercase: false,
        numbers: false,
        special: false,
        ..Default::default()
    };
    let generated = manager
        .client_for(&uid)
        .unwrap()
        .generate_password(weak)
        .await
        .unwrap();
    assert_eq!(generated.len(), 24);
    assert!(generated.chars().any(|c| c.is_ascii_uppercase()));
    assert!(generated.chars().any(|c| c.is_ascii_lowercase()));
    assert!(generated.chars().filter(|c| c.is_ascii_digit()).count() >= 4);
    assert!(
        generated
            .chars()
            .filter(|c| "!@#$%^&*".contains(*c))
            .count()
            >= 3
    );
    let passphrase = manager
        .client_for(&uid)
        .unwrap()
        .generate_passphrase(PassphraseGeneratorRequest::default())
        .await
        .unwrap();
    assert_eq!(passphrase.split(' ').count(), 6);
    assert!(passphrase.chars().any(|c| c.is_ascii_digit()));
    let mut view = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    view.login.as_mut().unwrap().uris = Some(
        serde_json::from_value(serde_json::json!([
            {"uri":"https://new.example.test"},{"uri":"https://explicit.example.test","match":5}
        ]))
        .unwrap(),
    );
    let saved = manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(view)
        .await
        .unwrap();
    let uris = saved.login.unwrap().uris.unwrap();
    assert_eq!(
        uris[0].r#match, None,
        "Default matching remains dynamic instead of freezing today's organization setting"
    );
    assert_eq!(
        manager.policy_rules_for(&uid).uri_default,
        Some(bitwarden_vault::UriMatchType::Exact)
    );
    assert_eq!(uris[1].r#match, Some(bitwarden_vault::UriMatchType::Never));
    drop(manager);
    let restored = crate::services::sdk::production::test_load(&f.root, &f.store).await;
    restored
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    assert_eq!(restored.policy_rules_for(&uid).generator.min_length, 24);
    assert_eq!(
        restored
            .client_for(&uid)
            .unwrap()
            .generate_password(PasswordGeneratorRequest::default())
            .await
            .unwrap()
            .len(),
        24
    );
    assert_eq!(
        restored
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .login
            .unwrap()
            .uris
            .unwrap()[0]
            .r#match,
        None
    );
    assert_eq!(
        restored.policy_rules_for(&uid).uri_default,
        Some(bitwarden_vault::UriMatchType::Exact)
    );
}

#[tokio::test]
async fn personal_ownership_blocks_local_edit_and_save_without_blocking_read_or_copy() {
    use crate::services::sdk::{
        Action, ClientExt, test_support::encrypted_sync_response, vault_sync,
    };
    let f = Fixture::new();
    let (manager, uid) = f.account().await;
    let mut payload = encrypted_sync_response(&manager, uid, "Personal item").await;
    let id = payload["ciphers"][0]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    org_policy(&mut payload, Uuid::new_v4(), 5, serde_json::json!({}));
    *f.sync_response.lock().unwrap() = (200, payload);
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
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
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher_for(id, Action::Edit)
            .await
            .is_err()
    );
    let mut view = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher_for(id, Action::Copy)
        .await
        .unwrap();
    view.name = "Blocked local edit".into();
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .save_cipher(view)
            .await
            .is_err()
    );
    assert_eq!(manager.local_edit_count(&uid), 0);
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .name,
        "Personal item"
    );
}

#[tokio::test]
async fn restricted_types_block_list_read_copy_and_ssh_agent_and_policy_removal_restores_them() {
    use crate::services::sdk::{
        Action, ClientExt, test_support::encrypted_ssh_sync_response, vault_sync,
    };
    let f = Fixture::new();
    let (manager, uid) = f.account().await;
    let (mut payload, _) = encrypted_ssh_sync_response(&manager, uid).await;
    let id = payload["ciphers"][1]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    org_policy(&mut payload, Uuid::new_v4(), 15, serde_json::json!([5]));
    *f.sync_response.lock().unwrap() = (200, payload.clone());
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
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
    for action in [Action::Read, Action::Copy, Action::Edit] {
        assert!(
            manager
                .client_for(&uid)
                .unwrap()
                .full_cipher_for(id, action)
                .await
                .is_err()
        );
    }
    assert!(
        manager
            .agent_session_for(&uid)
            .unwrap()
            .identities()
            .await
            .unwrap()
            .is_empty()
    );
    payload["policies"][0]["enabled"] = false.into();
    *f.sync_response.lock().unwrap() = (200, payload);
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .list_ciphers()
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        manager
            .agent_session_for(&uid)
            .unwrap()
            .identities()
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn organization_master_password_requirement_prevents_publishing_a_weak_login() {
    let f = Fixture::new();
    *f.login_policy.lock().unwrap() =
        Some(serde_json::json!({"EnforceOnLogin":true,"MinLength":64,"MinComplexity":null}));
    let transport = Transport::new(&f.store, f.import()).unwrap();
    let pending = PendingLogin::prepare(
        Attempt::new(),
        transport,
        "test@example.invalid".into(),
        Zeroizing::new("synthetic-password".into()),
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    let result = pending
        .submit(Some((
            Factor::Authenticator,
            Zeroizing::new("123456".into()),
        )))
        .await;
    assert!(result.unwrap_err().contains("stronger master password"));
    assert!(
        !f.root.join("accounts").exists(),
        "No local account or token is published after policy failure"
    );
}

async fn staged_write_fixture() -> (
    Fixture,
    crate::services::sdk::ClientManager,
    crate::domain::UserId,
    bitwarden_vault::CipherId,
) {
    use crate::services::sdk::{ClientExt, test_support::encrypted_sync_response, vault_sync};
    let f = Fixture::new();
    let (manager, uid) = f.account().await;
    *f.sync_response.lock().unwrap() = (
        200,
        encrypted_sync_response(&manager, uid, "Original write fixture").await,
    );
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
        .list_ciphers()
        .await
        .unwrap();
    let id = draft.remove(0).id.unwrap();
    let mut view = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    view.name = "Synthetic local upload title".into();
    view.login.as_mut().unwrap().password = Some("Synthetic-upload-password-only".into());
    manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(view)
        .await
        .unwrap();
    f.write_mode.store(1, Ordering::SeqCst);
    (f, manager, uid, id)
}
fn put_count(f: &Fixture) -> usize {
    f.requests
        .lock()
        .unwrap()
        .iter()
        .filter(|(r, _)| r.starts_with("PUT "))
        .count()
}
#[tokio::test]
async fn writable_sync_uploads_sdk_ciphertext_with_original_revision_and_persists_server_ack() {
    use crate::services::sdk::{ClientExt, vault_sync};
    let (f, mut manager, uid, id) = staged_write_fixture().await;
    let base = f.sync_response.lock().unwrap().1["ciphers"][0]["revisionDate"]
        .as_str()
        .unwrap()
        .to_owned();
    let original = chrono::DateTime::parse_from_rfc3339(&base).unwrap();
    assert_eq!(put_count(&f), 0);
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.uploaded, 1, "{:?}", report.problems);
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    assert_eq!(report.conflicts, 0);
    assert_eq!(manager.local_edit_count(&uid), 0);
    assert_eq!(put_count(&f), 1);
    let requests = f.requests.lock().unwrap().clone();
    let (request, peer) = requests
        .iter()
        .find(|(r, _)| r.starts_with("PUT "))
        .unwrap();
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(
        chrono::DateTime::parse_from_rfc3339(body["lastKnownRevisionDate"].as_str().unwrap())
            .unwrap(),
        original
    );
    assert_eq!(
        body["encryptedFor"],
        f.sync_response.lock().unwrap().1["profile"]["id"]
    );
    assert!(!peer.is_empty());
    assert!(
        request
            .to_lowercase()
            .contains("authorization: bearer synthetic-access-token")
    );
    assert!(!request.contains("Synthetic local upload title"));
    assert!(!request.contains("Synthetic-upload-password-only"));
    drop(requests);
    let current = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    assert_eq!(current.name, "Synthetic local upload title");
    assert!(
        current
            .password_history
            .as_ref()
            .unwrap()
            .iter()
            .any(|h| h.password == "synthetic sync vault secret")
    );
    assert!(current.revision_date > original);
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        put_count(&f),
        1,
        "Repeated sync must not replay acknowledged writes"
    );
    manager.lock(&uid).unwrap().finish().await;
    drop(manager);
    let restored = crate::services::sdk::production::test_load(&f.root, &f.store).await;
    restored
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    assert_eq!(restored.local_edit_count(&uid), 0);
    let current = restored
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    assert_eq!(current.name, "Synthetic local upload title");
    assert_eq!(
        current.login.unwrap().password.as_deref(),
        Some("Synthetic-upload-password-only")
    );
}
#[tokio::test]
async fn writable_sync_conflict_fetches_server_retains_draft_and_requires_explicit_reapply() {
    use crate::services::sdk::{ClientExt, vault_sync};
    let (f, manager, uid, id) = staged_write_fixture().await;
    let changed = chrono::Utc::now() + chrono::Duration::seconds(10);
    f.sync_response.lock().unwrap().1["ciphers"][0]["revisionDate"] =
        serde_json::json!(changed.to_rfc3339());
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.conflicts, 1);
    assert_eq!(put_count(&f), 0);
    assert_eq!(manager.local_edit_count(&uid), 1);
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .name,
        "Original write fixture"
    );
    assert_eq!(manager.local_conflicts(&uid), vec![id]);
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        put_count(&f),
        0,
        "A later sync must not silently rebase the draft"
    );
    let review = manager
        .client_for(&uid)
        .unwrap()
        .review_local(id)
        .await
        .unwrap();
    assert_eq!(review.name, "Synthetic local upload title");
    assert_eq!(review.revision_date, changed);
    manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(review)
        .await
        .unwrap();
    assert!(manager.local_conflicts(&uid).is_empty());
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.uploaded, 1, "{:?}", report.problems);
    assert_eq!(manager.local_edit_count(&uid), 0);
}
#[tokio::test]
async fn writable_sync_rejection_race_refreshes_without_replaying_and_survives_restart() {
    use crate::services::sdk::{ClientExt, vault_sync};
    let (f, mut manager, uid, id) = staged_write_fixture().await;
    f.write_mode.store(4, Ordering::SeqCst);
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(put_count(&f), 1);
    assert_eq!(report.uploaded, 0);
    assert_eq!(report.conflicts, 1);
    assert!(!report.problems.is_empty());
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .name,
        "Original write fixture"
    );
    manager.lock(&uid).unwrap().finish().await;
    crate::services::sdk::test_support::retain_sync_gui_fixture(&f.root);
    drop(manager);
    let restored = crate::services::sdk::production::test_load(&f.root, &f.store).await;
    restored
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    assert_eq!(restored.local_conflicts(&uid), vec![id]);
    let review = restored
        .client_for(&uid)
        .unwrap()
        .review_local(id)
        .await
        .unwrap();
    assert_eq!(review.name, "Synthetic local upload title");
    f.write_mode.store(1, Ordering::SeqCst);
    vault_sync::synchronize(
        restored.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(put_count(&f), 1);
    restored
        .client_for(&uid)
        .unwrap()
        .discard_local(id)
        .await
        .unwrap();
    assert_eq!(restored.local_edit_count(&uid), 0);
}
#[tokio::test]
async fn writable_sync_lost_reply_reconciles_applied_write_without_duplicate_upload() {
    use crate::services::sdk::{ClientExt, vault_sync};
    let (f, manager, uid, id) = staged_write_fixture().await;
    f.write_mode.store(2, Ordering::SeqCst);
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        report.uploaded, 1,
        "Read-after-write acknowledgment should be reported as confirmed"
    );
    assert!(report.problems.is_empty());
    assert_eq!(put_count(&f), 1);
    assert_eq!(report.conflicts, 0);
    assert_eq!(
        manager.local_edit_count(&uid),
        0,
        "A read-after-timeout can acknowledge exact ciphertext"
    );
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .name,
        "Synthetic local upload title"
    );
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(put_count(&f), 1);
}
#[tokio::test]
async fn writable_sync_unknown_unapplied_write_is_not_retried_automatically() {
    use crate::services::sdk::{ClientExt, vault_sync};
    let (f, manager, uid, id) = staged_write_fixture().await;
    f.write_mode.store(3, Ordering::SeqCst);
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.conflicts, 1);
    assert_eq!(put_count(&f), 1);
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .name,
        "Original write fixture"
    );
    f.write_mode.store(1, Ordering::SeqCst);
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(put_count(&f), 1);
    assert_eq!(manager.local_edit_count(&uid), 1);
}
#[tokio::test]
async fn writable_sync_respects_fresh_permissions_and_preserves_deleted_entry_drafts() {
    use crate::services::sdk::{ClientExt, vault_sync};
    let (f, manager, uid, id) = staged_write_fixture().await;
    f.sync_response.lock().unwrap().1["ciphers"][0]["edit"] = serde_json::json!(false);
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.conflicts, 1);
    assert_eq!(put_count(&f), 0);
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .review_local(id)
            .await
            .is_err()
    );
    f.sync_response.lock().unwrap().1["ciphers"] = serde_json::json!([]);
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.conflicts, 1);
    assert_eq!(put_count(&f), 0);
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .list_ciphers()
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .review_local(id)
            .await
            .is_err()
    );
    assert_eq!(manager.local_conflicts(&uid), vec![id]);
    manager
        .client_for(&uid)
        .unwrap()
        .discard_local(id)
        .await
        .unwrap();
    assert_eq!(manager.local_edit_count(&uid), 0);
}

#[tokio::test]
async fn writable_sync_checks_new_organization_policy_before_dispatch() {
    use crate::services::sdk::vault_sync;
    let (f, manager, uid, id) = staged_write_fixture().await;
    org_policy(
        &mut f.sync_response.lock().unwrap().1,
        Uuid::new_v4(),
        5,
        serde_json::json!({}),
    );
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.conflicts, 1);
    assert_eq!(put_count(&f), 0);
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .review_local(id)
            .await
            .is_err()
    );
}
async fn stage_second_write(
    f: &Fixture,
    manager: &crate::services::sdk::ClientManager,
    uid: crate::domain::UserId,
) {
    use crate::services::sdk::{ClientExt, vault_sync};
    // Add a second server item without reusing or replacing the first local draft.
    let mut payload = f.sync_response.lock().unwrap().1.clone();
    let mut second = payload["ciphers"][0].clone();
    let id = bitwarden_vault::CipherId::new_v4();
    second["id"] = serde_json::json!(id);
    payload["ciphers"].as_array_mut().unwrap().push(second);
    // Prevent the first draft from uploading during this preparatory download.
    // Its equal-revision edit permission is restored afterwards for the actual test.
    payload["ciphers"][0]["edit"] = serde_json::json!(false);
    *f.sync_response.lock().unwrap() = (200, payload);
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    f.sync_response.lock().unwrap().1["ciphers"][0]["edit"] = serde_json::json!(true);
    // Explicitly re-stage the first draft against the restored server permissions.
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    let first = manager.local_conflicts(&uid)[0];
    let review = manager
        .client_for(&uid)
        .unwrap()
        .review_local(first)
        .await
        .unwrap();
    manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(review)
        .await
        .unwrap();
    let mut view = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    view.name = "Second synthetic local upload".into();
    manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(view)
        .await
        .unwrap();
}
#[tokio::test]
async fn writable_sync_keeps_partial_batch_acknowledgments_and_never_replays_rejected_item() {
    use crate::services::sdk::vault_sync;
    let (f, manager, uid, _) = staged_write_fixture().await;
    stage_second_write(&f, &manager, uid).await;
    f.write_mode.store(8, Ordering::SeqCst);
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.uploaded, 1, "{:?}", report.problems);
    assert_eq!(report.conflicts, 1);
    assert_eq!(put_count(&f), 2);
    assert_eq!(manager.local_edit_count(&uid), 1);
    f.write_mode.store(1, Ordering::SeqCst);
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(put_count(&f), 2);
}
#[tokio::test]
async fn writable_sync_lock_during_dispatch_prevents_next_write_and_recovers_durable_intent() {
    use crate::services::sdk::vault_sync;
    let (f, mut manager, uid, _) = staged_write_fixture().await;
    stage_second_write(&f, &manager, uid).await;
    f.write_mode.store(7, Ordering::SeqCst);
    let task = tokio::spawn(vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    ));
    for _ in 0..200 {
        if put_count(&f) > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(put_count(&f), 1);
    manager.lock(&uid).unwrap().finish().await;
    assert!(task.await.unwrap().is_err());
    assert_eq!(
        put_count(&f),
        1,
        "Revocation must stop later writes in the batch"
    );
    drop(manager);
    let restored = crate::services::sdk::production::test_load(&f.root, &f.store).await;
    restored
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    assert_eq!(
        restored.local_edit_count(&uid),
        2,
        "Intent must survive lock before acknowledgment"
    );
    f.write_mode.store(1, Ordering::SeqCst);
    let report = vault_sync::synchronize(
        restored.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.uploaded, 1, "{:?}", report.problems);
    assert_eq!(
        put_count(&f),
        2,
        "The already applied write is reconciled, only the unsent draft uploads"
    );
    assert_eq!(restored.local_edit_count(&uid), 0);
}
#[tokio::test]
async fn writable_sync_enables_strict_field_decryption_for_remote_edits() {
    use crate::services::sdk::{ClientExt, test_support::encrypted_sync_response, vault_sync};
    let f = Fixture::new();
    let (manager, uid) = f.account().await;
    let mut payload =
        encrypted_sync_response(&manager, uid, "Synthetic corrupt password field").await;
    let id = payload["ciphers"][0]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    // Structurally valid ciphertext, encrypted under the wrong key slot.
    payload["ciphers"][0]["login"]["password"] = payload["folders"][0]["name"].clone();
    *f.sync_response.lock().unwrap() = (200, payload);
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
            .full_cipher(id)
            .await
            .is_err()
    );
    assert_eq!(put_count(&f), 0);
}

#[tokio::test]
async fn writable_sync_startup_download_preserves_pending_draft_without_uploading() {
    use crate::services::sdk::{ClientExt, vault_sync};
    let (f, manager, uid, id) = staged_write_fixture().await;
    let report = vault_sync::download(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.uploaded, 0);
    assert_eq!(report.conflicts, 0);
    assert_eq!(manager.local_edit_count(&uid), 1);
    assert_eq!(put_count(&f), 0);
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .name,
        "Synthetic local upload title"
    );
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.uploaded, 1);
    assert_eq!(put_count(&f), 1);
}
#[tokio::test]
async fn writable_sync_conflict_review_requires_one_use_password_grant_for_protected_draft() {
    use crate::services::sdk::{Action, Authorization, ClientExt, vault_sync};
    let (f, manager, uid, id) = staged_write_fixture().await;
    let mut view = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    view.reprompt = bitwarden_vault::CipherRepromptType::Password;
    manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(view)
        .await
        .unwrap();
    f.sync_response.lock().unwrap().1["ciphers"][0]["revisionDate"] =
        serde_json::json!((chrono::Utc::now() + chrono::Duration::seconds(10)).to_rfc3339());
    vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(manager.local_draft_reprompt(&uid, id));
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .review_local(id)
            .await
            .is_err()
    );
    let current = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    let target = Authorization {
        cipher: Some(id),
        revision: Some(current.revision_date),
        action: Action::Edit,
    };
    let (client, envelope) = manager.validation_data_for(&uid).unwrap();
    client
        .authorize_password(envelope, "synthetic-password".into(), target)
        .await
        .unwrap();
    let draft = manager
        .client_for(&uid)
        .unwrap()
        .review_local(id)
        .await
        .unwrap();
    assert_eq!(draft.name, "Synthetic local upload title");
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .review_local(id)
            .await
            .is_err()
    );
    assert_eq!(put_count(&f), 0);
}

#[tokio::test]
async fn writable_sync_legacy_keyless_attachment_edit_cannot_orphan_file_encryption() {
    use crate::services::sdk::{
        ClientExt, test_support::encrypted_legacy_attachment_sync_response, vault_sync,
    };
    let f = Fixture::new();
    let (manager, uid) = f.account().await;
    *f.sync_response.lock().unwrap() = (
        200,
        encrypted_legacy_attachment_sync_response(&manager, uid).await,
    );
    vault_sync::download(
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
    let mut view = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    assert!(view.key.is_none());
    assert_eq!(
        view.attachments.as_ref().unwrap()[0].file_name.as_deref(),
        Some("legacy.txt")
    );
    view.name = "Attempted legacy edit".into();
    let error = manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(view)
        .await
        .unwrap_err();
    assert!(error.contains("legacy attachments"));
    assert_eq!(manager.local_edit_count(&uid), 0);
    assert_eq!(put_count(&f), 0);
}

async fn core_for_automatic_save(
    manager: crate::services::sdk::ClientManager,
) -> crate::core::Core {
    let mut core = crate::core::Core::test(manager);
    core.settings.ssh_agent = false;
    core.settings.browser_integration = false;
    core.settings.show_favicons = false;
    core.load_vault();
    for _ in 0..200 {
        core.poll();
        if core.current_cache().is_some() {
            return core;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("Synthetic vault did not load");
}
async fn save_automatically(core: &mut crate::core::Core, id: bitwarden_vault::CipherId) {
    use crate::services::sdk::ClientExt;
    let mut view = core
        .manager
        .client_for(&core.active.unwrap())
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    view.name = "Automatically saved fixture".into();
    core.editor = Some(crate::core::model::Editor::from_cipher(&view));
    core.request(crate::core::model::Request::Save(Box::new(view)));
}
async fn finish_automatic_sync(core: &mut crate::core::Core, uid: crate::domain::UserId) {
    for _ in 0..400 {
        core.poll();
        if core.sync.get(&uid).is_some_and(|state| !state.busy)
            && (core.active != Some(uid)
                || (core.current_cache().is_some() && (!core.detail_open || core.detail.is_some())))
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("Automatic save synchronization did not finish");
}
#[tokio::test]
async fn saving_in_the_app_automatically_uploads_and_preserves_search_and_selection() {
    use crate::services::sdk::ClientExt;
    let (f, manager, uid, id) = staged_write_fixture().await;
    let mut core = core_for_automatic_save(manager).await;
    core.query = "fixture".into();
    save_automatically(&mut core, id).await;
    finish_automatic_sync(&mut core, uid).await;
    assert_eq!(
        put_count(&f),
        1,
        "No manual sync action should be necessary"
    );
    assert_eq!(core.manager.local_edit_count(&uid), 0);
    assert_eq!(core.query, "fixture");
    assert_eq!(core.selected, Some(id));
    assert_eq!(
        core.detail.as_ref().unwrap().name,
        "Automatically saved fixture"
    );
    assert_eq!(
        core.manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .name,
        "Automatically saved fixture"
    );
}
#[tokio::test]
async fn automatic_upload_failure_keeps_the_draft_without_a_retry_loop() {
    let (f, manager, uid, id) = staged_write_fixture().await;
    f.write_mode.store(3, Ordering::SeqCst);
    let mut core = core_for_automatic_save(manager).await;
    save_automatically(&mut core, id).await;
    finish_automatic_sync(&mut core, uid).await;
    assert_eq!(put_count(&f), 1);
    assert_eq!(core.manager.local_edit_count(&uid), 1);
    assert_eq!(core.manager.local_conflicts(&uid), vec![id]);
    f.write_mode.store(1, Ordering::SeqCst);
    for _ in 0..20 {
        core.poll();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(
        put_count(&f),
        1,
        "A failed automatic upload must not continuously retry"
    );
}
#[tokio::test]
async fn automatic_save_sync_retains_conflicts_without_overwriting_the_server() {
    use crate::services::sdk::ClientExt;
    let (f, manager, uid, id) = staged_write_fixture().await;
    let mut core = core_for_automatic_save(manager).await;
    f.sync_response.lock().unwrap().1["ciphers"][0]["revisionDate"] =
        serde_json::json!((chrono::Utc::now() + chrono::Duration::seconds(10)).to_rfc3339());
    save_automatically(&mut core, id).await;
    finish_automatic_sync(&mut core, uid).await;
    assert_eq!(put_count(&f), 0);
    assert_eq!(core.manager.local_conflicts(&uid), vec![id]);
    assert_eq!(
        core.manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .name,
        "Original write fixture"
    );
    assert_eq!(
        core.manager
            .client_for(&uid)
            .unwrap()
            .review_local(id)
            .await
            .unwrap()
            .name,
        "Automatically saved fixture"
    );
}
#[tokio::test]
async fn automatic_save_upload_stays_with_its_account_after_switching_away() {
    let (f, manager, uid, id) = staged_write_fixture().await;
    let mut core = core_for_automatic_save(manager).await;
    save_automatically(&mut core, id).await;
    core.clear_presentation();
    core.active = None;
    finish_automatic_sync(&mut core, uid).await;
    assert_eq!(put_count(&f), 1);
    assert_eq!(core.manager.local_edit_count(&uid), 0);
    assert!(core.active.is_none());
    assert!(core.detail.is_none());
    assert!(core.editor.is_none());
}

#[tokio::test]
async fn automatic_sync_does_not_reveal_a_protected_entry_after_its_revision_changes() {
    use crate::services::sdk::ClientExt;
    let (f, manager, uid, id) = staged_write_fixture().await;
    let mut core = core_for_automatic_save(manager).await;
    let mut view = core
        .manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    view.reprompt = bitwarden_vault::CipherRepromptType::Password;
    core.editor = Some(crate::core::model::Editor::from_cipher(&view));
    core.request(crate::core::model::Request::Save(Box::new(view)));
    finish_automatic_sync(&mut core, uid).await;
    assert_eq!(put_count(&f), 1);
    assert_eq!(core.manager.local_edit_count(&uid), 0);
    assert!(core.detail.is_none());
    assert!(!core.detail_open);
    assert!(core.prompt.is_none());
    assert!(
        core.manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .is_err()
    );
}

fn folder_writes(f: &Fixture) -> usize {
    f.requests
        .lock()
        .unwrap()
        .iter()
        .filter(|(r, _)| {
            r.lines().next().is_some_and(|line| {
                line.contains("/vault/api/folders") && !line.starts_with("GET ")
            })
        })
        .count()
}
#[tokio::test]
async fn folder_management_create_rename_delete_are_encrypted_and_keep_items() {
    use crate::services::sdk::{
        ClientExt,
        folder_management::{self, Change},
    };
    let (f, manager, uid, _) = staged_write_fixture().await;
    let created = folder_management::change(
        manager.client_for(&uid).unwrap(),
        Change::Create("Private folder / nested".into()),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(created.applied, "{:?}", created.report.problems);
    assert!(created.report.problems.is_empty());
    let folders = manager
        .client_for(&uid)
        .unwrap()
        .list_folders()
        .await
        .unwrap();
    let folder = folders
        .iter()
        .find(|f| f.name == "Private folder / nested")
        .unwrap();
    let id = folder.id.unwrap();
    let revision = folder.revision_date;
    let renamed = folder_management::change(
        manager.client_for(&uid).unwrap(),
        Change::Rename {
            id,
            revision,
            name: "Renamed private folder".into(),
        },
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(renamed.applied, "{:?}", renamed.report.problems);
    let folders = manager
        .client_for(&uid)
        .unwrap()
        .list_folders()
        .await
        .unwrap();
    let folder = folders.iter().find(|f| f.id == Some(id)).unwrap();
    assert_eq!(folder.name, "Renamed private folder");
    let deleted = folder_management::change(
        manager.client_for(&uid).unwrap(),
        Change::Delete {
            id,
            revision: folder.revision_date,
        },
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(deleted.applied, "{:?}", deleted.report.problems);
    assert!(!manager.has_folder_intent(&uid));
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
    assert!(
        !manager
            .client_for(&uid)
            .unwrap()
            .list_folders()
            .await
            .unwrap()
            .iter()
            .any(|f| f.id == Some(id))
    );
    assert_eq!(folder_writes(&f), 3);
    assert_eq!(
        put_count(&f),
        1,
        "Only the folder PUT was sent; item drafts were not uploaded by folder operations"
    );
    for (request, peer) in f.requests.lock().unwrap().iter() {
        assert!(!request.contains("Private folder / nested"));
        assert!(!request.contains("Renamed private folder"));
        assert!(!peer.is_empty());
    }
    for suffix in [".sqlite", ".sqlite-wal"] {
        if let Ok(bytes) = std::fs::read(f.root.join(format!("{uid}{suffix}"))) {
            for name in ["Private folder / nested", "Renamed private folder"] {
                assert!(!bytes.windows(name.len()).any(|w| w == name.as_bytes()));
            }
        }
    }
}
#[tokio::test]
async fn folder_management_deleting_assigned_folder_preserves_items_children_and_local_drafts() {
    use crate::services::sdk::{
        ClientExt,
        folder_management::{self, Change},
    };
    let (f, manager, uid, cipher_id) = staged_write_fixture().await;
    let folders = manager
        .client_for(&uid)
        .unwrap()
        .list_folders()
        .await
        .unwrap();
    let folder = &folders[0];
    let id = folder.id.unwrap();
    let revision = folder.revision_date;
    folder_management::change(
        manager.client_for(&uid).unwrap(),
        Change::Create(format!("{}/child", folder.name)),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    let outcome = folder_management::change(
        manager.client_for(&uid).unwrap(),
        Change::Delete { id, revision },
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(outcome.applied, "{:?}", outcome.report.problems);
    let items = manager
        .client_for(&uid)
        .unwrap()
        .list_ciphers()
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, Some(cipher_id));
    assert!(items[0].folder_id.is_none());
    assert_eq!(manager.local_conflicts(&uid), vec![cipher_id]);
    let draft = manager
        .client_for(&uid)
        .unwrap()
        .review_local(cipher_id)
        .await
        .unwrap();
    assert_eq!(draft.name, "Synthetic local upload title");
    assert!(draft.folder_id.is_none());
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .list_folders()
            .await
            .unwrap()
            .len(),
        1,
        "Nested folder is independent and must survive parent deletion"
    );
    assert_eq!(put_count(&f), 0);
}
#[tokio::test]
async fn folder_management_rename_preflight_rejects_stale_and_foreign_targets() {
    use crate::services::sdk::{
        ClientExt,
        folder_management::{self, Change},
    };
    let (f, manager, uid, _) = staged_write_fixture().await;
    let folders = manager
        .client_for(&uid)
        .unwrap()
        .list_folders()
        .await
        .unwrap();
    let folder = &folders[0];
    let count = f.requests.lock().unwrap().len();
    assert!(
        folder_management::change(
            manager.client_for(&uid).unwrap(),
            Change::Rename {
                id: bitwarden_vault::FolderId::new_v4(),
                revision: folder.revision_date,
                name: "Foreign".into()
            },
            f.root.clone(),
            f.store.clone()
        )
        .await
        .is_err()
    );
    assert_eq!(f.requests.lock().unwrap().len(), count);
    f.sync_response.lock().unwrap().1["folders"][0]["revisionDate"] =
        serde_json::json!((chrono::Utc::now() + chrono::Duration::seconds(10)).to_rfc3339());
    let outcome = folder_management::change(
        manager.client_for(&uid).unwrap(),
        Change::Rename {
            id: folder.id.unwrap(),
            revision: folder.revision_date,
            name: "Stale rename".into(),
        },
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(!outcome.applied);
    assert!(!outcome.report.problems.is_empty());
    assert_eq!(folder_writes(&f), 0);
    assert!(!manager.has_folder_intent(&uid));
}
#[tokio::test]
async fn folder_management_lost_create_reply_is_reconciled_without_duplicate_creation() {
    use crate::services::sdk::{
        ClientExt,
        folder_management::{self, Change},
        vault_sync,
    };
    let (f, manager, uid, _) = staged_write_fixture().await;
    f.folder_mode.store(2, Ordering::SeqCst);
    let outcome = folder_management::change(
        manager.client_for(&uid).unwrap(),
        Change::Create("Lost-reply folder".into()),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(outcome.applied, "{:?}", outcome.report.problems);
    assert!(outcome.report.problems.is_empty());
    assert!(!manager.has_folder_intent(&uid));
    assert_eq!(folder_writes(&f), 1);
    vault_sync::download(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(folder_writes(&f), 1);
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .list_folders()
            .await
            .unwrap()
            .iter()
            .filter(|f| f.name == "Lost-reply folder")
            .count(),
        1
    );
}
#[tokio::test]
async fn folder_management_unknown_create_survives_restart_and_is_never_replayed() {
    use crate::services::sdk::{
        folder_management::{self, Change},
        vault_sync,
    };
    let (f, mut manager, uid, _) = staged_write_fixture().await;
    f.folder_mode.store(3, Ordering::SeqCst);
    let outcome = folder_management::change(
        manager.client_for(&uid).unwrap(),
        Change::Create("Uncertain folder".into()),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(!outcome.applied);
    assert!(manager.has_folder_intent(&uid));
    assert_eq!(folder_writes(&f), 1);
    manager.lock(&uid).unwrap().finish().await;
    drop(manager);
    let restored = crate::services::sdk::production::test_load(&f.root, &f.store).await;
    restored
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    assert!(restored.has_folder_intent(&uid));
    f.folder_mode.store(1, Ordering::SeqCst);
    vault_sync::download(
        restored.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(restored.has_folder_intent(&uid));
    assert!(
        folder_management::change(
            restored.client_for(&uid).unwrap(),
            Change::Create("Must not duplicate".into()),
            f.root.clone(),
            f.store.clone()
        )
        .await
        .is_err()
    );
    assert_eq!(folder_writes(&f), 1);
    restored
        .client_for(&uid)
        .unwrap()
        .dismiss_folder_intent()
        .await
        .unwrap();
    assert!(!restored.has_folder_intent(&uid));
    assert_eq!(folder_writes(&f), 1);
}
#[tokio::test]
async fn folder_management_rejection_and_redirect_are_not_retried_or_followed() {
    use crate::services::sdk::folder_management::{self, Change};
    for mode in [4, 5, 8] {
        let (f, manager, uid, _) = staged_write_fixture().await;
        f.folder_mode.store(mode, Ordering::SeqCst);
        let outcome = folder_management::change(
            manager.client_for(&uid).unwrap(),
            Change::Create("Rejected folder".into()),
            f.root.clone(),
            f.store.clone(),
        )
        .await
        .unwrap();
        assert!(!outcome.applied);
        assert!(!outcome.report.problems.is_empty());
        assert_eq!(folder_writes(&f), 1);
        assert_eq!(manager.has_folder_intent(&uid), mode == 5);
        assert!(
            f.requests
                .lock()
                .unwrap()
                .iter()
                .all(|(r, _)| !r.contains("Host: untrusted.invalid"))
        );
    }
}
#[tokio::test]
async fn folder_management_lock_during_create_keeps_intent_and_recovers_without_replay() {
    use crate::services::sdk::{
        ClientExt,
        folder_management::{self, Change},
        vault_sync,
    };
    let (f, mut manager, uid, _) = staged_write_fixture().await;
    f.folder_mode.store(6, Ordering::SeqCst);
    let task = tokio::spawn(folder_management::change(
        manager.client_for(&uid).unwrap(),
        Change::Create("Locked folder fixture".into()),
        f.root.clone(),
        f.store.clone(),
    ));
    for _ in 0..200 {
        if folder_writes(&f) > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(folder_writes(&f), 1);
    manager.lock(&uid).unwrap().finish().await;
    assert!(task.await.unwrap().is_err());
    drop(manager);
    let restored = crate::services::sdk::production::test_load(&f.root, &f.store).await;
    restored
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    assert!(restored.has_folder_intent(&uid));
    vault_sync::download(
        restored.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(!restored.has_folder_intent(&uid));
    assert_eq!(folder_writes(&f), 1);
    assert!(
        restored
            .client_for(&uid)
            .unwrap()
            .list_folders()
            .await
            .unwrap()
            .iter()
            .any(|f| f.name == "Locked folder fixture")
    );
}

#[tokio::test]
async fn folder_management_app_actions_refresh_sidebar_and_clear_deleted_folder_filter() {
    use crate::services::vault_navigation::FolderFilter;
    let (f, manager, uid, cipher_id) = staged_write_fixture().await;
    let mut core = core_for_automatic_save(manager).await;
    core.open_folder(None, false);
    core.folder_editor.as_mut().unwrap().name = "UI folder fixture".into();
    core.submit_folder();
    finish_automatic_sync(&mut core, uid).await;
    assert!(core.folder_editor.is_none());
    let folder = core
        .current_cache()
        .unwrap()
        .folders
        .iter()
        .find(|f| f.name == "UI folder fixture")
        .unwrap();
    let id = folder.id;
    core.open_folder(Some(id), false);
    assert_eq!(
        core.folder_editor.as_ref().unwrap().name,
        "UI folder fixture"
    );
    core.folder_editor.as_mut().unwrap().name = "Renamed UI fixture".into();
    core.submit_folder();
    finish_automatic_sync(&mut core, uid).await;
    assert_eq!(
        core.current_cache()
            .unwrap()
            .folders
            .iter()
            .find(|f| f.id == id)
            .unwrap()
            .name,
        "Renamed UI fixture"
    );
    let mut filter = core.filter;
    filter.folder = FolderFilter::Folder(id);
    core.set_filter(filter);
    core.open_folder(Some(id), true);
    core.submit_folder();
    finish_automatic_sync(&mut core, uid).await;
    assert!(core.folder_editor.is_none());
    assert_eq!(core.filter.folder, FolderFilter::None);
    assert!(
        !core
            .current_cache()
            .unwrap()
            .folders
            .iter()
            .any(|f| f.id == id)
    );
    assert!(
        core.current_cache()
            .unwrap()
            .all
            .iter()
            .any(|i| i.id == Some(cipher_id))
    );
    assert_eq!(folder_writes(&f), 3);
}
#[tokio::test]
async fn folder_management_app_keeps_rejected_form_but_closes_uncertain_creation() {
    for mode in [4, 3] {
        let (f, manager, uid, _) = staged_write_fixture().await;
        f.folder_mode.store(mode, Ordering::SeqCst);
        let mut core = core_for_automatic_save(manager).await;
        core.open_folder(None, false);
        core.folder_editor.as_mut().unwrap().name = "Preserved form fixture".into();
        core.submit_folder();
        finish_automatic_sync(&mut core, uid).await;
        assert_eq!(folder_writes(&f), 1);
        if mode == 4 {
            let editor = core.folder_editor.as_ref().unwrap();
            assert!(!editor.busy);
            assert!(editor.error.is_some());
            assert_eq!(editor.name, "Preserved form fixture");
        } else {
            assert!(core.folder_editor.is_none());
            assert!(core.manager.has_folder_intent(&uid));
        }
    }
}

async fn clean_remote_fixture() -> (
    Fixture,
    crate::services::sdk::ClientManager,
    crate::domain::UserId,
    bitwarden_vault::CipherId,
) {
    let (f, manager, uid, id) = staged_write_fixture().await;
    manager
        .client_for(&uid)
        .unwrap()
        .discard_local(id)
        .await
        .unwrap();
    (f, manager, uid, id)
}
fn creation_count(f: &Fixture) -> usize {
    f.requests
        .lock()
        .unwrap()
        .iter()
        .filter(|(r, _)| {
            r.starts_with("POST /vault/api/ciphers HTTP")
                || r.starts_with("POST /vault/api/ciphers/create HTTP")
        })
        .count()
}
async fn new_login_view(
    manager: &crate::services::sdk::ClientManager,
    uid: crate::domain::UserId,
    id: bitwarden_vault::CipherId,
) -> bitwarden_vault::CipherView {
    use crate::services::sdk::ClientExt;
    let mut view = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    view.id = None;
    view.name = "Created fixture login".into();
    view.login.as_mut().unwrap().password = Some("created-only-password".into());
    view
}
#[tokio::test]
async fn new_entry_creation_is_encrypted_uses_server_ids_and_survives_restart() {
    use crate::services::sdk::{ClientExt, vault_sync};
    let (f, mut manager, uid, id) = clean_remote_fixture().await;
    let view = new_login_view(&manager, uid, id).await;
    let local = manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(view)
        .await
        .unwrap()
        .id
        .unwrap();
    assert_eq!(creation_count(&f), 0);
    assert_eq!(manager.local_edit_count(&uid), 1);
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.uploaded, 1, "{:?}", report.problems);
    assert!(report.problems.is_empty());
    assert_eq!(creation_count(&f), 1);
    assert_eq!(manager.local_edit_count(&uid), 0);
    let server = manager.resolve_cipher_id(&uid, local);
    assert_ne!(server, local);
    let view = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(server)
        .await
        .unwrap();
    assert_eq!(view.name, "Created fixture login");
    assert_eq!(
        view.login.unwrap().password.as_deref(),
        Some("created-only-password")
    );
    for (request, _) in f.requests.lock().unwrap().iter() {
        assert!(!request.contains("created-only-password"));
        assert!(!request.contains("Created fixture login"));
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
    assert_eq!(
        restored
            .client_for(&uid)
            .unwrap()
            .full_cipher(server)
            .await
            .unwrap()
            .name,
        "Created fixture login"
    );
}
#[tokio::test]
async fn new_entry_creation_lost_reply_is_not_duplicated_and_unapplied_creation_is_retained() {
    use crate::services::sdk::{ClientExt, vault_sync};
    for mode in [2, 3] {
        let (f, manager, uid, id) = clean_remote_fixture().await;
        f.write_mode.store(mode, Ordering::SeqCst);
        let view = new_login_view(&manager, uid, id).await;
        let local = manager
            .client_for(&uid)
            .unwrap()
            .save_cipher(view)
            .await
            .unwrap()
            .id
            .unwrap();
        let report = vault_sync::synchronize(
            manager.client_for(&uid).unwrap(),
            f.root.clone(),
            f.store.clone(),
        )
        .await
        .unwrap();
        assert_eq!(creation_count(&f), 1);
        if mode == 2 {
            assert_eq!(report.uploaded, 1);
            assert_eq!(manager.local_edit_count(&uid), 0);
            assert_ne!(manager.resolve_cipher_id(&uid, local), local);
        } else {
            assert_eq!(manager.local_edit_count(&uid), 1);
            assert_eq!(manager.local_conflicts(&uid), vec![local]);
            assert_eq!(
                manager
                    .client_for(&uid)
                    .unwrap()
                    .full_cipher(local)
                    .await
                    .unwrap()
                    .name,
                "Created fixture login"
            );
        }
        f.write_mode.store(1, Ordering::SeqCst);
        vault_sync::synchronize(
            manager.client_for(&uid).unwrap(),
            f.root.clone(),
            f.store.clone(),
        )
        .await
        .unwrap();
        assert_eq!(creation_count(&f), 1);
    }
}
#[tokio::test]
async fn new_entry_creation_in_writable_organization_collection_and_policy_rejection() {
    use crate::services::sdk::{ClientExt, test_support::encrypted_org_sync_response, vault_sync};
    let f = Fixture::new();
    let (manager, uid) = f.account().await;
    let (mut payload, org, id) = encrypted_org_sync_response(&manager, uid).await;
    payload["collections"][0]["readOnly"] = serde_json::json!(false);
    *f.sync_response.lock().unwrap() = (200, payload);
    vault_sync::download(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    f.write_mode.store(1, Ordering::SeqCst);
    let mut view = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    view.id = None;
    view.name = "Organization-created fixture".into();
    let local = manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(view)
        .await
        .unwrap()
        .id
        .unwrap();
    let report = vault_sync::synchronize(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(report.uploaded, 1, "{:?}", report.problems);
    let view = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(manager.resolve_cipher_id(&uid, local))
        .await
        .unwrap();
    assert_eq!(view.organization_id, Some(org));
    assert_eq!(view.collection_ids.len(), 1);
    f.sync_response.lock().unwrap().1["collections"][0]["readOnly"] = serde_json::json!(true);
    vault_sync::download(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    let mut forbidden = view;
    forbidden.id = None;
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .save_cipher(forbidden)
            .await
            .is_err()
    );
    assert_eq!(creation_count(&f), 1);
}
#[tokio::test]
async fn new_entry_creation_from_app_is_automatically_synchronized_and_selected() {
    let (f, manager, uid, _) = clean_remote_fixture().await;
    let mut core = core_for_automatic_save(manager).await;
    core.new_item(bitwarden_vault::CipherType::Login);
    assert!(core.editor.is_some());
    core.editor.as_mut().unwrap().data["name"] =
        serde_json::json!("Created from application fixture");
    core.save_editor();
    finish_automatic_sync(&mut core, uid).await;
    assert_eq!(creation_count(&f), 1);
    assert_eq!(core.manager.local_edit_count(&uid), 0);
    assert_eq!(
        core.detail.as_ref().unwrap().name,
        "Created from application fixture"
    );
    assert_eq!(core.selected, core.detail.as_ref().unwrap().id);
}
fn attachment_posts(f: &Fixture) -> usize {
    f.requests
        .lock()
        .unwrap()
        .iter()
        .filter(|(r, _)| {
            r.starts_with("POST ") && r.lines().next().unwrap_or("").contains("/attachment/")
        })
        .count()
}
async fn upload_fixture(
    f: &Fixture,
    manager: &crate::services::sdk::ClientManager,
    uid: crate::domain::UserId,
    id: bitwarden_vault::CipherId,
) -> crate::services::sdk::attachments::Outcome {
    let path = f.root.join("private-attachment-name.txt");
    std::fs::write(&path, b"Synthetic private attachment contents\n").unwrap();
    crate::services::sdk::attachments::perform(
        manager.client_for(&uid).unwrap(),
        id,
        crate::services::sdk::attachments::Operation::Upload(path),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap()
}
async fn first_attachment(
    manager: &crate::services::sdk::ClientManager,
    uid: crate::domain::UserId,
    id: bitwarden_vault::CipherId,
) -> String {
    use crate::services::sdk::ClientExt;
    manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap()
        .attachments
        .unwrap()[0]
        .id
        .clone()
        .unwrap()
}
#[tokio::test]
async fn attachments_upload_download_delete_use_sdk_encryption_and_account_mtls() {
    use crate::services::sdk::{
        ClientExt,
        attachments::{self, Operation},
    };
    let (f, manager, uid, id) = clean_remote_fixture().await;
    let uploaded = upload_fixture(&f, &manager, uid, id).await;
    assert!(uploaded.applied, "{:?}", uploaded.report.problems);
    assert!(uploaded.report.problems.is_empty());
    assert!(!manager.has_attachment_intent(&uid));
    let aid = first_attachment(&manager, uid, id).await;
    let view = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    assert_eq!(
        view.attachments.as_ref().unwrap()[0].file_name.as_deref(),
        Some("private-attachment-name.txt")
    );
    let stored = f
        .attachment_files
        .lock()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    assert!(!stored.windows(9).any(|w| w == b"Synthetic"));
    let output = f.root.join("downloaded.txt");
    let downloaded = attachments::perform(
        manager.client_for(&uid).unwrap(),
        id,
        Operation::Download {
            id: aid.clone(),
            path: output.clone(),
        },
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(downloaded.applied, "{:?}", downloaded.report.problems);
    assert_eq!(
        std::fs::read(&output).unwrap(),
        b"Synthetic private attachment contents\n"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(output).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    for (request, peer) in f.requests.lock().unwrap().iter() {
        assert!(!peer.is_empty());
        assert!(!request.contains("private-attachment-name.txt"));
        assert!(!request.contains("Synthetic private attachment contents"));
        if request.starts_with("GET /vault/attachments/") {
            assert!(!request.to_ascii_lowercase().contains("authorization:"));
        }
    }
    crate::services::sdk::test_support::retain_sync_gui_fixture(&f.root);
    let deleted = attachments::perform(
        manager.client_for(&uid).unwrap(),
        id,
        Operation::Delete(aid),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(deleted.applied, "{:?}", deleted.report.problems);
    assert!(f.attachment_files.lock().unwrap().is_empty());
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .attachments
            .unwrap_or_default()
            .is_empty()
    );
}
#[tokio::test]
async fn attachments_lost_binary_reply_is_verified_without_duplicate_upload() {
    let (f, manager, uid, id) = clean_remote_fixture().await;
    f.attachment_mode.store(3, Ordering::SeqCst);
    let uploaded = upload_fixture(&f, &manager, uid, id).await;
    assert!(uploaded.applied, "{:?}", uploaded.report.problems);
    assert!(uploaded.report.problems.is_empty());
    assert!(!manager.has_attachment_intent(&uid));
    assert_eq!(attachment_posts(&f), 2);
    crate::services::sdk::vault_sync::download(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(attachment_posts(&f), 2);
}
#[tokio::test]
async fn attachments_lost_slot_reply_preserves_incomplete_upload_and_can_be_removed() {
    use crate::services::sdk::attachments::{self, Operation};
    let (f, manager, uid, id) = clean_remote_fixture().await;
    f.attachment_mode.store(2, Ordering::SeqCst);
    let uploaded = upload_fixture(&f, &manager, uid, id).await;
    assert!(!uploaded.applied);
    assert!(manager.has_attachment_intent(&uid));
    assert_eq!(attachment_posts(&f), 1);
    assert!(f.attachment_files.lock().unwrap().is_empty());
    let aid = first_attachment(&manager, uid, id).await;
    f.attachment_mode.store(1, Ordering::SeqCst);
    let removed = attachments::perform(
        manager.client_for(&uid).unwrap(),
        id,
        Operation::Delete(aid),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(removed.applied, "{:?}", removed.report.problems);
    assert!(!manager.has_attachment_intent(&uid));
    assert_eq!(attachment_posts(&f), 1);
}
#[tokio::test]
async fn attachments_reject_external_urls_wrong_ids_tampering_and_oversized_downloads() {
    use crate::services::sdk::attachments::{self, Operation};
    let (f, manager, uid, id) = clean_remote_fixture().await;
    assert!(upload_fixture(&f, &manager, uid, id).await.applied);
    let aid = first_attachment(&manager, uid, id).await;
    for mode in [6, 7, 10, 13] {
        f.attachment_mode.store(mode, Ordering::SeqCst);
        let output = f.root.join(format!("preserved-{mode}.txt"));
        std::fs::write(&output, b"original output").unwrap();
        let outcome = attachments::perform(
            manager.client_for(&uid).unwrap(),
            id,
            Operation::Download {
                id: aid.clone(),
                path: output.clone(),
            },
            f.root.clone(),
            f.store.clone(),
        )
        .await
        .unwrap();
        assert!(
            !outcome.applied,
            "Mode {mode}: {:?}",
            outcome.report.problems
        );
        assert!(!outcome.report.problems.is_empty());
        assert_eq!(std::fs::read(output).unwrap(), b"original output");
    }
    assert!(std::fs::read_dir(&f.root).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".bitwarden-export-")
    }));
}
#[tokio::test]
async fn attachments_unknown_data_upload_survives_restart_and_is_not_resent() {
    use crate::services::sdk::{attachments, vault_sync};
    let (f, mut manager, uid, id) = clean_remote_fixture().await;
    f.attachment_mode.store(4, Ordering::SeqCst);
    let outcome = upload_fixture(&f, &manager, uid, id).await;
    assert!(!outcome.applied);
    assert!(manager.has_attachment_intent(&uid));
    assert_eq!(attachment_posts(&f), 2);
    manager.lock(&uid).unwrap().finish().await;
    drop(manager);
    let restored = crate::services::sdk::production::test_load(&f.root, &f.store).await;
    restored
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    assert!(restored.has_attachment_intent(&uid));
    f.attachment_mode.store(1, Ordering::SeqCst);
    vault_sync::download(
        restored.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(restored.has_attachment_intent(&uid));
    assert_eq!(attachment_posts(&f), 2);
    let aid = first_attachment(&restored, uid, id).await;
    assert!(
        attachments::perform(
            restored.client_for(&uid).unwrap(),
            id,
            attachments::Operation::Delete(aid),
            f.root.clone(),
            f.store.clone()
        )
        .await
        .unwrap()
        .applied
    );
}
#[tokio::test]
async fn attachments_lock_during_transfer_cancels_promptly_and_never_replays_the_upload() {
    use crate::services::sdk::{
        attachments::{self, Operation},
        vault_sync,
    };
    let (f, mut manager, uid, id) = clean_remote_fixture().await;
    f.attachment_mode.store(8, Ordering::SeqCst);
    let path = f.root.join("lock-upload.txt");
    std::fs::write(&path, b"Lock fixture attachment").unwrap();
    let task = tokio::spawn(attachments::perform(
        manager.client_for(&uid).unwrap(),
        id,
        Operation::Upload(path),
        f.root.clone(),
        f.store.clone(),
    ));
    for _ in 0..400 {
        if attachment_posts(&f) == 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(attachment_posts(&f), 2);
    let began = std::time::Instant::now();
    manager.lock(&uid).unwrap().finish().await;
    assert!(began.elapsed() < std::time::Duration::from_millis(450));
    assert!(task.await.unwrap().is_err());
    drop(manager);
    let restored = crate::services::sdk::production::test_load(&f.root, &f.store).await;
    restored
        .unlock_data_for(&uid)
        .unwrap()
        .unlock("synthetic-password".into())
        .await
        .unwrap();
    f.attachment_mode.store(1, Ordering::SeqCst);
    vault_sync::download(
        restored.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(!restored.has_attachment_intent(&uid));
    assert_eq!(attachment_posts(&f), 2);
}

#[tokio::test]
async fn attachments_reprompt_and_fresh_permissions_guard_file_access() {
    use crate::services::sdk::{
        Action, Authorization, ClientExt,
        attachments::{self, Operation},
        vault_sync,
    };
    let (f, manager, uid, id) = clean_remote_fixture().await;
    assert!(upload_fixture(&f, &manager, uid, id).await.applied);
    let aid = first_attachment(&manager, uid, id).await;
    f.sync_response.lock().unwrap().1["ciphers"][0]["reprompt"] = serde_json::json!(1);
    vault_sync::download(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    let revision = manager
        .client_for(&uid)
        .unwrap()
        .list_ciphers()
        .await
        .unwrap()[0]
        .revision_date;
    let output = f.root.join("protected-output.txt");
    assert!(
        attachments::perform(
            manager.client_for(&uid).unwrap(),
            id,
            Operation::Download {
                id: aid.clone(),
                path: output.clone()
            },
            f.root.clone(),
            f.store.clone()
        )
        .await
        .is_err()
    );
    assert!(!output.exists());
    let (client, envelope) = manager.validation_data_for(&uid).unwrap();
    client
        .authorize_password(
            envelope,
            "synthetic-password".into(),
            Authorization {
                cipher: Some(id),
                revision: Some(revision),
                action: Action::Copy,
            },
        )
        .await
        .unwrap();
    assert!(
        attachments::perform(
            manager.client_for(&uid).unwrap(),
            id,
            Operation::Download {
                id: aid.clone(),
                path: output.clone()
            },
            f.root.clone(),
            f.store.clone()
        )
        .await
        .unwrap()
        .applied
    );
    assert!(output.exists());
    assert!(
        attachments::perform(
            manager.client_for(&uid).unwrap(),
            id,
            Operation::Download {
                id: aid.clone(),
                path: output
            },
            f.root.clone(),
            f.store.clone()
        )
        .await
        .is_err()
    );
    f.sync_response.lock().unwrap().1["ciphers"][0]["reprompt"] = serde_json::json!(0);
    vault_sync::download(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    f.sync_response.lock().unwrap().1["ciphers"][0]["edit"] = serde_json::json!(false);
    let denied = attachments::perform(
        manager.client_for(&uid).unwrap(),
        id,
        Operation::Delete(aid),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(!denied.applied);
    assert_eq!(f.attachment_files.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn attachments_reject_upload_to_an_external_endpoint_and_rejected_metadata_clears_intent() {
    for mode in [5, 11] {
        let (f, manager, uid, id) = clean_remote_fixture().await;
        f.attachment_mode.store(mode, Ordering::SeqCst);
        let outcome = upload_fixture(&f, &manager, uid, id).await;
        assert!(!outcome.applied);
        assert!(!outcome.report.problems.is_empty());
        assert_eq!(attachment_posts(&f), 1);
        assert!(f.attachment_files.lock().unwrap().is_empty());
        assert_eq!(manager.has_attachment_intent(&uid), mode == 11);
    }
}
#[cfg(unix)]
#[tokio::test]
async fn attachments_do_not_upload_a_symlink_source() {
    use crate::services::sdk::attachments::{self, Operation};
    let (f, manager, uid, id) = clean_remote_fixture().await;
    let real = f.root.join("source.txt");
    std::fs::write(&real, b"Do not follow the symlink").unwrap();
    let link = f.root.join("link.txt");
    std::os::unix::fs::symlink(real, &link).unwrap();
    let result = attachments::perform(
        manager.client_for(&uid).unwrap(),
        id,
        Operation::Upload(link),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(!result.applied);
    assert_eq!(attachment_posts(&f), 0);
    assert!(!manager.has_attachment_intent(&uid));
}
async fn finish_incoming(core: &mut crate::core::Core) {
    for _ in 0..400 {
        core.poll();
        if core.incoming_idle_for_test() {
            core.poll();
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("Incoming sync did not finish");
}
#[tokio::test]
async fn incoming_sync_only_downloads_changed_revisions_and_preserves_selection_and_search() {
    use crate::services::sdk::{ClientExt, test_support::encrypted_sync_response};
    let (f, manager, uid, id) = clean_remote_fixture().await;
    let mut core = core_for_automatic_save(manager).await;
    let baseline = f.sync_requests();
    let generation = core.generation;
    core.query = "incoming".into();
    core.selected = Some(id);
    core.incoming_for_test();
    finish_incoming(&mut core).await;
    assert_eq!(f.sync_requests(), baseline);
    assert_eq!(core.generation, generation);
    let payload = encrypted_sync_response(&core.manager, uid, "Incoming remote fixture").await;
    *f.sync_response.lock().unwrap() = (200, payload);
    core.incoming_for_test();
    finish_incoming(&mut core).await;
    for _ in 0..100 {
        core.poll();
        if core
            .current_cache()
            .is_some_and(|c| c.all.iter().any(|i| i.name == "Incoming remote fixture"))
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(f.sync_requests(), baseline + 1);
    assert_eq!(core.query, "incoming");
    assert!(
        core.manager
            .client_for(&uid)
            .unwrap()
            .list_ciphers()
            .await
            .unwrap()
            .iter()
            .any(|c| c.name == "Incoming remote fixture")
    );
    assert!(core.notice.is_none());
}
#[tokio::test]
async fn incoming_sync_defers_open_editors_does_not_upload_drafts_and_stops_when_locked() {
    use crate::services::sdk::test_support::encrypted_sync_response;
    let (f, manager, uid, id) = staged_write_fixture().await;
    let mut core = core_for_automatic_save(manager).await;
    core.new_item(bitwarden_vault::CipherType::Login);
    let editor = core.editor.as_ref().unwrap().id;
    let requests = f.requests.lock().unwrap().len();
    core.incoming_for_test();
    finish_incoming(&mut core).await;
    assert_eq!(core.editor.as_ref().unwrap().id, editor);
    assert_eq!(f.requests.lock().unwrap().len(), requests);
    core.editor = None;
    *f.sync_response.lock().unwrap() = (
        200,
        encrypted_sync_response(&core.manager, uid, "Incoming conflict fixture").await,
    );
    core.incoming_for_test();
    finish_incoming(&mut core).await;
    assert_eq!(put_count(&f), 0);
    assert_eq!(creation_count(&f), 0);
    assert_eq!(core.manager.local_conflicts(&uid), vec![id]);
    core.lock_user(uid);
    let requests = f.requests.lock().unwrap().len();
    core.incoming_for_test();
    finish_incoming(&mut core).await;
    assert_eq!(f.requests.lock().unwrap().len(), requests);
}

#[tokio::test]
async fn attachments_legacy_v1_download_uses_the_sdk_user_key_path() {
    use crate::services::sdk::{
        attachments::{self, Operation},
        test_support::{
            encrypted_legacy_attachment_bytes, encrypted_legacy_attachment_sync_response,
        },
        vault_sync,
    };
    let f = Fixture::new();
    let (manager, uid) = f.account().await;
    let mut payload = encrypted_legacy_attachment_sync_response(&manager, uid).await;
    let cid: bitwarden_vault::CipherId = payload["ciphers"][0]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let aid = "synthetic-v1-attachment";
    let data = encrypted_legacy_attachment_bytes(&manager, uid, b"Legacy attachment bytes");
    let path = format!("/vault/attachments/{cid}/{aid}");
    f.attachment_files.lock().unwrap().insert(path, data);
    payload["ciphers"][0]["attachments"][0]["url"] = serde_json::json!(format!(
        "{}attachments/{cid}/{aid}?token=fixture",
        f.profile.base
    ));
    *f.sync_response.lock().unwrap() = (200, payload);
    vault_sync::download(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    let output = f.root.join("legacy-downloaded.txt");
    let result = attachments::perform(
        manager.client_for(&uid).unwrap(),
        cid,
        Operation::Download {
            id: aid.into(),
            path: output.clone(),
        },
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(result.applied, "{:?}", result.report.problems);
    assert_eq!(std::fs::read(output).unwrap(), b"Legacy attachment bytes");
}
#[tokio::test]
async fn attachments_lost_delete_reply_is_reconciled_and_certificate_changes_block_publication() {
    use crate::services::sdk::attachments::{self, Operation};
    let (f, manager, uid, cid) = clean_remote_fixture().await;
    assert!(upload_fixture(&f, &manager, uid, cid).await.applied);
    let aid = first_attachment(&manager, uid, cid).await;
    f.attachment_mode.store(15, Ordering::SeqCst);
    let output = f.root.join("binding-preserved.txt");
    std::fs::write(&output, b"Original output").unwrap();
    let before = f
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|(r, _)| r.starts_with("GET /vault/attachments/"))
        .count();
    let task = tokio::spawn(attachments::perform(
        manager.client_for(&uid).unwrap(),
        cid,
        Operation::Download {
            id: aid.clone(),
            path: output.clone(),
        },
        f.root.clone(),
        f.store.clone(),
    ));
    for _ in 0..200 {
        if f.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(r, _)| r.starts_with("GET /vault/attachments/"))
            .count()
            > before
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let profile = f.store.get(manager.profile_for(&uid).unwrap()).unwrap();
    let revision = profile.revision;
    f.store.save(profile, Some(revision)).unwrap();
    let result = task.await.unwrap().unwrap();
    assert!(!result.applied);
    assert_eq!(std::fs::read(output).unwrap(), b"Original output");
    f.attachment_mode.store(9, Ordering::SeqCst);
    let result = attachments::perform(
        manager.client_for(&uid).unwrap(),
        cid,
        Operation::Delete(aid),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(result.applied, "{:?}", result.report.problems);
    assert!(!manager.has_attachment_intent(&uid));
}
#[tokio::test]
async fn incoming_sync_checkpoint_does_not_miss_a_change_during_the_full_download() {
    use crate::services::sdk::{incoming, test_support::encrypted_sync_response, vault_sync};
    let (f, manager, uid, _) = clean_remote_fixture().await;
    let before = f.sync_requests();
    f.sync_delay.store(300, Ordering::SeqCst);
    let task = tokio::spawn(vault_sync::download(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    ));
    for _ in 0..200 {
        if f.sync_requests() > before {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let payload = encrypted_sync_response(&manager, uid, "Revision changed during download").await;
    *f.sync_response.lock().unwrap() = (200, payload);
    task.await.unwrap().unwrap();
    f.sync_delay.store(0, Ordering::SeqCst);
    assert!(
        incoming::changed(
            manager.client_for(&uid).unwrap(),
            f.root.clone(),
            f.store.clone()
        )
        .await
        .unwrap()
    );
    vault_sync::download(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert!(
        !incoming::changed(
            manager.client_for(&uid).unwrap(),
            f.root.clone(),
            f.store.clone()
        )
        .await
        .unwrap()
    );
}

#[tokio::test]
async fn incoming_sync_preserves_generator_opened_while_download_was_in_flight() {
    use crate::services::sdk::test_support::encrypted_sync_response;
    let (f, manager, _, _) = clean_remote_fixture().await;
    let mut core = core_for_automatic_save(manager).await;
    let uid = core.active.unwrap();
    *f.sync_response.lock().unwrap() = (
        200,
        encrypted_sync_response(&core.manager, uid, "Changed during generator use").await,
    );
    f.sync_delay.store(300, Ordering::SeqCst);
    let before = f.sync_requests();
    core.incoming_for_test();
    for _ in 0..200 {
        core.poll();
        if f.sync_requests() > before {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(f.sync_requests() > before);
    core.generator_open = true;
    core.generator.output = Zeroizing::new("Synthetic generated value".into());
    finish_incoming(&mut core).await;
    assert!(core.generator_open);
    assert_eq!(core.generator.output.as_str(), "Synthetic generated value");
    assert!(core.notice.is_none());
}

#[tokio::test]
async fn incoming_sync_periodically_refreshes_policy_changes_without_a_user_revision_bump() {
    use crate::services::sdk::{incoming, test_support::age_sync_snapshot, vault_sync};
    let (f, manager, uid, _) = clean_remote_fixture().await;
    let revision = age_sync_snapshot(&manager, uid);
    *f.revision_override.lock().unwrap() = Some(revision);
    org_policy(
        &mut f.sync_response.lock().unwrap().1,
        Uuid::new_v4(),
        2,
        serde_json::json!({"minLength":28}),
    );
    assert!(
        incoming::changed(
            manager.client_for(&uid).unwrap(),
            f.root.clone(),
            f.store.clone()
        )
        .await
        .unwrap()
    );
    vault_sync::download(
        manager.client_for(&uid).unwrap(),
        f.root.clone(),
        f.store.clone(),
    )
    .await
    .unwrap();
    assert_eq!(manager.policy_rules_for(&uid).generator.min_length, 28);
    assert!(
        !incoming::changed(
            manager.client_for(&uid).unwrap(),
            f.root.clone(),
            f.store.clone()
        )
        .await
        .unwrap()
    );
}
