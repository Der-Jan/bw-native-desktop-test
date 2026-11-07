//! Offline compatibility checks against the pinned SDK's real crypto and repositories.

use bitwarden_crypto::Kdf;
use bitwarden_exporters::ExportFormat;
use bitwarden_vault::CipherView;

use super::{ClientExt, test_support::unlocked_manager};

#[tokio::test]
async fn password_unlock_crud_export_and_reunlock_use_real_sdk_state() {
    let uid = "00000000-0000-0000-0000-000000000041".parse().unwrap();
    let mut manager = unlocked_manager(uid);
    manager.lock(&uid).unwrap().finish().await;
    let raw = manager.session_for(&uid).unwrap().state.client().unwrap();
    let email = "sdk-upgrade-test@example.invalid";
    let password = "synthetic-sdk-upgrade-password";
    let kdf = Kdf::default_pbkdf2();
    let registration = raw
        .0
        .auth()
        .make_register_keys(email.into(), password.into(), kdf.clone())
        .unwrap();
    let entry = manager.users.get_mut(&uid).unwrap();
    entry.email = email.into();
    entry.master_unlock = bitwarden_core::key_management::MasterPasswordUnlockData {
        kdf,
        salt: email.into(),
        master_key_wrapped_user_key: registration.encrypted_user_key,
        contained_key_id: None,
    };
    entry.crypto_state = bitwarden_core::key_management::account_cryptographic_state::WrappedAccountCryptographicState::V1 {private_key:registration.keys.private};

    manager
        .unlock_data_for(&uid)
        .unwrap()
        .unlock(password.into())
        .await
        .unwrap();
    assert!(manager.is_unlocked(&uid));
    let (validation, envelope) = manager.validation_data_for(&uid).unwrap();
    assert!(
        validation
            .validate_master_password(envelope, "wrong synthetic password".into())
            .await
            .is_err()
    );
    let (validation, envelope) = manager.validation_data_for(&uid).unwrap();
    validation
        .validate_master_password(envelope, password.into())
        .await
        .unwrap();

    let folder = manager
        .client_for(&uid)
        .unwrap()
        .create_folder("Synthetic folder".into())
        .await
        .unwrap();
    assert_eq!(
        manager
            .client_for(&uid)
            .unwrap()
            .list_folders()
            .await
            .unwrap()
            .len(),
        1
    );
    let stamp = chrono::Utc::now();
    let cipher: CipherView = serde_json::from_value(serde_json::json!({
        "name": "SDK upgrade roundtrip",
        "type": 1,
        "folderId": folder.id,
        "collectionIds": [],
        "partial": false,
        "favorite": false,
        "reprompt": 0,
        "organizationUseTotp": false,
        "edit": true,
        "viewPassword": true,
        "creationDate": stamp,
        "revisionDate": stamp,
        "login": { "password": "synthetic vault secret" }
    }))
    .unwrap();
    let saved = manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(cipher)
        .await
        .unwrap();
    let id = saved.id.unwrap();
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
    let (validation, envelope) = manager.validation_data_for(&uid).unwrap();
    validation
        .authorize_export(envelope, password.into())
        .await
        .unwrap();
    let export = manager
        .client_for(&uid)
        .unwrap()
        .export_vault(ExportFormat::Json)
        .await
        .unwrap();
    let exported: serde_json::Value = serde_json::from_str(&export).unwrap();
    assert_eq!(
        exported["items"][0]["login"]["password"],
        "synthetic vault secret"
    );

    let stale = manager.client_for(&uid).unwrap();
    manager.lock(&uid).unwrap().finish().await;
    assert!(!raw.is_unlocked());
    assert!(stale.full_cipher(id).await.is_err());
    manager
        .unlock_data_for(&uid)
        .unwrap()
        .unlock(password.into())
        .await
        .unwrap();
    let loaded = manager
        .client_for(&uid)
        .unwrap()
        .full_cipher(id)
        .await
        .unwrap();
    assert_eq!(
        loaded.login.unwrap().password.as_deref(),
        Some("synthetic vault secret")
    );
    manager
        .client_for(&uid)
        .unwrap()
        .soft_delete_cipher(id)
        .await
        .unwrap();
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .unwrap()
            .deleted_date
            .is_some()
    );
}

#[tokio::test]
async fn protected_items_require_fresh_action_grants_and_enforce_permissions() {
    use super::{Action, Authorization};
    let uid = "00000000-0000-0000-0000-000000000043".parse().unwrap();
    let mut manager = unlocked_manager(uid);
    manager.lock(&uid).unwrap().finish().await;
    let raw = manager.session_for(&uid).unwrap().state.client().unwrap();
    let email = "authorization@example.invalid";
    let password = "synthetic-test-password";
    let kdf = Kdf::default_pbkdf2();
    let registration = raw
        .0
        .auth()
        .make_register_keys(email.into(), password.into(), kdf.clone())
        .unwrap();
    let entry = manager.users.get_mut(&uid).unwrap();
    entry.email = email.into();
    entry.master_unlock = bitwarden_core::key_management::MasterPasswordUnlockData {
        kdf,
        salt: email.into(),
        contained_key_id: None,
        master_key_wrapped_user_key: registration.encrypted_user_key,
    };
    entry.crypto_state=bitwarden_core::key_management::account_cryptographic_state::WrappedAccountCryptographicState::V1{private_key:registration.keys.private};
    manager
        .unlock_data_for(&uid)
        .unwrap()
        .unlock(password.into())
        .await
        .unwrap();
    let now = chrono::Utc::now();
    let cipher:CipherView=serde_json::from_value(serde_json::json!({"name":"Protected","type":1,"collectionIds":[],"partial":false,"favorite":false,"reprompt":1,"organizationUseTotp":false,"edit":true,"viewPassword":true,"creationDate":now,"revisionDate":now,"login":{"password":"protected secret"}})).unwrap();
    let saved = manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(cipher)
        .await
        .unwrap();
    let id = saved.id.unwrap();
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .is_err()
    );
    let target = Authorization {
        cipher: Some(id),
        revision: Some(saved.revision_date),
        action: Action::Read,
    };
    let (c, envelope) = manager.validation_data_for(&uid).unwrap();
    c.authorize_password(envelope, password.into(), target.clone())
        .await
        .unwrap();
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .is_ok()
    );
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .full_cipher(id)
            .await
            .is_err()
    );
    let mut modified = saved.clone();
    modified.reprompt = bitwarden_vault::CipherRepromptType::None;
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .save_cipher(modified.clone())
            .await
            .is_err(),
        "Removing the reprompt flag cannot bypass the stored policy"
    );
    let (c, envelope) = manager.validation_data_for(&uid).unwrap();
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    assert!(
        c.authorize_password_scoped(envelope, password.into(), target.clone(), cancelled)
            .await
            .is_err()
    );
    assert!(!manager.permitted(&uid, &target, false));
    let mut write = target.clone();
    write.action = Action::Write;
    let (c, envelope) = manager.validation_data_for(&uid).unwrap();
    c.authorize_password(envelope, password.into(), write)
        .await
        .unwrap();
    modified.edit = false;
    modified.view_password = false;
    manager
        .client_for(&uid)
        .unwrap()
        .save_cipher(modified.clone())
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
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .save_cipher(modified)
            .await
            .is_err()
    );
    assert!(
        manager
            .client_for(&uid)
            .unwrap()
            .soft_delete_cipher(id)
            .await
            .is_err()
    );
}
