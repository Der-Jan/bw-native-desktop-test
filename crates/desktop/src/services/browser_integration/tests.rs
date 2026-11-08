use super::*;
async fn approval(receiver: &mut broadcast::Receiver<Event>) -> Arc<Approval> {
    loop {
        if let Event::Approval(approval) =
            tokio::time::timeout(Duration::from_secs(2), receiver.recv())
                .await
                .unwrap()
                .unwrap()
        {
            return approval;
        }
    }
}
#[tokio::test]
async fn key_release_is_explicit_account_bound_and_once_means_once() {
    let (runtime, _manager, connection) = test_fixture();
    let mut events = runtime.events().subscribe();
    assert!(
        connection
            .key(bitwarden_core::UserId::new_v4())
            .await
            .is_none()
    );
    let c = connection.clone();
    let pending = tokio::spawn(async move { c.key(c.session.server_user_id).await });
    let prompt = approval(&mut events).await;
    assert!(!pending.is_finished());
    assert!(prompt.current());
    prompt.respond(Decision::Deny);
    assert!(pending.await.unwrap().is_none());
    for _ in 0..2 {
        let c = connection.clone();
        let pending = tokio::spawn(async move { c.key(c.session.server_user_id).await });
        let prompt = approval(&mut events).await;
        prompt.respond(Decision::Once);
        assert!(pending.await.unwrap().is_some());
    }
}
#[tokio::test]
async fn until_lock_grants_do_not_survive_lock_or_account_incarnation_change() {
    let (runtime, mut manager, connection) = test_fixture();
    let mut events = runtime.events().subscribe();
    let c = connection.clone();
    let pending = tokio::spawn(async move { c.key(c.session.server_user_id).await });
    approval(&mut events).await.respond(Decision::UntilLock);
    assert!(pending.await.unwrap().is_some());
    assert!(
        connection
            .key(connection.session.server_user_id)
            .await
            .is_some()
    );
    manager
        .lock(&connection.session.uid)
        .unwrap()
        .finish()
        .await;
    assert!(!connection.current());
    assert!(
        connection
            .key(connection.session.server_user_id)
            .await
            .is_none()
    );
}
#[tokio::test]
async fn pending_approval_is_denied_on_switch_disconnect_and_disable() {
    for event in 0..3 {
        let (mut runtime, _manager, connection) = test_fixture();
        let mut events = runtime.events().subscribe();
        let c = connection.clone();
        let pending = tokio::spawn(async move { c.key(c.session.server_user_id).await });
        let prompt = approval(&mut events).await;
        match event {
            0 => runtime.set_session(None),
            1 => connection.alive.store(false, Ordering::SeqCst),
            _ => runtime.configure(false),
        }
        assert!(!prompt.current());
        prompt.respond(Decision::Once);
        assert!(pending.await.unwrap().is_none());
    }
}
#[tokio::test]
async fn frames_are_bounded_and_truncation_is_not_silently_accepted() {
    use tokio::io::AsyncWriteExt;
    let (mut a, mut b) = tokio::io::duplex(64);
    let writer = tokio::spawn(async move {
        wire::write_frame(&mut a, b"synthetic-frame").await.unwrap();
    });
    assert_eq!(
        wire::read_frame(&mut b).await.unwrap().unwrap(),
        b"synthetic-frame"
    );
    writer.await.unwrap();
    for length in [0, wire::MAX_FRAME as u32 + 1] {
        let (mut a, mut b) = tokio::io::duplex(8);
        a.write_all(&length.to_ne_bytes()).await.unwrap();
        assert!(wire::read_frame(&mut b).await.is_err());
    }
    let (mut a, mut b) = tokio::io::duplex(8);
    a.write_all(&[1, 0]).await.unwrap();
    drop(a);
    assert!(wire::read_frame(&mut b).await.is_err());
}
#[test]
fn native_host_origins_require_the_exact_official_extension() {
    assert!(
        wire::extension_origin(&[format!("chrome-extension://{}/", wire::CHROMIUM_IDS[0])])
            .is_some()
    );
    for origin in [
        "https://example.test",
        "chrome-extension://nngceckbapebfimnlniiiahkandclblb.evil/",
        "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/",
    ] {
        assert!(wire::extension_origin(&[origin.into()]).is_none());
    }
    assert!(
        wire::extension_origin(&[
            "/tmp/com.8bit.bitwarden.json".into(),
            wire::FIREFOX_ID.into()
        ])
        .is_some()
    );
    assert!(
        wire::extension_origin(&["/tmp/unrelated.json".into(), wire::FIREFOX_ID.into()]).is_none()
    );
}

