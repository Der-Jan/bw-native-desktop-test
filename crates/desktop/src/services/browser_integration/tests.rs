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
