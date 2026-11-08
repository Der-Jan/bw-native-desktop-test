//! Compatibility with Bitwarden's RSA-OAEP/SHA1 + AES-CBC/HMAC native channel.
//! Algorithms are fixed by the existing extension protocol; MAC before decrypt.
use super::Connection;
use base64::{Engine, engine::general_purpose::STANDARD};
use openssl::{
    encrypt::Encrypter,
    hash::MessageDigest,
    pkey::PKey,
    rand::rand_bytes,
    rsa::Padding,
    sign::Signer,
    symm::{Cipher, decrypt, encrypt},
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use zeroize::Zeroizing;
pub(super) struct Channel {
    app_id: String,
    key: Zeroizing<Vec<u8>>,
    seen: Mutex<VecDeque<(i64, i64)>>,
}
fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
fn recent(timestamp: i64) -> bool {
    now().abs_diff(timestamp) <= 10_000
}
impl Channel {
    pub fn setup(value: &Value, connection: &Connection) -> Result<(Arc<Self>, Vec<u8>), String> {
        let app_id = value["appId"]
            .as_str()
            .filter(|v| {
                !v.is_empty()
                    && v.len() <= 128
                    && v.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            })
            .ok_or("Invalid browser app ID")?
            .to_owned();
        let message = &value["message"];
        if message["command"] != "setupEncryption"
            || !message["timestamp"].as_i64().is_some_and(recent)
            || message["userId"]
                .as_str()
                .and_then(|s| s.parse::<bitwarden_core::UserId>().ok())
                != Some(connection.session.server_user_id)
        {
            return Err("Invalid browser setup or account".into());
        }
        let public = message["publicKey"]
            .as_str()
            .filter(|v| v.len() <= 2048)
            .ok_or("Missing browser public key")?;
        let public = STANDARD
            .decode(public)
            .map_err(|_| "Invalid browser public key")?;
        let pkey = PKey::public_key_from_der(&public).map_err(|_| "Invalid RSA public key")?;
        let rsa = pkey.rsa().map_err(|_| "Browser key must be RSA")?;
        if !matches!(rsa.size(), 256 | 384 | 512) {
            return Err("Unsupported RSA key size".into());
        }
        let mut key = Zeroizing::new(vec![0u8; 64]);
        rand_bytes(&mut key).map_err(|_| "Random source unavailable")?;
        let mut wrapper = Encrypter::new(&pkey).map_err(|_| "RSA unavailable")?;
        wrapper
            .set_rsa_padding(Padding::PKCS1_OAEP)
            .map_err(|_| "RSA unavailable")?;
        wrapper
            .set_rsa_oaep_md(MessageDigest::sha1())
            .map_err(|_| "RSA unavailable")?;
        wrapper
            .set_rsa_mgf1_md(MessageDigest::sha1())
            .map_err(|_| "RSA unavailable")?;
        let mut encrypted = vec![0; wrapper.encrypt_len(&key).map_err(|_| "RSA unavailable")?];
        let n = wrapper
            .encrypt(&key, &mut encrypted)
            .map_err(|_| "RSA setup failed")?;
        encrypted.truncate(n);
        let response=serde_json::to_vec(&serde_json::json!({"command":"setupEncryption","appId":app_id,"messageId":-1,"sharedSecret":STANDARD.encode(encrypted)})).map_err(|_|"Setup response failed")?;
        Ok((
            Arc::new(Self {
                app_id,
                key,
                seen: Mutex::new(VecDeque::new()),
            }),
            response,
        ))
    }
    pub fn request(&self, value: &Value) -> Result<Value, String> {
        if value["appId"].as_str() != Some(&self.app_id) {
            return Err("Browser app ID changed".into());
        }
        let encoded = value["message"]
            .as_str()
            .or_else(|| value["message"]["encryptedString"].as_str())
            .ok_or("Missing encrypted message")?;
        let bytes = decrypt_bytes(&self.key, encoded)?;
        let message: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid encrypted request")?;
        let timestamp = message["timestamp"]
            .as_i64()
            .filter(|t| recent(*t))
            .ok_or("Expired browser request")?;
        let id = message["messageId"]
            .as_i64()
            .filter(|id| (0..=9_007_199_254_740_991).contains(id))
            .ok_or("Invalid browser request ID")?;
        let mut seen = self.seen.lock().unwrap();
        seen.retain(|(_, t)| recent(*t));
        if seen.len() >= 512 || seen.iter().any(|(seen, _)| *seen == id) {
            return Err("Repeated browser request".into());
        }
        seen.push_back((id, timestamp));
        Ok(message)
    }
    pub async fn respond(
        &self,
        message: Value,
        connection: Arc<Connection>,
    ) -> Result<Vec<u8>, String> {
        let command = message["command"]
            .as_str()
            .ok_or("Missing browser command")?;
        let id = message["messageId"]
            .as_i64()
            .ok_or("Missing browser request ID")?;
        let user = message["userId"]
            .as_str()
            .and_then(|s| s.parse::<bitwarden_core::UserId>().ok());
        let mut key = None;
        let outcome = match command {
            "getBiometricsStatus" | "getBiometricsStatusForUser" => {
                Outcome::Status(connection.status(user))
            }
            "unlockWithBiometricsForUser" => {
                if let Some(user) = user {
                    key = connection.key(user).await.map(|k| super::encode_key(&k));
                }
                Outcome::Boolean(key.is_some())
            }
            "authenticateWithBiometrics" => Outcome::Boolean(false),
            _ => Outcome::Boolean(false),
        };
        if !connection.current() {
            return Err("Browser session changed".into());
        }
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Reply<'a> {
            command: &'a str,
            message_id: i64,
            timestamp: i64,
            response: Outcome,
            #[serde(skip_serializing_if = "Option::is_none")]
            user_key_b64: Option<&'a str>,
        }
        let body = Zeroizing::new(
            serde_json::to_vec(&Reply {
                command,
                message_id: id,
                timestamp: now(),
                response: outcome,
                user_key_b64: key.as_deref().map(|s| s.as_str()),
            })
            .map_err(|_| "Reply encoding failed")?,
        );
        let encrypted = encrypt_bytes(&self.key, &body)?;
        serde_json::to_vec(
            &serde_json::json!({"appId":self.app_id,"messageId":id,"message":encrypted}),
        )
        .map_err(|_| "Reply encoding failed".into())
    }
}
#[derive(Serialize)]
#[serde(untagged)]
enum Outcome {
    Boolean(bool),
    Status(u8),
}
fn mac(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let key = PKey::hmac(&key[32..]).map_err(|_| "MAC unavailable")?;
    let mut signer = Signer::new(MessageDigest::sha256(), &key).map_err(|_| "MAC unavailable")?;
    signer
        .update(iv)
        .and_then(|_| signer.update(data))
        .map_err(|_| "MAC failed")?;
    signer.sign_to_vec().map_err(|_| "MAC failed".into())
}
fn encrypt_bytes(key: &[u8], body: &[u8]) -> Result<String, String> {
    let mut iv = [0u8; 16];
    rand_bytes(&mut iv).map_err(|_| "Random source unavailable")?;
    let encrypted = encrypt(Cipher::aes_256_cbc(), &key[..32], Some(&iv), body)
        .map_err(|_| "Encryption failed")?;
    Ok(format!(
        "2.{}|{}|{}",
        STANDARD.encode(iv),
        STANDARD.encode(&encrypted),
        STANDARD.encode(mac(key, &iv, &encrypted)?)
    ))
}
fn decrypt_bytes(key: &[u8], encoded: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    if encoded.len() > 16 * 1024 {
        return Err("Browser request too large".into());
    }
    let pieces: Vec<_> = encoded
        .strip_prefix("2.")
        .ok_or("Unsupported browser encryption")?
        .split('|')
        .collect();
    if pieces.len() != 3 {
        return Err("Invalid encrypted browser message".into());
    }
    let iv = STANDARD.decode(pieces[0]).map_err(|_| "Invalid IV")?;
    let data = STANDARD
        .decode(pieces[1])
        .map_err(|_| "Invalid ciphertext")?;
    let tag = STANDARD.decode(pieces[2]).map_err(|_| "Invalid MAC")?;
    if iv.len() != 16
        || tag.len() != 32
        || data.is_empty()
        || data.len() % 16 != 0
        || !openssl::memcmp::eq(&tag, &mac(key, &iv, &data)?)
    {
        return Err("Invalid encrypted browser message".into());
    }
    decrypt(Cipher::aes_256_cbc(), &key[..32], Some(&iv), &data)
        .map(Zeroizing::new)
        .map_err(|_| "Invalid encrypted browser message".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitwarden_crypto::{EncString, KeyDecryptable, KeyEncryptable, SymmetricCryptoKey};
    fn setup(connection: &Connection) -> (Arc<Channel>, Vec<u8>, PKey<openssl::pkey::Private>) {
        let private = PKey::from_rsa(openssl::rsa::Rsa::generate(2048).unwrap()).unwrap();
        let request = serde_json::json!({"appId":"test-browser-app","message":{"command":"setupEncryption","timestamp":now(),"userId":connection.session.server_user_id,"publicKey":STANDARD.encode(private.public_key_to_der().unwrap())}});
        let (channel, reply) = Channel::setup(&request, connection).unwrap();
        (channel, reply, private)
    }
    #[tokio::test]
    async fn native_encryption_matches_sdk_and_rejects_tampering_and_replay() {
        let (_runtime, _manager, connection) = super::super::test_fixture();
        let (channel, reply, private) = setup(&connection);
        let reply: Value = serde_json::from_slice(&reply).unwrap();
        let mut unwrap = openssl::encrypt::Decrypter::new(&private).unwrap();
        unwrap.set_rsa_padding(Padding::PKCS1_OAEP).unwrap();
        unwrap.set_rsa_oaep_md(MessageDigest::sha1()).unwrap();
        unwrap.set_rsa_mgf1_md(MessageDigest::sha1()).unwrap();
        let encrypted = STANDARD
            .decode(reply["sharedSecret"].as_str().unwrap())
            .unwrap();
        let mut clear = Zeroizing::new(vec![0; unwrap.decrypt_len(&encrypted).unwrap()]);
        let n = unwrap.decrypt(&encrypted, &mut clear).unwrap();
        clear.truncate(n);
        assert_eq!(*clear, *channel.key);
        let key = STANDARD
            .encode(&*channel.key)
            .parse::<SymmetricCryptoKey>()
            .unwrap();
        let message = serde_json::json!({"command":"getBiometricsStatusForUser","messageId":42,"timestamp":now(),"userId":connection.session.server_user_id});
        let cipher = message
            .to_string()
            .encrypt_with_key(&key)
            .unwrap()
            .to_string();
        let request =
            serde_json::json!({"appId":"test-browser-app","message":{"encryptedString":cipher}});
        let decoded = channel.request(&request).unwrap();
        assert_eq!(decoded, message);
        assert!(channel.request(&request).is_err());
        let response = channel.respond(decoded, connection.clone()).await.unwrap();
        let response: Value = serde_json::from_slice(&response).unwrap();
        let enc: EncString = response["message"].as_str().unwrap().parse().unwrap();
        let clear: String = enc.decrypt_with_key(&key).unwrap();
        let clear: Value = serde_json::from_str(&clear).unwrap();
        assert_eq!(clear["response"], 0);
        let expired=serde_json::json!({"command":"getBiometricsStatus","messageId":43,"timestamp":now()-20_000}).to_string().encrypt_with_key(&key).unwrap().to_string();
        assert!(
            channel
                .request(&serde_json::json!({"appId":"test-browser-app","message":expired}))
                .is_err()
        );
        let valid = encrypt_bytes(&channel.key, b"test").unwrap();
        let mut pieces: Vec<String> = valid[2..].split('|').map(str::to_owned).collect();
        let mut mac = STANDARD.decode(&pieces[2]).unwrap();
        mac[0] ^= 1;
        pieces[2] = STANDARD.encode(mac);
        assert!(decrypt_bytes(&channel.key, &format!("2.{}", pieces.join("|"))).is_err());
    }
    #[tokio::test]
    async fn legacy_never_releases_a_key_without_approval_or_for_another_account() {
        let (runtime, _manager, connection) = super::super::test_fixture();
        let mut events = runtime.events().subscribe();
        let (channel, _, _) = setup(&connection);
        let c = connection.clone();
        let ch = channel.clone();
        let id = connection.session.server_user_id;
        let pending = tokio::spawn(async move {
            ch.respond(serde_json::json!({"command":"unlockWithBiometricsForUser","userId":id,"messageId":1}),c).await.unwrap()
        });
        let approval = loop {
            if let super::super::Event::Approval(a) = events.recv().await.unwrap() {
                break a;
            }
        };
        assert!(!pending.is_finished());
        approval.respond(super::super::Decision::Deny);
        let encrypted: Value = serde_json::from_slice(&pending.await.unwrap()).unwrap();
        let clear = decrypt_bytes(&channel.key, encrypted["message"].as_str().unwrap()).unwrap();
        let clear: Value = serde_json::from_slice(&clear).unwrap();
        assert_eq!(clear["response"], false);
        assert!(clear.get("userKeyB64").is_none());
    }
}
