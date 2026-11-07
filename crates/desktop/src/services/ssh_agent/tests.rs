use super::*;
use protocol::{Reader, parse_sign};
use signature::{Signer, Verifier};
use ssh_key::{PrivateKey, PublicKey, Signature};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn request(key: &[u8], session: &[u8], user: &str) -> Vec<u8> {
    auth_request(key, session, user, None)
}
fn auth_request(key: &[u8], session: &[u8], user: &str, host: Option<&[u8]>) -> Vec<u8> {
    let mut data = Vec::new();
    push_string(&mut data, session);
    data.push(50);
    for s in [
        user.as_bytes(),
        b"ssh-connection",
        if host.is_some() {
            b"publickey-hostbound-v00@openssh.com".as_slice()
        } else {
            b"publickey".as_slice()
        },
    ] {
        push_string(&mut data, s)
    }
    data.push(1);
    push_string(&mut data, b"ssh-ed25519");
    push_string(&mut data, key);
    if let Some(host) = host {
        push_string(&mut data, host)
    }
    let mut packet = vec![13];
    push_string(&mut packet, key);
    push_string(&mut packet, &data);
    packet.extend_from_slice(&0u32.to_be_bytes());
    packet
}

fn host_binding(session: &[u8], forwarded: bool) -> Vec<u8> {
    let key =
        bitwarden_ssh::generator::generate_sshkey(bitwarden_ssh::generator::KeyAlgorithm::Ed25519)
            .unwrap();
    let private = PrivateKey::from_openssh(&key.private_key).unwrap();
    let signature: Signature = private.try_sign(session).unwrap();
    let signature: Vec<u8> = signature.try_into().unwrap();
    let mut packet = vec![27];
    push_string(&mut packet, b"session-bind@openssh.com");
    push_string(&mut packet, &private.public_key().to_bytes().unwrap());
    push_string(&mut packet, session);
    push_string(&mut packet, &signature);
    packet.push(u8::from(forwarded));
    packet
}
#[tokio::test]
async fn authentication_parser_rejects_generic_signatures_and_invalid_binding() {
    let (_, _, identity) = crate::services::sdk::ssh_agent::test_fixture().await;
    let sid = [7; 32];
    let packet = request(&identity.public_key, &sid, "alice");
    let mut bindings = Bindings::default();
    assert!(parse_sign(&packet, &bindings).is_ok());
    for i in 0..packet.len() {
        assert!(parse_sign(&packet[..i], &bindings).is_err());
    }
    assert!(parse_sign(&request(&identity.public_key, &sid, "a\nb"), &bindings).is_err());
    let binding = host_binding(&sid, false);
    assert_eq!(bindings.extension(&binding), vec![6]);
    assert!(
        !parse_sign(&packet, &bindings)
            .unwrap()
            .authentication
            .rememberable
    );
    let bound = auth_request(
        &identity.public_key,
        &sid,
        "alice",
        Some(&bindings.values[0].host),
    );
    assert!(
        parse_sign(&bound, &bindings)
            .unwrap()
            .authentication
            .rememberable
    );
    let wrong_host = auth_request(
        &identity.public_key,
        &sid,
        "alice",
        Some(b"unrelated host key"),
    );
    assert!(parse_sign(&wrong_host, &bindings).is_err());
    assert!(parse_sign(&packet, &bindings).is_ok());
    assert!(parse_sign(&request(&identity.public_key, &[8; 32], "alice"), &bindings).is_err());
    assert_eq!(bindings.extension(&binding), vec![28]);
    assert!(parse_sign(&packet, &bindings).is_err());
    let mut tampered = host_binding(&sid, false);
    let n = tampered.len();
    tampered[n - 2] ^= 1;
    assert_eq!(Bindings::default().extension(&tampered), vec![28]);
    let mut forwarding = Bindings::default();
    assert_eq!(forwarding.extension(&host_binding(&sid, true)), vec![6]);
    assert!(parse_sign(&packet, &forwarding).is_err());
}
#[cfg(unix)]
async fn exchange(stream: &mut tokio::net::UnixStream, packet: &[u8]) -> Vec<u8> {
    stream.write_u32(packet.len() as u32).await.unwrap();
    stream.write_all(packet).await.unwrap();
    let len = stream.read_u32().await.unwrap();
    let mut reply = vec![0; len as usize];
    stream.read_exact(&mut reply).await.unwrap();
    reply
}
#[cfg(unix)]
fn runtime() -> (AgentRuntime, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let base = std::env::temp_dir().join(format!("bw-agent-test-{}", Uuid::new_v4()));
    std::fs::create_dir(&base).unwrap();
    std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut runtime = AgentRuntime::new();
    runtime.path = base.join("agent.sock");
    (runtime, base)
}
#[cfg(unix)]
#[tokio::test]
async fn openssh_listing_approval_signing_and_lock_interoperate() {
    use std::os::unix::fs::PermissionsExt;
    let (mut manager, uid, identity) = crate::services::sdk::ssh_agent::test_fixture().await;
    let (mut agent, base) = runtime();
    let mut events = agent.events().subscribe();
    agent.configure(true);
    agent.set_context(manager.agent_session_for(&uid), SshPromptBehavior::Always);
    assert_eq!(
        std::fs::metadata(agent.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let mut stream = tokio::net::UnixStream::connect(agent.path()).await.unwrap();
    let listing = exchange(&mut stream, &[11]).await;
    let mut reader = Reader::new(&listing);
    assert_eq!(reader.byte().unwrap(), 12);
    assert_eq!(reader.u32().unwrap(), 1);
    assert_eq!(reader.string().unwrap(), identity.public_key);
    let socket = agent.path().to_owned();
    let cli = tokio::task::spawn_blocking(move || {
        std::process::Command::new("ssh-add")
            .arg("-L")
            .env("SSH_AUTH_SOCK", socket)
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    assert!(cli.status.success());
    assert!(
        String::from_utf8(cli.stdout)
            .unwrap()
            .starts_with("ssh-ed25519 ")
    );
    let packet = request(&identity.public_key, &[7; 32], "alice");
    let pending = tokio::spawn(async move {
        let reply = exchange(&mut stream, &packet).await;
        (stream, packet, reply)
    });
    let UiEvent::Approval(approval) = events.recv().await.unwrap() else {
        panic!("approval expected")
    };
    assert!(!approval.can_remember);
    approval.respond(Decision::Once);
    let (mut stream, packet, reply) = pending.await.unwrap();
    let mut reader = Reader::new(&reply);
    assert_eq!(reader.byte().unwrap(), 14);
    let signature = Signature::try_from(reader.string().unwrap()).unwrap();
    PublicKey::from_bytes(&identity.public_key)
        .unwrap()
        .key_data()
        .verify(
            &parse_sign(&packet, &Bindings::default()).unwrap().data,
            &signature,
        )
        .unwrap();
    assert!(matches!(events.recv().await.unwrap(), UiEvent::Dismiss(_)));
    let pending = tokio::spawn(async move { exchange(&mut stream, &packet).await });
    let UiEvent::Approval(approval) = events.recv().await.unwrap() else {
        panic!("approval expected")
    };
    let lock = manager.lock(&uid).unwrap();
    agent.set_context(None, SshPromptBehavior::Always);
    assert!(!approval.current());
    approval.respond(Decision::Once);
    // Revocation closes the connection rather than returning a late signature.
    assert!(pending.await.is_err());
    lock.finish().await;
    let mut locked = tokio::net::UnixStream::connect(agent.path()).await.unwrap();
    assert_eq!(exchange(&mut locked, &[11]).await, vec![12, 0, 0, 0, 0]);
    assert_eq!(exchange(&mut locked, &[17]).await, vec![5]);
    agent.stop();
    assert!(!agent.path().exists());
    std::fs::remove_dir(base).unwrap();
}
#[cfg(unix)]
#[tokio::test]
async fn socket_conflicts_permissions_and_cleanup_fail_closed() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let (mut agent, base) = runtime();
    std::fs::write(agent.path(), "foreign").unwrap();
    agent.configure(true);
    assert!(agent.task.is_none());
    assert_eq!(std::fs::read(agent.path()).unwrap(), b"foreign");
    agent.stop();
    std::fs::remove_file(agent.path()).unwrap();
    symlink(base.join("missing"), agent.path()).unwrap();
    agent.configure(true);
    assert!(agent.task.is_none());
    agent.stop();
    std::fs::remove_file(agent.path()).unwrap();
    std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o755)).unwrap();
    agent.configure(true);
    assert!(agent.task.is_none());
    agent.stop();
    std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
    agent.configure(true);
    assert!(agent.task.is_some());
    std::fs::remove_file(agent.path()).unwrap();
    std::fs::write(agent.path(), "replacement").unwrap();
    agent.stop();
    assert_eq!(std::fs::read(agent.path()).unwrap(), b"replacement");
    std::fs::remove_file(agent.path()).unwrap();
    std::fs::remove_dir(base).unwrap();
}

#[cfg(unix)]
#[tokio::test(start_paused = true)]
async fn frame_limits_and_absolute_read_deadline_close_connections() {
    let (mut agent, base) = runtime();
    agent.configure(true);
    let mut stream = tokio::net::UnixStream::connect(agent.path()).await.unwrap();
    stream.write_u32((MAX_PACKET + 1) as u32).await.unwrap();
    assert_eq!(
        stream.read_u8().await.unwrap_err().kind(),
        std::io::ErrorKind::UnexpectedEof
    );
    let mut stream = tokio::net::UnixStream::connect(agent.path()).await.unwrap();
    stream.write_u32(10).await.unwrap();
    stream.write_u8(11).await.unwrap();
    // Tokio advances virtual time to the absolute frame deadline.
    assert_eq!(
        stream.read_u8().await.unwrap_err().kind(),
        std::io::ErrorKind::UnexpectedEof
    );
    agent.stop();
    std::fs::remove_dir(base).unwrap();
}
#[tokio::test(start_paused = true)]
async fn approval_times_out_and_context_change_clears_cached_grants() {
    let (manager, uid, identity) = crate::services::sdk::ssh_agent::test_fixture().await;
    let mut agent = AgentRuntime::new();
    agent.shared.state.lock().unwrap().enabled = true;
    agent.set_context(
        manager.agent_session_for(&uid),
        SshPromptBehavior::RememberUntilLock,
    );
    let epoch = agent.shared.state.lock().unwrap().epoch;
    let auth = protocol::Authentication {
        username: "alice".into(),
        hosts: vec![[1; 32]],
        fingerprints: vec!["verified test host".into()],
        rememberable: true,
    };
    let mut receiver = agent.events().subscribe();
    let shared = agent.shared.clone();
    let task = tokio::spawn(async move {
        shared
            .approve(
                &identity,
                &auth,
                epoch,
                "test",
                SshPromptBehavior::RememberUntilLock,
            )
            .await
    });
    let UiEvent::Approval(approval) = receiver.recv().await.unwrap() else {
        panic!("approval")
    };
    assert!(approval.can_remember);
    assert!(matches!(task.await.unwrap(), Decision::Deny));
    assert!(matches!(
        receiver.recv().await.unwrap(),
        UiEvent::Dismiss(_)
    ));
    agent.set_context(None, SshPromptBehavior::Always);
    assert!(!approval.current());
    assert!(agent.shared.state.lock().unwrap().grants.is_empty());
}

#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "requires local OpenSSH sshd and /run/sshd"]
async fn real_openssh_authentication_uses_verified_binding() {
    use std::process::{Command, Stdio};
    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let (manager, uid, identity) = crate::services::sdk::ssh_agent::test_fixture().await;
    let (mut agent, base) = runtime();
    agent.configure(true);
    agent.set_context(manager.agent_session_for(&uid), SshPromptBehavior::Always);
    let mut events = agent.events().subscribe();
    let host = base.join("host");
    assert!(
        Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&host)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(
        base.join("authorized_keys"),
        PublicKey::from_bytes(&identity.public_key)
            .unwrap()
            .to_openssh()
            .unwrap(),
    )
    .unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let config = format!(
        "Port {port}\nListenAddress 127.0.0.1\nHostKey {}\nPidFile {}\nAuthorizedKeysFile {}\nStrictModes no\nUsePAM no\nPasswordAuthentication no\nKbdInteractiveAuthentication no\nLogLevel VERBOSE\n",
        host.display(),
        base.join("sshd.pid").display(),
        base.join("authorized_keys").display()
    );
    let config_path = base.join("sshd_config");
    std::fs::write(&config_path, config).unwrap();
    let log = std::fs::File::create(base.join("sshd.log")).unwrap();
    let mut server = ChildGuard(
        Command::new("/usr/bin/sshd")
            .args(["-D", "-e", "-f"])
            .arg(&config_path)
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .unwrap(),
    );
    for _ in 0..100 {
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "{}",
            std::fs::read_to_string(base.join("sshd.log")).unwrap()
        );
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let socket = agent.path().to_owned();
    let run_ssh = move || {
        Command::new("timeout")
            .args(["8", "ssh"])
            .args([
                "-F",
                "/dev/null",
                "-o",
                "IdentityFile=none",
                "-o",
                "IdentitiesOnly=no",
                "-o",
                "StrictHostKeyChecking=no",
                "-o",
                "UserKnownHostsFile=/dev/null",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=5",
                "-o",
                "PubkeyAuthentication=host-bound",
                "-p",
            ])
            .arg(port.to_string())
            .env("SSH_AUTH_SOCK", socket)
            .args(["127.0.0.1", "printf agent-authenticated"])
            .output()
            .unwrap()
    };
    let command = tokio::task::spawn_blocking(run_ssh.clone());
    let event = tokio::time::timeout(std::time::Duration::from_secs(10), events.recv())
        .await
        .expect("SSH approval")
        .unwrap();
    let UiEvent::Approval(approval) = event else {
        panic!("approval expected")
    };
    assert_eq!(
        approval.hosts.len(),
        1,
        "OpenSSH must supply verified host binding"
    );
    assert!(
        approval.can_remember,
        "OpenSSH must negotiate host-bound signing"
    );
    assert_eq!(approval.default_decision, Decision::Once);
    approval.respond(Decision::UntilLock);
    let output = command.await.unwrap();
    assert!(
        output.status.success(),
        "ssh: {}\nsshd: {}",
        String::from_utf8_lossy(&output.stderr),
        std::fs::read_to_string(base.join("sshd.log")).unwrap()
    );
    assert_eq!(output.stdout, b"agent-authenticated");
    assert!(matches!(events.recv().await.unwrap(), UiEvent::Dismiss(_)));
    let again = tokio::task::spawn_blocking(run_ssh).await.unwrap();
    assert!(
        again.status.success(),
        "a fresh host-bound connection must reuse the explicit grant: {}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert_eq!(again.stdout, b"agent-authenticated");
    assert!(
        events.try_recv().is_err(),
        "remembered grant should avoid another prompt"
    );
    drop(server);
    agent.stop();
    std::fs::remove_dir_all(base).unwrap();
}

#[tokio::test]
async fn remembered_approvals_are_bound_to_verified_host_username_and_authority() {
    let (manager, uid, identity) = crate::services::sdk::ssh_agent::test_fixture().await;
    let mut agent = AgentRuntime::new();
    agent.shared.state.lock().unwrap().enabled = true;
    agent.set_context(
        manager.agent_session_for(&uid),
        SshPromptBehavior::RememberUntilLock,
    );
    let epoch = agent.shared.state.lock().unwrap().epoch;
    let mut receiver = agent.events().subscribe();
    let sid = [9; 32];
    let binding = host_binding(&sid, false);
    let mut bindings = Bindings::default();
    assert_eq!(bindings.extension(&binding), vec![6]);
    let packet = auth_request(
        &identity.public_key,
        &sid,
        "alice",
        Some(&bindings.values.last().unwrap().host),
    );
    let shared = agent.shared.clone();
    let first = packet.clone();
    let task = tokio::spawn(async move {
        let reply = shared.handle(first, &mut bindings, epoch, "test").await;
        (reply, bindings)
    });
    let UiEvent::Approval(approval) = receiver.recv().await.unwrap() else {
        panic!("approval")
    };
    assert!(approval.can_remember);
    approval.respond(Decision::UntilLock);
    let (reply, mut bindings) = task.await.unwrap();
    assert_eq!(reply[0], 14);
    assert!(matches!(
        receiver.recv().await.unwrap(),
        UiEvent::Dismiss(_)
    ));
    assert_eq!(
        agent
            .shared
            .handle(packet, &mut bindings, epoch, "different local process")
            .await[0],
        14
    );
    assert!(receiver.try_recv().is_err());
    let shared = agent.shared.clone();
    let changed = auth_request(
        &identity.public_key,
        &sid,
        "bob",
        Some(&bindings.values.last().unwrap().host),
    );
    let task =
        tokio::spawn(async move { shared.handle(changed, &mut bindings, epoch, "test").await });
    let UiEvent::Approval(approval) = receiver.recv().await.unwrap() else {
        panic!("different SSH user needs approval")
    };
    approval.respond(Decision::Deny);
    assert_eq!(task.await.unwrap(), vec![5]);
    assert_eq!(agent.shared.state.lock().unwrap().grants.len(), 1);
    agent.set_context(manager.agent_session_for(&uid), SshPromptBehavior::Always);
    assert!(agent.shared.state.lock().unwrap().grants.is_empty());
    assert!(!agent.shared.current(epoch));
}
#[cfg(unix)]
#[tokio::test]
async fn disconnected_request_dismisses_prompt_without_signing() {
    let (manager, uid, identity) = crate::services::sdk::ssh_agent::test_fixture().await;
    let (mut agent, base) = runtime();
    agent.configure(true);
    agent.set_context(manager.agent_session_for(&uid), SshPromptBehavior::Always);
    let mut receiver = agent.events().subscribe();
    let mut stream = tokio::net::UnixStream::connect(agent.path()).await.unwrap();
    let packet = request(&identity.public_key, &[7; 32], "alice");
    stream.write_u32(packet.len() as u32).await.unwrap();
    stream.write_all(&packet).await.unwrap();
    let UiEvent::Approval(approval) = receiver.recv().await.unwrap() else {
        panic!("approval")
    };
    drop(stream);
    assert!(matches!(
        tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
            .await
            .unwrap()
            .unwrap(),
        UiEvent::Dismiss(_)
    ));
    approval.respond(Decision::Once);
    assert!(agent.shared.state.lock().unwrap().grants.is_empty());
    agent.stop();
    std::fs::remove_dir(base).unwrap();
}

#[test]
fn default_socket_path_is_fixed_and_has_a_private_per_user_fallback() {
    assert_eq!(
        stable_socket_path(Some(Path::new("/run/user/1000")), Path::new("/tmp"), 1000),
        Path::new("/run/user/1000/bitwarden-desktop-next/ssh-agent.sock")
    );
    assert_eq!(
        stable_socket_path(None, Path::new("/tmp"), 1000),
        Path::new("/tmp/bw-native-ssh-1000/ssh-agent.sock")
    );
    assert_eq!(
        stable_socket_path(Some(Path::new("")), Path::new("/tmp"), 1001),
        Path::new("/tmp/bw-native-ssh-1001/ssh-agent.sock")
    );
}
#[cfg(unix)]
#[test]
fn copied_shell_command_preserves_spaces_quotes_and_shell_metacharacters() {
    let mut agent = AgentRuntime::new();
    agent.path = PathBuf::from("/tmp/private dir/quote'$(printf injected);agent.sock");
    let script = format!("{}\nprintf '%s' \"$SSH_AUTH_SOCK\"", agent.shell_setup());
    let result = std::process::Command::new("sh")
        .args(["-c", &script])
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout, agent.path.as_os_str().as_encoded_bytes());
}
#[cfg(unix)]
#[tokio::test]
async fn another_profile_cannot_replace_the_default_listener_and_relative_overrides_fail() {
    assert!(transport::bind(Path::new("relative-agent.sock")).is_err());
    let (mut first, base) = runtime();
    first.configure(true);
    assert!(first.task.is_some());
    let mut second = AgentRuntime::new();
    second.path = first.path.clone();
    second.configure(true);
    assert!(second.task.is_none());
    assert!(second.error.is_some());
    second.stop();
    let mut stream = tokio::net::UnixStream::connect(first.path()).await.unwrap();
    assert_eq!(exchange(&mut stream, &[11]).await, vec![12, 0, 0, 0, 0]);
    first.stop();
    std::fs::remove_dir(base).unwrap();
}

#[tokio::test]
async fn explicit_until_lock_is_available_with_default_once_but_plain_auth_cannot_reuse_it() {
    let (manager, uid, identity) = crate::services::sdk::ssh_agent::test_fixture().await;
    let mut agent = AgentRuntime::new();
    agent.shared.state.lock().unwrap().enabled = true;
    agent.set_context(manager.agent_session_for(&uid), SshPromptBehavior::Always);
    let epoch = agent.shared.state.lock().unwrap().epoch;
    let mut events = agent.events().subscribe();
    let sid = [5; 32];
    let mut bindings = Bindings::default();
    let bind = host_binding(&sid, false);
    assert_eq!(bindings.extension(&bind), vec![6]);
    let bound = auth_request(
        &identity.public_key,
        &sid,
        "alice",
        Some(&bindings.values[0].host),
    );
    let shared = agent.shared.clone();
    let task = tokio::spawn(async move {
        let reply = shared.handle(bound, &mut bindings, epoch, "test").await;
        (reply, bindings)
    });
    let UiEvent::Approval(approval) = events.recv().await.unwrap() else {
        panic!("approval")
    };
    assert!(approval.can_remember);
    assert_eq!(approval.default_decision, Decision::Once);
    approval.respond(Decision::UntilLock);
    let (reply, mut bindings) = task.await.unwrap();
    assert_eq!(reply[0], 14);
    assert!(matches!(events.recv().await.unwrap(), UiEvent::Dismiss(_)));
    assert_eq!(agent.shared.state.lock().unwrap().grants.len(), 1);
    let plain = request(&identity.public_key, &sid, "alice");
    let shared = agent.shared.clone();
    let task =
        tokio::spawn(async move { shared.handle(plain, &mut bindings, epoch, "test").await });
    let UiEvent::Approval(approval) = events.recv().await.unwrap() else {
        panic!("plain auth cannot reuse bound grant")
    };
    assert!(!approval.can_remember);
    assert_eq!(approval.default_decision, Decision::Once);
    approval.respond(Decision::Deny);
    assert_eq!(task.await.unwrap(), vec![5]);
}
