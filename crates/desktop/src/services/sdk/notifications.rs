//! Vaultwarden SignalR hints. Payloads never become vault state: the coordinator
//! downloads an authenticated snapshot. TLS is upgraded by the account transport,
//! not by a second WebSocket TLS stack with different trust or mTLS settings.
use super::{SessionClient, SessionToken, refresh};
use crate::services::accounts::profile::ProfileStore;
use futures::{SinkExt, StreamExt};
use std::{
    io::Cursor,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{
        Message,
        handshake::derive_accept_key,
        protocol::{Role, WebSocketConfig},
    },
};
use tokio_util::sync::CancellationToken;

const LIMIT: usize = 64 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(10);
type Socket = WebSocketStream<reqwest_account::Upgraded>;
struct Connection {
    socket: Socket,
    transport: crate::services::accounts::transport::Transport,
    endpoint: url::Url,
    expires: i64,
    proxy_version: u64,
}

#[derive(Default)]
struct Hints {
    changed: AtomicBool,
    reconnected: AtomicBool,
    revoked: AtomicBool,
}
pub struct Subscription {
    lifetime: CancellationToken,
    hints: Arc<Hints>,
    _task: tokio::task::JoinHandle<()>,
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.lifetime.cancel();
    }
}
impl Subscription {
    pub fn is_active(&self) -> bool {
        !self.lifetime.is_cancelled()
    }
    pub fn take_changed(&self) -> bool {
        self.hints.changed.swap(false, Ordering::SeqCst)
    }
    pub fn take_reconnected(&self) -> bool {
        self.hints.reconnected.swap(false, Ordering::SeqCst)
    }
    pub fn take_revoked(&self) -> bool {
        self.hints.revoked.swap(false, Ordering::SeqCst)
    }
}