fn reconnect(runtime: &BrowserRuntime, previous: &Connection) -> Arc<Connection> {
    let state = runtime.shared.state.lock().unwrap();
    Arc::new(Connection {
        shared: runtime.shared.clone(),
        session: state.session.clone().unwrap(),
        epoch: state.epoch,
        browser: previous.browser.clone(),
        browser_executable: previous.browser_executable.clone(),
        origin: previous.origin.clone(),
        alive: AtomicBool::new(true),
        remembered: AtomicBool::new(false),
    })
}

#[tokio::test]
async fn permanent_pairing_survives_lock_restart_and_reconnect_and_can_be_revoked() {
    let directory = TestDirectory::new();
    let path = directory.path().join("trusted-browsers.json");
    let (runtime, mut manager, connection) = test_fixture();
    *runtime.shared.trust.lock().unwrap() = trust::Store::load(Some(path.clone()));
    let mut events = runtime.events().subscribe();
    let c = connection.clone();
    let pending = tokio::spawn(async move { c.key(c.session.server_user_id).await });
    approval(&mut events).await.respond(Decision::Always);
    assert!(pending.await.unwrap().is_some());
    assert_eq!(runtime.permanent_approvals(), 1);
    assert!(
        reconnect(&runtime, &connection)
            .key(connection.session.server_user_id)
            .await
            .is_some()
    );
    manager
        .lock(&connection.session.uid)
        .unwrap()
        .finish()
        .await;
    assert!(
        connection
            .key(connection.session.server_user_id)
            .await
            .is_none()
    );

    // Recreate both runtime and unlocked SDK authority for the same account,
    // as on restart/re-unlock. A saved grant must not retain an old session key.
    let new_manager = crate::services::sdk::test_support::unlocked_manager(connection.session.uid);
    let new_runtime = BrowserRuntime::with_trust_path(Some(path.clone()));
    new_runtime.set_session(new_manager.browser_session_for(&connection.session.uid));
    new_runtime.shared.state.lock().unwrap().enabled = true;
    let fresh = reconnect(&new_runtime, &connection);
    assert!(fresh.key(fresh.session.server_user_id).await.is_some());
    assert!(fresh.key(bitwarden_core::UserId::new_v4()).await.is_none());

    // Every identity component is part of the permanent permission.
    for part in 0..5 {
        let mut other = reconnect(&new_runtime, &fresh);
        let other = Arc::get_mut(&mut other).unwrap();
        match part {
            0 => other.browser_executable = PathBuf::from("/other/helium"),
            1 => other.origin = format!("chrome-extension://{}", wire::CHROMIUM_IDS[1]),
            2 => other.session.server = "https://other.invalid".into(),
            3 => other.session.server_user_id = bitwarden_core::UserId::new_v4(),
            _ => other.session.uid = bitwarden_core::UserId::new_v4(),
        }
        assert!(!new_runtime.shared.trust.lock().unwrap().contains(other));
    }
    new_runtime.revoke_permanent().await.unwrap();
    assert!(!fresh.current());
    assert_eq!(new_runtime.permanent_approvals(), 0);
    assert_eq!(trust::Store::load(Some(path)).len(), 0);
    let mut events = new_runtime.events().subscribe();
    let fresh = reconnect(&new_runtime, &fresh);
    let c = fresh.clone();
    let pending = tokio::spawn(async move { c.key(c.session.server_user_id).await });
    approval(&mut events).await.respond(Decision::Deny);
    assert!(pending.await.unwrap().is_none());
}

#[tokio::test]
async fn permanent_approval_fails_closed_on_storage_failure_or_revoked_prompt() {
    for revoke in [false, true] {
        let (runtime, _manager, connection) = test_fixture();
        let directory = TestDirectory::new();
        let blocker = directory.path().join("file");
        std::fs::write(&blocker, b"not a directory").unwrap();
        if !revoke {
            *runtime.shared.trust.lock().unwrap() =
                trust::Store::load(Some(blocker.join("trust.json")));
        }
        let mut events = runtime.events().subscribe();
        let c = connection.clone();
        let pending = tokio::spawn(async move { c.key(c.session.server_user_id).await });
        let prompt = approval(&mut events).await;
        if revoke {
            runtime.revoke_permanent().await.unwrap();
        }
        prompt.respond(Decision::Always);
        assert!(pending.await.unwrap().is_none());
        assert_eq!(runtime.permanent_approvals(), 0);
    }
}

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("bw-browser-trust-{}", Uuid::new_v4()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
