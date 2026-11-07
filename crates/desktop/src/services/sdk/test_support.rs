use super::*;
use bitwarden_core::key_management::SymmetricKeySlotId;
use bitwarden_crypto::{EncString, Kdf, SymmetricCryptoKey};
use bitwarden_pm::PasswordManagerClient;

pub fn raw_client() -> PasswordManagerClient {
    PasswordManagerClient::new(None)
}

#[allow(deprecated)] // Tests install a synthetic key directly to exercise lifecycle cleanup.
pub fn install_user_key(client: &PasswordManagerClient) {
    client
        .0
        .internal
        .get_key_store()
        .context_mut()
        .set_symmetric_key(
            SymmetricKeySlotId::User,
            SymmetricCryptoKey::make(bitwarden_crypto::SymmetricKeyAlgorithm::Aes256CbcHmac),
        )
        .unwrap();
}

/// Synthetic account with real SDK keystore state; no disk or network access.
pub fn unlocked_manager(uid: UserId) -> ClientManager {
    let client = raw_client();
    install_user_key(&client);
    let session = session::SessionState::new(uid, client);
    session.begin_unlock().unwrap().finish_unlock(true).unwrap();
    let envelope: EncString = "2.AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".parse().unwrap();
    let mut manager = ClientManager::empty();
    manager.users.insert(
        uid,
        Box::new(UserEntry {
            session,
            email: "test@example.invalid".into(),
            display_name: "Test".into(),
            server_url: "https://example.invalid".into(),
            unlock_methods: UnlockMethods {
                master_password: true,
                pin: false,
                biometrics: false,
            },
            server_user_id:uid,
            master_unlock: bitwarden_core::key_management::MasterPasswordUnlockData {kdf:Kdf::default_pbkdf2(),salt:"test@example.invalid".into(),master_key_wrapped_user_key:envelope.clone(),contained_key_id:None},
            crypto_state: bitwarden_core::key_management::account_cryptographic_state::WrappedAccountCryptographicState::V1{private_key:envelope},
            profile:None,
            organizations: vec![],
            collections: vec![],
        }),
    );
    manager
}

/// Isolated profile storage for login/update tests; never access an OS keyring.
pub fn manager_with_profiles(
    profiles: crate::services::accounts::profile::ProfileStore,
) -> ClientManager {
    let mut manager = ClientManager::empty();
    manager.profiles = Some(profiles);
    manager
}

/// Generate actual encrypted API data using the unlocked account's SDK crypto.
pub async fn encrypted_sync_response(
    manager: &ClientManager,
    uid: UserId,
    name: &str,
) -> serde_json::Value {
    use bitwarden_vault::{CipherId, CipherView, FolderId, FolderView};
    let client = manager.users.get(&uid).unwrap().session.client().unwrap();
    let entry = manager.users.get(&uid).unwrap();
    let stamp = chrono::Utc::now();
    let folder = client
        .0
        .internal
        .get_key_store()
        .encrypt(FolderView {
            id: Some(FolderId::new_v4()),
            name: "Synthetic encrypted folder".into(),
            revision_date: stamp,
        })
        .unwrap();
    let cipher:CipherView=serde_json::from_value(serde_json::json!({"id":CipherId::new_v4(),"name":name,"type":1,"collectionIds":[],"folderId":folder.id,"partial":false,"favorite":false,"reprompt":0,"organizationUseTotp":false,"edit":true,"viewPassword":true,"creationDate":stamp,"revisionDate":stamp,"login":{"username":"synthetic username","password":"synthetic sync vault secret"}})).unwrap();
    let encrypted = client
        .vault()
        .ciphers()
        .encrypt(cipher)
        .await
        .unwrap()
        .cipher;
    serde_json::json!({"object":"sync","profile":{"id":entry.server_user_id,"email":entry.email,"name":entry.display_name,"key":entry.master_unlock.master_key_wrapped_user_key.to_string(),"organizations":[],"securityStamp":"synthetic-stamp"},"ciphers":[encrypted],"folders":[folder],"collections":[],"policies":[],"sends":[]})
}