pub fn subscribe(
    client: SessionClient,
    root: PathBuf,
    store: ProfileStore,
    runtime: &tokio::runtime::Handle,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> Result<Subscription, String> {
    let session = client.session();
    let lifetime = session.notifications_lifetime()?.child_token();
    let hints = Arc::new(Hints::default());
    let task_hints = hints.clone();
    let cancelled = lifetime.clone();
    let task = runtime.spawn(run(session, root, store, task_hints, wake, cancelled));
    Ok(Subscription {
        lifetime,
        hints,
        _task: task,
    })
}

async fn run(
    session: SessionToken,
    root: PathBuf,
    store: ProfileStore,
    hints: Arc<Hints>,
    wake: Arc<dyn Fn() + Send + Sync>,
    cancelled: CancellationToken,
) {
    let mut failures = 0u32;
    loop {
        let current = session.state.token();
        if cancelled.is_cancelled()
            || current.ensure_unlocked().is_err()
            || !current.state.network_authorized.load(Ordering::SeqCst)
        {
            return;
        }
        let connected = connect(&current, root.clone(), store.clone(), &cancelled).await;
        if let Ok(connection) = connected {
            let started = tokio::time::Instant::now();
            let _ = cancelled
                .run_until_cancelled(listen(connection, &hints, &wake, &current))
                .await;
            if started.elapsed() > Duration::from_secs(30) {
                failures = 0;
            }
        }
        failures = failures.saturating_add(1).min(7);
        // Bounded backoff with per-attempt jitter avoids reconnect storms.
        let delay = Duration::from_secs((1u64 << failures).min(120))
            + Duration::from_millis(u64::from(uuid::Uuid::new_v4().as_bytes()[0]) * 4);
        if cancelled
            .run_until_cancelled(tokio::time::sleep(delay))
            .await
            .is_none()
        {
            return;
        }
    }
}

async fn connect(
    session: &SessionToken,
    root: PathBuf,
    store: ProfileStore,
    cancelled: &CancellationToken,
) -> Result<Connection, String> {
    // Refresh needs a short lease; revoke cancels the outer future even during TLS.
    let lease = session.state.gate.read().await;
    let mut authority =
        refresh::authorized_notifications(session, root.clone(), store.clone(), false).await?;
    drop(lease);
    for attempt in 0..2 {
        session.ensure_unlocked()?;
        let endpoint = authority.transport.profile.endpoint("notifications/hub")?;
        let key = tokio_tungstenite::tungstenite::handshake::client::generate_key();
        let request = authority
            .transport
            .request(reqwest_account::Method::GET, endpoint.clone())?
            .version(reqwest_account::Version::HTTP_11)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", &key)
            .bearer_auth(&authority.credentials.access_token)
            .timeout(IO_TIMEOUT)
            .build()
            .map_err(|_| "Could not prepare notification connection")?;
        let proxy_version = authority.transport.proxy_version();
        let response = cancelled
            .run_until_cancelled(authority.transport.execute(request))
            .await
            .ok_or("Notification session cancelled")??;
        session.ensure_unlocked()?;
        if response.status().as_u16() == 401 && attempt == 0 {
            let _lease = session.state.gate.read().await;
            authority =
                refresh::authorized_notifications(session, root.clone(), store.clone(), true)
                    .await?;
            continue;
        }
        let token_header = |name: &str, expected: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| {
                    v.split(',')
                        .any(|v| v.trim().eq_ignore_ascii_case(expected))
                })
        };
        if response.status().as_u16() != 101
            || !token_header("Connection", "upgrade")
            || !token_header("Upgrade", "websocket")
            || response
                .headers()
                .get("Sec-WebSocket-Accept")
                .and_then(|v| v.to_str().ok())
                != Some(derive_accept_key(key.as_bytes()).as_str())
            || response.headers().contains_key("Sec-WebSocket-Extensions")
            || response.headers().contains_key("Sec-WebSocket-Protocol")
        {
            return Err("Notification upgrade rejected; revision polling remains active".into());
        }
        let socket = cancelled
            .run_until_cancelled(async {
                let stream = tokio::time::timeout(IO_TIMEOUT, response.upgrade())
                    .await
                    .map_err(|_| "Notification upgrade timed out")?
                    .map_err(|_| "Notification upgrade failed")?;
                let config = WebSocketConfig::default()
                    .max_frame_size(Some(LIMIT))
                    .max_message_size(Some(LIMIT))
                    .write_buffer_size(0)
                    .max_write_buffer_size(LIMIT);
                let mut socket =
                    WebSocketStream::from_raw_socket(stream, Role::Client, Some(config)).await;
                tokio::time::timeout(
                    IO_TIMEOUT,
                    socket.send(Message::Text(
                        "{\"protocol\":\"messagepack\",\"version\":1}\u{1e}".into(),
                    )),
                )
                .await
                .map_err(|_| "Notification handshake timed out")?
                .map_err(|_| "Notification handshake failed")?;
                Ok::<_, String>(socket)
            })
            .await
            .ok_or("Notification session cancelled")??;
        if authority.transport.proxy_version() != proxy_version {
            return Err("Proxy authentication changed during upgrade".into());
        }
        return Ok(Connection {
            socket,
            transport: authority.transport,
            endpoint,
            expires: authority.credentials.expires_at,
            proxy_version,
        });
    }
    Err("Notification session expired".into())
}

