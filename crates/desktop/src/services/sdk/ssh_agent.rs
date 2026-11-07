//! Ephemeral SSH operations behind the same revocable SDK leases as vault reads.
//! No private key or raw SDK handle leaves this module.
use super::{ClientExt, SessionToken, session_client::SessionClient};
use bitwarden_vault::{Cipher, CipherId, CipherRepromptType, CipherType};
use chrono::{DateTime, Utc};
use signature::{Signer, Verifier};
use ssh_key::{Algorithm, HashAlg, PrivateKey, PublicKey, Signature};
use std::sync::Arc;
use zeroize::Zeroizing;

const DENIED: &str = "SSH-Anfrage abgelehnt";
#[derive(Debug)]
pub struct InvalidKey;
#[derive(Clone)]
pub struct AgentSession {
    token: SessionToken,
}
#[derive(Clone, Debug)]
pub struct AgentIdentity {
    pub id: CipherId,
    pub revision: DateTime<Utc>,
    pub name: String,
    pub public_key: Vec<u8>,
    pub fingerprint: String,
}
impl AgentSession {
    pub(super) fn new(token: SessionToken) -> Self {
        Self { token }
    }
    pub fn valid(&self) -> bool {
        self.token.ensure_unlocked().is_ok()
    }
    pub fn same_authority(&self, other: &Self) -> bool {
        self.token.generation == other.token.generation
            && Arc::ptr_eq(&self.token.state, &other.token.state)
    }
    fn client(&self) -> SessionClient {
        SessionClient {
            session: self.token.clone(),
        }
    }
    pub async fn identities(&self) -> Result<Vec<AgentIdentity>, String> {
        self.client()
            .run(|client| async move {
                let repo = client
                    .platform()
                    .state()
                    .get::<Cipher>()
                    .map_err(|_| DENIED)?;
                let mut result = Vec::new();
                for encrypted in repo.list().await.map_err(|_| DENIED)? {
                    if encrypted.r#type != CipherType::SshKey
                        || encrypted.deleted_date.is_some()
                        || encrypted.archived_date.is_some()
                        || !encrypted.view_password
                        || encrypted.reprompt != CipherRepromptType::None
                    {
                        continue;
                    }
                    let Some(id) = encrypted.id else { continue };
                    let Ok(mut view) = client.clone().full_cipher(id).await else {
                        continue;
                    };
                    if !view.view_password
                        || view.reprompt != CipherRepromptType::None
                        || view.deleted_date.is_some()
                        || view.archived_date.is_some()
                    {
                        continue;
                    }
                    let Some(mut ssh) = view.ssh_key.take() else {
                        continue;
                    };
                    let secret = Zeroizing::new(std::mem::take(&mut ssh.private_key));
                    if secret.is_empty() || secret.len() > 128_000 || ssh.public_key.len() > 16_384
                    {
                        continue;
                    }
                    let Ok(public) = PublicKey::from_openssh(&ssh.public_key) else {
                        continue;
                    };
                    if validate_public(&public).is_err() {
                        continue;
                    }
                    let Ok(blob) = public.to_bytes() else {
                        continue;
                    };
                    result.push(AgentIdentity {
                        id,
                        revision: view.revision_date,
                        name: clean_label(&view.name, 256),
                        public_key: blob,
                        fingerprint: public.fingerprint(HashAlg::Sha256).to_string(),
                    });
                    if result.len() == 64 {
                        break;
                    }
                }
                result.sort_by_key(|a| a.id);
                Ok(result)
            })
            .await
            .map_err(|_| DENIED.into())
    }
    pub async fn sign(
        &self,
        identity: AgentIdentity,
        data: Vec<u8>,
        flags: u32,
    ) -> Result<Vec<u8>, String> {
        if data.len() > 65_536 {
            return Err(DENIED.into());
        }
        let session = self.token.clone();
        self.client()
            .run(move |client| async move {
                super::session_client::check_cipher(
                    &client,
                    &session,
                    identity.id,
                    super::Action::Read,
                    Some(identity.revision),
                )
                .await?;
                let mut view = client.full_cipher(identity.id).await.map_err(|_| DENIED)?;
                if view.r#type != CipherType::SshKey
                    || view.deleted_date.is_some()
                    || view.archived_date.is_some()
                    || !view.view_password
                    || view.reprompt != CipherRepromptType::None
                    || view.revision_date != identity.revision
                {
                    return Err(DENIED.into());
                }
                let Some(mut ssh) = view.ssh_key.take() else {
                    return Err(DENIED.into());
                };
                let secret = Zeroizing::new(std::mem::take(&mut ssh.private_key));
                sign_private(&secret, &identity.public_key, &data, flags).map_err(|_| DENIED.into())
            })
            .await
            .map_err(|_| DENIED.into())
    }
}

