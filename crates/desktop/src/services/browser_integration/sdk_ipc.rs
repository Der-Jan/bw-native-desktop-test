//! SDK IPC adapter. Uses the pinned SDK's unmodified Noise implementation.
use super::Connection;
use bitwarden_core::UserId;
use bitwarden_ipc::{
    CommunicationBackend, CommunicationBackendReceiver, Endpoint, HostId,
    InMemorySessionRepository, IncomingMessage, IpcClient, IpcClientExt, IpcClientImpl,
    NoiseCryptoProvider, OutgoingMessage, RpcHandler, RpcRequest, RpcRequestInfo, Source,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};
#[derive(Debug)]
struct Closed;
impl bitwarden_ipc::IpcErrorKind for Closed {
    fn kind(&self) -> bitwarden_ipc::ErrorKind {
        bitwarden_ipc::ErrorKind::Fatal
    }
}
#[derive(Clone)]
struct Backend {
    incoming: broadcast::Sender<IncomingMessage>,
    outgoing: mpsc::Sender<Vec<u8>>,
    renderer: bool,
    connection: Arc<Connection>,
}
struct Receiver(Mutex<broadcast::Receiver<IncomingMessage>>);
impl CommunicationBackendReceiver for Receiver {
    type ReceiveError = Closed;
    async fn receive(&self) -> Result<IncomingMessage, Closed> {
        self.0.lock().await.recv().await.map_err(|_| Closed)
    }
}
impl CommunicationBackend for Backend {
    type SendError = Closed;
    type Receiver = Receiver;
    async fn send(&self, message: OutgoingMessage) -> Result<(), Closed> {
        if !self.connection.current()
            || message.destination != (Endpoint::BrowserBackground { id: HostId::Id(0) })
        {
            return Err(Closed);
        }
        let mut envelope = serde_json::json!({"type":if self.renderer {"forwarded-bitwarden-ipc-message"}else{"bitwarden-ipc-message"},"message":message});
        if self.renderer {
            envelope["originalSource"] = serde_json::json!("DesktopRenderer");
        }
        self.outgoing
            .send(serde_json::to_vec(&envelope).map_err(|_| Closed)?)
            .await
            .map_err(|_| Closed)
    }
    async fn subscribe(&self) -> Receiver {
        Receiver(Mutex::new(self.incoming.subscribe()))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
enum Status {
    Available,
    UnlockNeeded,
    HardwareUnavailable,
    NotEnabled,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatusRequest {
    user_id: UserId,
}
impl RpcRequest for StatusRequest {
    type Response = Status;
    const NAME: &'static str = "GetBiometricsStatus";
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UnlockRequest {
    user_id: UserId,
}
#[derive(Serialize, Deserialize)]
struct UnlockResponse {
    user_key: Option<SecretKey>,
}
struct SecretKey(zeroize::Zeroizing<String>);
impl Serialize for SecretKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for SecretKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self(zeroize::Zeroizing::new(String::deserialize(
            deserializer,
        )?)))
    }
}
impl RpcRequest for UnlockRequest {
    type Response = UnlockResponse;
    const NAME: &'static str = "UnlockBiometrics";
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthenticateRequest;
impl RpcRequest for AuthenticateRequest {
    type Response = bool;
    const NAME: &'static str = "AuthenticateBiometrics";
}
struct StatusHandler(Arc<Connection>);
impl RpcHandler for StatusHandler {
    type Request = StatusRequest;
    async fn handle(&self, request: StatusRequest, _: RpcRequestInfo) -> Status {
        match self.0.status(Some(request.user_id)) {
            0 => Status::Available,
            1 => Status::UnlockNeeded,
            _ => Status::NotEnabled,
        }
    }
}
struct UnlockHandler(Arc<Connection>);
impl RpcHandler for UnlockHandler {
    type Request = UnlockRequest;
    async fn handle(&self, request: UnlockRequest, _: RpcRequestInfo) -> UnlockResponse {
        UnlockResponse {
            user_key: self
                .0
                .key(request.user_id)
                .await
                .map(|key| SecretKey(super::encode_key(&key))),
        }
    }
}
struct AuthenticateHandler;
impl RpcHandler for AuthenticateHandler {
    type Request = AuthenticateRequest;
    // This bridge does not claim OS/biometric user verification. Key release is
    // a separate, explicit approval from an already unlocked desktop session.
    async fn handle(&self, _: AuthenticateRequest, _: RpcRequestInfo) -> bool {
        false
    }
}
pub(super) struct Peer {
    main: broadcast::Sender<IncomingMessage>,
    renderer: broadcast::Sender<IncomingMessage>,
    cancel: tokio_util::sync::CancellationToken,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
impl Peer {
    pub async fn new(
        connection: Arc<Connection>,
        outgoing: mpsc::Sender<Vec<u8>>,
    ) -> Result<Self, String> {
        let cancel = tokio_util::sync::CancellationToken::new();
        let (main, _) = broadcast::channel(64);
        let (renderer, _) = broadcast::channel(64);
        for (incoming, is_renderer) in [(main.clone(), false), (renderer.clone(), true)] {
            let backend = Backend {
                incoming,
                outgoing: outgoing.clone(),
                renderer: is_renderer,
                connection: connection.clone(),
            };
            let client = IpcClientImpl::new(
                NoiseCryptoProvider::new(),
                backend,
                InMemorySessionRepository::default(),
            );
            client
                .register_rpc_handler(bitwarden_ipc::discover::DiscoverHandler::new(
                    bitwarden_ipc::discover::DiscoverResponse {
                        version: crate::services::accounts::transport::BITWARDEN_PROTOCOL_VERSION
                            .into(),
                    },
                ))
                .await;
            if is_renderer {
                client
                    .register_rpc_handler(StatusHandler(connection.clone()))
                    .await;
                client
                    .register_rpc_handler(UnlockHandler(connection.clone()))
                    .await;
                client.register_rpc_handler(AuthenticateHandler).await;
            }
            client
                .start(Some(cancel.clone()))
                .await
                .map_err(|_| "SDK IPC could not start")?;
        }
        Ok(Self {
            main,
            renderer,
            cancel,
        })
    }
    pub fn receive(&self, value: serde_json::Value) -> Result<(), String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Envelope {
            r#type: String,
            message: OutgoingMessage,
        }
        let envelope: Envelope =
            serde_json::from_value(value).map_err(|_| "Invalid SDK message")?;
        if envelope.r#type != "bitwarden-ipc-message"
            || envelope
                .message
                .topic
                .as_ref()
                .is_some_and(|t| t.len() > 128)
        {
            return Err("Unsupported SDK message".into());
        }
        let destination = envelope.message.destination;
        let sender = match destination {
            Endpoint::DesktopMain => &self.main,
            Endpoint::DesktopRenderer => &self.renderer,
            _ => return Err("Unsupported SDK destination".into()),
        };
        sender
            .send(IncomingMessage {
                payload: envelope.message.payload,
                destination,
                source: Source::BrowserBackground { id: HostId::Id(0) },
                topic: envelope.message.topic,
            })
            .map_err(|_| "SDK receiver unavailable")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[derive(Clone)]
    struct ClientBackend {
        incoming: broadcast::Sender<IncomingMessage>,
        outgoing: mpsc::Sender<OutgoingMessage>,
    }
    impl CommunicationBackend for ClientBackend {
        type SendError = Closed;
        type Receiver = Receiver;
        async fn send(&self, message: OutgoingMessage) -> Result<(), Closed> {
            self.outgoing.send(message).await.map_err(|_| Closed)
        }
        async fn subscribe(&self) -> Receiver {
            Receiver(Mutex::new(self.incoming.subscribe()))
        }
    }
    #[tokio::test]
    async fn real_sdk_noise_discovery_status_and_approved_key_release_interoperate() {
        let (runtime, _manager, connection) = super::super::test_fixture();
        let expected =
            super::super::encode_key(&connection.session.approved_browser_key().await.unwrap());
        let mut approvals = runtime.events().subscribe();
        let (server_out, mut server_rx) = mpsc::channel::<Vec<u8>>(32);
        let peer = Arc::new(Peer::new(connection.clone(), server_out).await.unwrap());
        let (incoming, _) = broadcast::channel(64);
        let (to_server, mut server_input) = mpsc::channel::<OutgoingMessage>(32);
        let client = Arc::new(IpcClientImpl::new(
            NoiseCryptoProvider::new(),
            ClientBackend {
                incoming: incoming.clone(),
                outgoing: to_server,
            },
            InMemorySessionRepository::default(),
        ));
        let cancel = tokio_util::sync::CancellationToken::new();
        client.start(Some(cancel.clone())).await.unwrap();
        let observed = Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
        let capture = observed.clone();
        let server = peer.clone();
        let router = tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg=server_input.recv()=>{let Some(msg)=msg else {break;};server.receive(serde_json::json!({"type":"bitwarden-ipc-message","message":msg})).unwrap();},
                    msg=server_rx.recv()=>{let Some(msg)=msg else {break;};capture.lock().unwrap().push(msg.clone());let value:serde_json::Value=serde_json::from_slice(&msg).unwrap();let source=if value["type"]=="forwarded-bitwarden-ipc-message"{Source::DesktopRenderer}else{Source::DesktopMain};let msg:OutgoingMessage=serde_json::from_value(value["message"].clone()).unwrap();let _=incoming.send(IncomingMessage{payload:msg.payload,destination:msg.destination,source,topic:msg.topic});}
                }
            }
        });
        let discover = tokio::time::timeout(
            Duration::from_secs(5),
            client.request(
                bitwarden_ipc::discover::DiscoverRequest,
                Endpoint::DesktopMain,
                None,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!discover.version.is_empty());
        let id = connection.session.server_user_id;
        let status = tokio::time::timeout(
            Duration::from_secs(5),
            client.request(
                StatusRequest { user_id: id },
                Endpoint::DesktopRenderer,
                None,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(matches!(status, Status::Available));
        let wrong = client
            .request(
                StatusRequest {
                    user_id: UserId::new_v4(),
                },
                Endpoint::DesktopRenderer,
                None,
            )
            .await
            .unwrap();
        assert!(matches!(wrong, Status::NotEnabled));
        assert!(
            !client
                .request(AuthenticateRequest, Endpoint::DesktopRenderer, None)
                .await
                .unwrap()
        );
        for allowed in [false, true] {
            let c = client.clone();
            let pending = tokio::spawn(async move {
                c.request(
                    UnlockRequest { user_id: id },
                    Endpoint::DesktopRenderer,
                    None,
                )
                .await
            });
            let prompt = loop {
                if let super::super::Event::Approval(prompt) =
                    tokio::time::timeout(Duration::from_secs(5), approvals.recv())
                        .await
                        .unwrap()
                        .unwrap()
                {
                    break prompt;
                }
            };
            assert!(!pending.is_finished());
            prompt.respond(if allowed {
                super::super::Decision::Once
            } else {
                super::super::Decision::Deny
            });
            let response = pending.await.unwrap().unwrap();
            if allowed {
                assert_eq!(response.user_key.unwrap().0.as_str(), expected.as_str());
            } else {
                assert!(response.user_key.is_none());
            }
        }
        for bytes in observed.lock().unwrap().iter() {
            assert!(
                !bytes
                    .windows(expected.len())
                    .any(|w| w == expected.as_bytes()),
                "Vault key appeared unencrypted on the native wire"
            );
        }
        cancel.cancel();
        drop(peer);
        router.abort();
    }
}