#[allow(deprecated)] // Synthetic fixtures use the SDK's legacy RSA wrapping supported by Vaultwarden.
pub async fn encrypted_org_sync_response(
    manager: &ClientManager,
    uid: UserId,
) -> (
    serde_json::Value,
    bitwarden_core::OrganizationId,
    bitwarden_vault::CipherId,
) {
    use bitwarden_core::key_management::{PrivateKeySlotId, SymmetricKeySlotId};
    use bitwarden_crypto::{SymmetricCryptoKey, SymmetricKeyAlgorithm, UnsignedSharedKey};
    use bitwarden_vault::{CipherId, CipherView};
    let entry = manager.users.get(&uid).unwrap();
    let client = entry.session.client().unwrap();
    let store = client.0.internal.get_key_store();
    let org_id = bitwarden_core::OrganizationId::new_v4();
    let cipher_id = CipherId::new_v4();
    let collection_id = bitwarden_collections::collection::CollectionId::new_v4();
    let public = store
        .context()
        .get_public_key(PrivateKeySlotId::UserPrivateKey)
        .unwrap();
    let org_key = SymmetricCryptoKey::make(SymmetricKeyAlgorithm::Aes256CbcHmac);
    let wrapped = UnsignedSharedKey::encapsulate_key_unsigned(&org_key, &public).unwrap();
    store
        .context_mut()
        .set_symmetric_key(SymmetricKeySlotId::Organization(org_id), org_key)
        .unwrap();
    let collection:bitwarden_collections::collection::CollectionView=serde_json::from_value(serde_json::json!({"id":collection_id,"organizationId":org_id,"name":"Synthetic secret collection","type":0,"hidePasswords":false,"readOnly":true,"manage":false})).unwrap();
    let collection = client.collections().encrypt(collection).unwrap();
    let now = chrono::Utc::now();
    let view:CipherView=serde_json::from_value(serde_json::json!({"id":cipher_id,"organizationId":org_id,"collectionIds":[collection_id],"name":"Synthetic organization login","type":1,"partial":false,"favorite":false,"reprompt":0,"organizationUseTotp":false,"edit":false,"viewPassword":true,"creationDate":now,"revisionDate":now,"login":{"password":"synthetic organization secret"}})).unwrap();
    let encrypted = client.vault().ciphers().encrypt(view).await.unwrap().cipher;
    let mut collection = serde_json::to_value(collection).unwrap();
    collection.as_object_mut().unwrap().remove("type");
    (
        serde_json::json!({"profile":{"id":entry.server_user_id,"email":entry.email,"key":entry.master_unlock.master_key_wrapped_user_key.to_string(),"organizations":[{"id":org_id,"name":"Synthetic organization","key":wrapped.to_string(),"enabled":true,"status":2}]},"ciphers":[encrypted],"folders":[],"collections":[collection],"policies":[]}),
        org_id,
        cipher_id,
    )
}
pub fn has_org_key(
    manager: &ClientManager,
    uid: UserId,
    org: bitwarden_core::OrganizationId,
) -> bool {
    manager
        .users
        .get(&uid)
        .unwrap()
        .session
        .client()
        .unwrap()
        .0
        .internal
        .get_key_store()
        .context()
        .has_symmetric_key(bitwarden_core::key_management::SymmetricKeySlotId::Organization(org))
}

/// Optional synthetic artifact for the offline GUI acceptance check. Never use
/// this with user data; callers are the disposable local TLS integration tests.
pub fn retain_sync_gui_fixture(source: &std::path::Path) {
    let Some(destination) = std::env::var_os("BW_SYNC_GUI_FIXTURE") else {
        return;
    };
    fn copy_dir(source: &std::path::Path, destination: &std::path::Path) {
        std::fs::create_dir_all(destination).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let target = destination.join(entry.file_name());
            if path.is_dir() {
                copy_dir(&path, &target)
            } else {
                std::fs::copy(path, target).unwrap();
            }
        }
    }
    copy_dir(source, &std::path::PathBuf::from(destination));
}

/// Actual encrypted SSH API item, including Vaultwarden's keyFingerprint field.
pub async fn encrypted_ssh_sync_response(
    manager: &ClientManager,
    uid: UserId,
) -> (serde_json::Value, Vec<u8>) {
    let mut payload = encrypted_sync_response(manager, uid, "Synthetic ordinary login").await;
    let client = manager.users.get(&uid).unwrap().session.client().unwrap();
    let key =
        bitwarden_ssh::generator::generate_sshkey(bitwarden_ssh::generator::KeyAlgorithm::Ed25519)
            .unwrap();
    let public = ssh_key::PublicKey::from_openssh(&key.public_key)
        .unwrap()
        .to_bytes()
        .unwrap();
    let now = chrono::Utc::now();
    let view:bitwarden_vault::CipherView=serde_json::from_value(serde_json::json!({
        "id":bitwarden_vault::CipherId::new_v4(),"name":"Synthetic synced SSH key","type":5,
        "collectionIds":[],"partial":false,"favorite":false,"reprompt":0,"organizationUseTotp":false,"edit":true,"viewPassword":true,
        "creationDate":now,"revisionDate":now,"sshKey":{"privateKey":key.private_key,"publicKey":key.public_key,"fingerprint":key.fingerprint}
    })).unwrap();
    let cipher = client.vault().ciphers().encrypt(view).await.unwrap().cipher;
    let mut wire = serde_json::to_value(cipher).unwrap();
    let ssh = wire["sshKey"].as_object_mut().unwrap();
    let fingerprint = ssh.remove("fingerprint").unwrap();
    ssh.insert("keyFingerprint".into(), fingerprint);
    payload["ciphers"].as_array_mut().unwrap().push(wire);
    (payload, public)
}
