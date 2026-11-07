//! Full read sync, bounded HTTP, account refresh, atomic encrypted SDK-SQLite snapshot.
use super::{
    SessionClient, SessionToken, refresh,
    snapshot::{self, OrgRollback, Snapshot},
};
use crate::services::accounts::{
    login::AccountRecord, profile::ProfileStore, transport::json_body_limited,
};
use bitwarden_vault::{Cipher, CipherListView};
use chrono::{DateTime, Utc};
use std::{path::PathBuf, sync::atomic::Ordering};
#[derive(Clone, Debug)]
pub struct Report {
    pub session: SessionToken,
    pub downloaded_at: DateTime<Utc>,
    pub ciphers: usize,
}
pub async fn synchronize(
    client: SessionClient,
    root: PathBuf,
    store: ProfileStore,
) -> Result<Report, String> {
    let session = client.session();
    let sequence = session.state.sync_sequence.load(Ordering::SeqCst);
    let _coordinator = session.state.sync_gate.lock().await;
    if session.state.sync_sequence.load(Ordering::SeqCst) != sequence {
        let current = session.state.token();
        if current.generation == session.generation + 1 && current.ensure_unlocked().is_ok() {
            return report(current);
        }
    }
    session.ensure_unlocked()?;
    let fetch_session = session.clone();
    let (snapshot,record,transport)=client.run(move |_|async move {
        let session=fetch_session;
        let authority=refresh::authorized(&session,root.clone(),store.clone(),false).await?;
        let mut record=authority.record;let mut credentials=authority.credentials;let mut transport=authority.transport;
        let started=Utc::now();
        let url=url::Url::parse(&record.api).and_then(|u|u.join("sync?excludeDomains=true")).map_err(|_|"Ungültiger Sync-Endpunkt")?;
        let mut response=transport.execute(transport.request(reqwest_account::Method::GET,url.clone())?.bearer_auth(&credentials.access_token).build().map_err(|_|"Sync-Anfrage konnte nicht erstellt werden")?).await?;
        if response.status().as_u16()==401 {
            let renewed=refresh::authorized(&session,root,store,true).await?;
            record=renewed.record;credentials=renewed.credentials;transport=renewed.transport;
            response=transport.execute(transport.request(reqwest_account::Method::GET,url)?.bearer_auth(&credentials.access_token).build().map_err(|_|"Sync-Anfrage konnte nicht erstellt werden")?).await?;
        }
        if matches!(response.status().as_u16(),401|403){
            if response.status().as_u16()==401 {session.state.network_authorized.store(false,Ordering::SeqCst);}
            return Err("Die Anmeldung ist abgelaufen oder der Server verweigert den Zugriff. Bitte erneut anmelden.".into())}
        if !response.status().is_success(){return Err("Der Server konnte den Tresor nicht synchronisieren. Der bisherige Snapshot bleibt erhalten.".into())}
        let data=json_body_limited(response,64*1024*1024).await?;
        let expected=record.server_user_id;let local_account=record.id;
        let snapshot=tokio::task::spawn_blocking(move||{
            let response=serde_json::from_value(data).map_err(|_|"Die Sync-Antwort ist ungültig oder enthält nicht unterstützte Datentypen")?;
            Snapshot::prepare(response,expected,local_account,started)
        }).await.map_err(|_|"Sync-Antwort konnte nicht verarbeitet werden")??;
        Ok((snapshot,record,transport))
    }).await?;
    let _exclusive = session.state.gate.write().await;
    session.ensure_unlocked()?;
    let client = session.state.client()?;
    transport.check(&transport.profile.default_api()?)?;
    if let Err(error) = validate_account_keys(&snapshot, &record) {
        session
            .state
            .network_authorized
            .store(false, Ordering::SeqCst);
        return Err(error);
    }
    let old = session.state.snapshot.current();
    if let (Some(before), Some(after)) = (
        old.as_ref()
            .and_then(|s| s.persisted.response.profile.as_ref())
            .and_then(|p| p.security_stamp.as_ref()),
        snapshot
            .persisted
            .response
            .profile
            .as_ref()
            .and_then(|p| p.security_stamp.as_ref()),
    ) && before != after
    {
        session
            .state
            .network_authorized
            .store(false, Ordering::SeqCst);
        return Err(
            "Die Kontosicherheit wurde auf dem Server geändert. Bitte erneut anmelden.".into(),
        );
    }
    let mut rollback = OrgRollback::new(
        &client,
        session.clone(),
        old.as_ref()
            .map(|s| s.organizations.clone())
            .unwrap_or_default(),
    );
    snapshot::org_keys(&client, &snapshot.organizations)?;
    // Validate every list row; the SDK's partial-success list API must not make a
    // malformed/incompletely decryptable response look like an empty remote vault.
    let ciphers: Vec<Cipher> = snapshot.ciphers.values().cloned().collect();
    let _:Vec<CipherListView>=client.0.internal.get_key_store().decrypt_list(&ciphers).map_err(|_|"Einträge konnten nicht vollständig entschlüsselt werden. Der bisherige Snapshot bleibt erhalten.")?;
    {
        use bitwarden_crypto::Decryptable;
        let keys = client.0.internal.get_key_store();
        let mut context = keys.context_mut();
        for folder in snapshot.folders.values() {
            let _: String = folder
                .name
                .decrypt(
                    &mut context,
                    bitwarden_core::key_management::SymmetricKeySlotId::User,
                )
                .map_err(|_| "Ordner konnten nicht entschlüsselt werden")?;
        }
    }
    let names = snapshot.collection_names(&client)?;
    session.ensure_unlocked()?;
    transport.check(&transport.profile.default_api()?)?;
    // Commit authorization linearizes here. An already authorized encrypted SQLite
    // transaction may finish while lock/logout drains this lease; no plaintext or
    // new session authority is published after revocation. Logout then removes it.
    client.platform().state().setting(snapshot::CACHE).map_err(|_|"Snapshot-Speicher fehlt")?.update(snapshot.persisted.clone()).await.map_err(|_|"Der neue Snapshot konnte nicht gespeichert werden. Der bisherige Snapshot bleibt erhalten.")?;
    session.state.snapshot.install(snapshot);
    let new_session = session.advance_snapshot(names)?;
    rollback.committed = true;
    report(new_session)
}
fn validate_account_keys(snapshot: &Snapshot, record: &AccountRecord) -> Result<(), String> {
    let profile = snapshot
        .persisted
        .response
        .profile
        .as_ref()
        .ok_or("Kontoprofil fehlt")?;
    if profile
        .email
        .as_ref()
        .is_none_or(|email| !email.eq_ignore_ascii_case(&record.email))
    {
        return Err("Die Konto-E-Mail wurde geändert. Bitte erneut anmelden.".into());
    }
    if profile
        .key
        .as_ref()
        .is_some_and(|key| key != &record.master_unlock.master_key_wrapped_user_key.to_string())
    {
        return Err("Die Passwort-/Kontoschlüssel wurden geändert. Bitte erneut anmelden.".into());
    }
    if let Some(model) = &profile.account_keys {
        let received=bitwarden_core::key_management::account_cryptographic_state::WrappedAccountCryptographicState::try_from(model.as_ref()).map_err(|_|"Ungültige Kontoschlüssel in der Sync-Antwort")?;
        if serde_json::to_value(&received)
            .map_err(|_| "Kontoschlüssel konnten nicht geprüft werden")?
            != serde_json::to_value(&record.crypto_state)
                .map_err(|_| "Lokale Kontoschlüssel konnten nicht geprüft werden")?
        {
            return Err(
                "Der kryptografische Kontozustand wurde geändert. Bitte erneut anmelden.".into(),
            );
        }
    }
    if let Some(model) = snapshot
        .persisted
        .response
        .user_decryption
        .as_ref()
        .and_then(|u| u.master_password_unlock.as_ref())
    {
        let received =
            bitwarden_core::key_management::MasterPasswordUnlockData::try_from(model.as_ref())
                .map_err(|_| "Ungültige Passwort-Entsperrdaten in der Sync-Antwort")?;
        if serde_json::to_value(&received)
            .map_err(|_| "Entsperrdaten konnten nicht geprüft werden")?
            != serde_json::to_value(&record.master_unlock)
                .map_err(|_| "Lokale Entsperrdaten konnten nicht geprüft werden")?
        {
            return Err(
                "Die Passwort-/KDF-Einstellungen wurden geändert. Bitte erneut anmelden.".into(),
            );
        }
    }
    if let bitwarden_core::key_management::account_cryptographic_state::WrappedAccountCryptographicState::V1{private_key}=&record.crypto_state && profile.private_key.as_ref().is_some_and(|key|key!=&private_key.to_string()){return Err("Der private Kontoschlüssel wurde geändert. Bitte erneut anmelden.".into())}
    Ok(())
}
fn report(session: SessionToken) -> Result<Report, String> {
    let snapshot = session
        .state
        .snapshot
        .current()
        .ok_or("Noch kein synchronisierter Tresor vorhanden")?;
    Ok(Report {
        session,
        downloaded_at: snapshot.persisted.downloaded_at,
        ciphers: snapshot.ciphers.len(),
    })
}