pub fn clean_label(value: &str, limit: usize) -> String {
    let mut result = String::new();
    for c in value.chars().filter(|c| {
        !c.is_control() && !matches!(*c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
    }) {
        if result.len() + c.len_utf8() > limit {
            break;
        }
        result.push(c);
    }
    result
}
fn validate_public(key: &PublicKey) -> Result<(), InvalidKey> {
    match key.algorithm() {
        Algorithm::Ed25519 | Algorithm::Ecdsa { .. } => Ok(()),
        Algorithm::Rsa { .. }
            if key
                .key_data()
                .rsa()
                .is_some_and(|r| (2048..=8192).contains(&r.key_size())) =>
        {
            Ok(())
        }
        _ => Err(InvalidKey),
    }
}
pub fn signing_algorithm(blob: &[u8], flags: u32) -> Result<String, InvalidKey> {
    if blob.len() > 16_384 {
        return Err(InvalidKey);
    }
    let key = PublicKey::from_bytes(blob).map_err(|_| InvalidKey)?;
    validate_public(&key)?;
    match (key.algorithm(), flags) {
        (Algorithm::Ed25519, 0) => Ok("ssh-ed25519".into()),
        (Algorithm::Ecdsa { curve }, 0) => Ok(format!("ecdsa-sha2-{curve}")),
        (Algorithm::Rsa { .. }, 2) => Ok("rsa-sha2-256".into()),
        (Algorithm::Rsa { .. }, 4) => Ok("rsa-sha2-512".into()),
        _ => Err(InvalidKey),
    }
}
pub fn verify_binding(host: &[u8], session: &[u8], signature: &[u8]) -> Result<String, InvalidKey> {
    if host.len() > 16_384 || signature.len() > 16_384 || !(16..=128).contains(&session.len()) {
        return Err(InvalidKey);
    }
    let key = PublicKey::from_bytes(host).map_err(|_| InvalidKey)?;
    validate_public(&key)?;
    let signature = Signature::try_from(signature).map_err(|_| InvalidKey)?;
    if matches!(signature.algorithm(), Algorithm::Rsa { hash: None }) {
        return Err(InvalidKey);
    }
    key.key_data()
        .verify(session, &signature)
        .map_err(|_| InvalidKey)?;
    Ok(key.fingerprint(HashAlg::Sha256).to_string())
}
fn sign_private(
    secret: &str,
    expected_public: &[u8],
    data: &[u8],
    flags: u32,
) -> Result<Vec<u8>, InvalidKey> {
    if secret.len() > 128_000 || secret.contains("ENCRYPTED") {
        return Err(InvalidKey);
    }
    let input = if secret.contains("-----BEGIN RSA PRIVATE KEY-----") {
        let key =
            openssl::pkey::PKey::private_key_from_pem(secret.as_bytes()).map_err(|_| InvalidKey)?;
        Zeroizing::new(
            String::from_utf8(key.private_key_to_pem_pkcs8().map_err(|_| InvalidKey)?)
                .map_err(|_| InvalidKey)?,
        )
    } else {
        Zeroizing::new(secret.to_owned())
    };
    // The SDK normalizes supported PEM/OpenSSH formats; no key is written to disk.
    let mut normalized =
        bitwarden_ssh::import::import_key(input.to_string(), None).map_err(|_| InvalidKey)?;
    let normalized_secret = Zeroizing::new(std::mem::take(&mut normalized.private_key));
    let key = PrivateKey::from_openssh(normalized_secret.as_bytes()).map_err(|_| InvalidKey)?;
    if key.public_key().to_bytes().map_err(|_| InvalidKey)? != expected_public {
        return Err(InvalidKey);
    }
    signing_algorithm(expected_public, flags)?;
    let signature: Signature = match key.key_data() {
        ssh_key::private::KeypairData::Rsa(rsa) => (
            rsa,
            Some(if flags == 2 {
                HashAlg::Sha256
            } else {
                HashAlg::Sha512
            }),
        )
            .try_sign(data)
            .map_err(|_| InvalidKey)?,
        _ => key.try_sign(data).map_err(|_| InvalidKey)?,
    };
    signature.try_into().map_err(|_| InvalidKey)
}

#[cfg(test)]
pub(crate) async fn test_fixture() -> (super::ClientManager, bitwarden_core::UserId, AgentIdentity)
{
    let uid = bitwarden_core::UserId::new_v4();
    let manager = super::test_support::unlocked_manager(uid);
    manager
        .users
        .get(&uid)
        .unwrap()
        .session
        .client()
        .unwrap()
        .0
        .internal
        .init_user_id(uid)
        .await
        .unwrap();
    let key =
        bitwarden_ssh::generator::generate_sshkey(bitwarden_ssh::generator::KeyAlgorithm::Ed25519)
            .unwrap();
    let now = chrono::Utc::now();
    let cipher: bitwarden_vault::CipherView = serde_json::from_value(serde_json::json!({
        "name":"Synthetic SSH key", "type":5,"collectionIds":[],"partial":false,"favorite":false,
        "reprompt":0,"organizationUseTotp":false,"edit":true,"viewPassword":true,
        "creationDate":now,"revisionDate":now,
        "sshKey":{"privateKey":key.private_key,"publicKey":key.public_key,"fingerprint":key.fingerprint}
    })).unwrap();
    manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(cipher)
        .await
        .unwrap();
    let identity = manager
        .agent_session_for(&uid)
        .unwrap()
        .identities()
        .await
        .unwrap()
        .remove(0);
    (manager, uid, identity)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats_and_signature_algorithms_interoperate() {
        use bitwarden_ssh::generator::{KeyAlgorithm, generate_sshkey};
        for (algorithm, flags) in [
            (KeyAlgorithm::Ed25519, 0),
            (KeyAlgorithm::EcdsaP256, 0),
            (KeyAlgorithm::EcdsaP384, 0),
            (KeyAlgorithm::EcdsaP521, 0),
            (KeyAlgorithm::Rsa3072, 2),
        ] {
            let key = generate_sshkey(algorithm).unwrap();
            let public = PublicKey::from_openssh(&key.public_key).unwrap();
            let blob = public.to_bytes().unwrap();
            let data = b"synthetic signing test";
            let signature = Signature::try_from(
                sign_private(&key.private_key, &blob, data, flags)
                    .unwrap()
                    .as_slice(),
            )
            .unwrap();
            public.key_data().verify(data, &signature).unwrap();
            assert!(public.key_data().verify(b"tampered", &signature).is_err());
            assert!(sign_private(&key.private_key, b"wrong public key", data, flags).is_err());
            if flags == 2 {
                assert!(sign_private(&key.private_key, &blob, data, 0).is_err());
                let signature = Signature::try_from(
                    sign_private(&key.private_key, &blob, data, 4)
                        .unwrap()
                        .as_slice(),
                )
                .unwrap();
                public.key_data().verify(data, &signature).unwrap();
            }
        }
    }
    #[test]
    fn rsa_pkcs1_and_pkcs8_pem_imports_produce_verifiable_signatures() {
        let rsa = openssl::rsa::Rsa::generate(2048).unwrap();
        let pkcs1 = Zeroizing::new(String::from_utf8(rsa.private_key_to_pem().unwrap()).unwrap());
        let pkey = openssl::pkey::PKey::from_rsa(rsa).unwrap();
        let pkcs8 =
            Zeroizing::new(String::from_utf8(pkey.private_key_to_pem_pkcs8().unwrap()).unwrap());
        let normalized = bitwarden_ssh::import::import_key(pkcs8.to_string(), None).unwrap();
        let public = PublicKey::from_openssh(&normalized.public_key).unwrap();
        let blob = public.to_bytes().unwrap();
        for pem in [&pkcs1, &pkcs8] {
            let signed = sign_private(pem, &blob, b"PEM interoperability", 4).unwrap();
            public
                .key_data()
                .verify(
                    b"PEM interoperability",
                    &Signature::try_from(signed.as_slice()).unwrap(),
                )
                .unwrap();
        }
        let encrypted = pkey
            .private_key_to_pem_pkcs8_passphrase(
                openssl::symm::Cipher::aes_256_cbc(),
                b"synthetic-passphrase",
            )
            .unwrap();
        assert!(sign_private(std::str::from_utf8(&encrypted).unwrap(), &blob, b"test", 4).is_err());
    }
    #[tokio::test]
    async fn sdk_permissions_revision_and_lock_revoke_signing() {
        let (mut manager, uid, identity) = test_fixture().await;
        let session = manager.agent_session_for(&uid).unwrap();
        let signature = session
            .sign(identity.clone(), b"test".to_vec(), 0)
            .await
            .unwrap();
        PublicKey::from_bytes(&identity.public_key)
            .unwrap()
            .key_data()
            .verify(b"test", &Signature::try_from(signature.as_slice()).unwrap())
            .unwrap();
        let mut stale = identity.clone();
        stale.revision += chrono::Duration::seconds(1);
        assert!(session.sign(stale, b"test".to_vec(), 0).await.is_err());
        let mut view = manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(identity.id)
            .await
            .unwrap();
        view.reprompt = CipherRepromptType::Password;
        manager
            .client_for(&uid)
            .unwrap()
            .save_cipher(view)
            .await
            .unwrap();
        assert!(session.identities().await.unwrap().is_empty());
        assert!(
            session
                .sign(identity.clone(), b"test".to_vec(), 0)
                .await
                .is_err()
        );
        let pending = manager.lock(&uid).unwrap();
        assert!(!session.valid());
        assert!(session.sign(identity, b"test".to_vec(), 0).await.is_err());
        pending.finish().await;
    }
    #[tokio::test]
    async fn hidden_archived_deleted_and_malformed_items_do_not_poison_valid_keys() {
        let (manager, uid, identity) = test_fixture().await;
        let session = manager.agent_session_for(&uid).unwrap();
        let original = manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(identity.id)
            .await
            .unwrap();
        for variant in 0..5 {
            let mut item = original.clone();
            item.id = None;
            match variant {
                0 => item.view_password = false,
                1 => item.archived_date = Some(Utc::now()),
                2 => item.deleted_date = Some(Utc::now()),
                3 => item.ssh_key.as_mut().unwrap().public_key = "malformed public key".into(),
                _ => item.reprompt = CipherRepromptType::Password,
            }
            manager
                .client_for(&uid)
                .unwrap()
                .save_cipher(item)
                .await
                .unwrap();
        }
        let keys = session.identities().await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].id, identity.id);
        // A syntactically valid public key paired with unrelated private material cannot sign.
        let unrelated = bitwarden_ssh::generator::generate_sshkey(
            bitwarden_ssh::generator::KeyAlgorithm::Ed25519,
        )
        .unwrap();
        let mut item = original;
        item.ssh_key.as_mut().unwrap().private_key = unrelated.private_key;
        manager
            .client_for(&uid)
            .unwrap()
            .save_cipher(item)
            .await
            .unwrap();
        let keys = session.identities().await.unwrap();
        assert_eq!(keys.len(), 1);
        assert!(
            session
                .sign(keys[0].clone(), b"test".to_vec(), 0)
                .await
                .is_err()
        );
    }
    #[test]
    fn ui_labels_remove_controls_and_bidi() {
        assert_eq!(clean_label("key\n\u{202e}name", 256), "keyname");
        assert_eq!(clean_label("éé", 3), "é");
    }
}