async fn listen(
    connection: Connection,
    hints: &Hints,
    wake: &Arc<dyn Fn() + Send + Sync>,
    session: &SessionToken,
) -> Result<(), String> {
    let Connection {
        mut socket,
        transport,
        endpoint,
        expires,
        proxy_version,
    } = connection;
    let mut decoder = Decoder::default();
    let mut guard = tokio::time::interval(Duration::from_millis(100));
    guard.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_received = tokio::time::Instant::now();
    let mut last_sent = last_received;
    let handshake_deadline = last_received + IO_TIMEOUT;
    loop {
        tokio::select! {
            biased;
            _ = guard.tick() => {
                transport.check(&endpoint)?;
                if transport.proxy_version()!=proxy_version {return Err("Proxy authentication changed".into());}
                if !session.state.network_authorized.load(Ordering::SeqCst) { return Err("Notification session expired".into()); }
                let now = tokio::time::Instant::now();
                if (!decoder.ready && now >= handshake_deadline) || now.duration_since(last_received) > Duration::from_secs(45) { return Err("Notification connection timed out".into()); }
                if chrono::Utc::now().timestamp() >= expires - 60 { return Ok(()); }
                if decoder.ready && now.duration_since(last_sent) >= Duration::from_secs(15) {
                    tokio::time::timeout(IO_TIMEOUT, socket.send(Message::Binary(vec![2, 0x91, 6].into()))).await
                        .map_err(|_| "Notification heartbeat timed out")?.map_err(|_| "Notification heartbeat failed")?;
                    last_sent = now;
                }
            }
            message = socket.next() => {
                transport.check(&endpoint)?;
                if transport.proxy_version()!=proxy_version {return Err("Proxy authentication changed".into());}
                if !session.state.network_authorized.load(Ordering::SeqCst) { return Err("Notification session expired".into()); }
                let was_ready = decoder.ready;
                last_received = tokio::time::Instant::now();
                match message {
                    Some(Ok(Message::Text(data))) => { if decoder.feed(data.as_bytes())? && !hints.changed.swap(true, Ordering::SeqCst) { wake(); } },
                    Some(Ok(Message::Binary(data))) => { if decoder.feed(&data)? && !hints.changed.swap(true, Ordering::SeqCst) { wake(); } },
                    Some(Ok(Message::Ping(_))) => { tokio::time::timeout(IO_TIMEOUT, socket.flush()).await.map_err(|_| "Notification pong timed out")?.map_err(|_| "Notification pong failed")?; },
                    Some(Ok(Message::Pong(_))) => {},
                    _ => return Err("Notification connection closed".into()),
                }
                if decoder.revoked {
                    hints.revoked.store(true, Ordering::SeqCst);
                    wake();
                    return Err("Server revoked the notification session".into());
                }
                if decoder.ready && !was_ready && !hints.reconnected.swap(true, Ordering::SeqCst) { wake(); }
            }
        }
    }
}

#[derive(Default)]
struct Decoder {
    ready: bool,
    revoked: bool,
    pending: Vec<u8>,
}
impl Decoder {
    fn feed(&mut self, data: &[u8]) -> Result<bool, String> {
        if self.pending.len().saturating_add(data.len()) > LIMIT {
            return Err("Notification message is too large".into());
        }
        self.pending.extend_from_slice(data);
        if !self.ready {
            let Some(end) = self.pending.iter().position(|b| *b == 0x1e) else {
                return Ok(false);
            };
            let handshake: serde_json::Value = serde_json::from_slice(&self.pending[..end])
                .map_err(|_| "Invalid notification handshake")?;
            if handshake != serde_json::json!({}) {
                return Err("Notification protocol rejected".into());
            }
            self.pending.drain(..=end);
            self.ready = true;
        }
        let mut changed = false;
        loop {
            let mut len = 0usize;
            let mut prefix = None;
            for (i, b) in self.pending.iter().take(5).enumerate() {
                len |= usize::from(b & 0x7f) << (7 * i);
                if len > LIMIT {
                    return Err("Notification message is too large".into());
                }
                if b & 0x80 == 0 {
                    prefix = Some(i + 1);
                    break;
                }
                if i == 4 {
                    return Err("Invalid notification length".into());
                }
            }
            let Some(prefix) = prefix else {
                break;
            };
            if len == 0 {
                return Err("Empty notification message".into());
            }
            if self.pending.len() < prefix + len {
                break;
            }
            let mut cursor = Cursor::new(&self.pending[prefix..prefix + len]);
            let value = rmpv::decode::read_value_with_max_depth(&mut cursor, 24)
                .map_err(|_| "Invalid notification payload")?;
            if cursor.position() != len as u64 {
                return Err("Invalid notification payload length".into());
            }
            let array = value.as_array().ok_or("Invalid notification invocation")?;
            match array.first().and_then(|v| v.as_u64()) {
                Some(1) if array.get(3).and_then(|v| v.as_str()) == Some("ReceiveMessage") => {
                    changed = true;
                    // Authenticated LogOut hints fail closed. Other payload
                    // fields never become account authority or vault state.
                    self.revoked |= array.get(4).and_then(|v| v.as_array()).is_some_and(|args| {
                        args.iter().any(|arg| {
                            arg.as_map().is_some_and(|fields| {
                                fields.iter().any(|(k, v)| {
                                    k.as_str() == Some("Type") && v.as_u64() == Some(11)
                                })
                            })
                        })
                    });
                }
                Some(7) => return Err("Notification hub closed".into()),
                Some(_) => {}
                None => return Err("Invalid notification message type".into()),
            }
            self.pending.drain(..prefix + len);
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn framed(value: rmpv::Value) -> Vec<u8> {
        let mut body = Vec::new();
        rmpv::encode::write_value(&mut body, &value).unwrap();
        let mut size = body.len();
        let mut bytes = Vec::new();
        loop {
            let b = (size & 127) as u8;
            size >>= 7;
            bytes.push(b | if size > 0 { 128 } else { 0 });
            if size == 0 {
                break;
            }
        }
        bytes.extend(body);
        bytes
    }
    fn notification() -> Vec<u8> {
        framed(rmpv::Value::Array(vec![
            1.into(),
            rmpv::Value::Map(vec![]),
            rmpv::Value::Nil,
            "ReceiveMessage".into(),
            rmpv::Value::Array(vec![]),
        ]))
    }
    #[test]
    fn signalr_stream_handles_split_handshake_prefix_payload_and_coalesced_invocations() {
        let mut decoder = Decoder::default();
        let mut bytes = b"{}\x1e".to_vec();
        bytes.extend(framed(rmpv::Value::Array(vec![6.into()])));
        bytes.extend(notification());
        bytes.extend(notification());
        let mut count = 0;
        for b in bytes {
            count += usize::from(decoder.feed(&[b]).unwrap());
        }
        assert!(decoder.ready);
        assert_eq!(count, 2);
        assert!(decoder.pending.is_empty());
        let burst = notification().repeat(10);
        assert!(decoder.feed(&burst).unwrap());
    }
    #[test]
    fn signalr_rejects_failed_handshakes_oversize_invalid_prefix_and_deep_payloads() {
        assert!(
            Decoder::default()
                .feed(b"{\"error\":\"rejected\"}\x1e")
                .is_err()
        );
        assert!(Decoder::default().feed(&vec![0; LIMIT + 1]).is_err());
        for bad in [
            vec![0xff; 5],
            vec![0x80, 0x80, 0x05],
            vec![0],
            vec![2, 0x91, 0x91],
        ] {
            let mut decoder = Decoder::default();
            decoder.feed(b"{}\x1e").unwrap();
            assert!(decoder.feed(&bad).is_err());
        }
        let mut value = rmpv::Value::Nil;
        for _ in 0..30 {
            value = rmpv::Value::Array(vec![value]);
        }
        let mut decoder = Decoder::default();
        decoder.feed(b"{}\x1e").unwrap();
        assert!(decoder.feed(&framed(value)).is_err());
    }
    #[test]
    fn signalr_ignores_ping_and_other_targets_and_reconnects_on_hub_close() {
        let mut decoder = Decoder::default();
        decoder.feed(b"{}\x1e").unwrap();
        assert!(!decoder.feed(&[2, 0x91, 6]).unwrap());
        assert!(
            !decoder
                .feed(&framed(rmpv::Value::Array(vec![
                    1.into(),
                    rmpv::Value::Nil,
                    rmpv::Value::Nil,
                    "OtherTarget".into()
                ])))
                .unwrap()
        );
        assert!(
            decoder
                .feed(&framed(rmpv::Value::Array(vec![7.into()])))
                .is_err()
        );
    }
}
