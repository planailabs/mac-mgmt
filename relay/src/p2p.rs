//! libp2p swarm for the relay node.
//!
//! Runs a circuit relay server so daemons behind NAT can reach each
//! other.  Also implements the control request-response protocol to
//! send session/metrics/proxy requests to daemons via libp2p.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use libp2p::identity::Keypair;
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{Multiaddr, PeerId, Swarm, identify};
use tokio::sync::RwLock;

use crate::daemon_registry::DaemonRegistry;

// ── Protocol types (shared with daemon) ──────────────────────────────

/// Re-use the same control protocol wire format as the daemon.
/// We inline the codec here to avoid a cross-crate dep on the daemon.
mod control_codec {
    use async_trait::async_trait;
    use futures_util::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
    use libp2p::request_response;
    use libp2p::StreamProtocol;

    pub const PROTOCOL_NAME: StreamProtocol = StreamProtocol::new("/mac-mgmt/control/1.0.0");

    pub type ControlRequest = serde_json::Value;
    pub type ControlResponse = serde_json::Value;

    #[derive(Debug, Clone, Default)]
    pub struct ControlCodec;

    const MAX_MSG_SIZE: u64 = 16 * 1024 * 1024;

    #[async_trait]
    impl request_response::Codec for ControlCodec {
        type Protocol = StreamProtocol;
        type Request = ControlRequest;
        type Response = ControlResponse;

        async fn read_request<T>(&mut self, _: &StreamProtocol, io: &mut T) -> std::io::Result<Self::Request>
        where T: AsyncRead + Unpin + Send {
            read_lp(io).await
        }

        async fn read_response<T>(&mut self, _: &StreamProtocol, io: &mut T) -> std::io::Result<Self::Response>
        where T: AsyncRead + Unpin + Send {
            read_lp(io).await
        }

        async fn write_request<T>(&mut self, _: &StreamProtocol, io: &mut T, req: Self::Request) -> std::io::Result<()>
        where T: AsyncWrite + Unpin + Send {
            write_lp(io, &req).await
        }

        async fn write_response<T>(&mut self, _: &StreamProtocol, io: &mut T, resp: Self::Response) -> std::io::Result<()>
        where T: AsyncWrite + Unpin + Send {
            write_lp(io, &resp).await
        }
    }

    async fn read_lp<T, M>(io: &mut T) -> std::io::Result<M>
    where T: AsyncRead + Unpin + Send, M: serde::de::DeserializeOwned {
        let mut len_buf = [0u8; 4];
        io.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as u64;
        if len > MAX_MSG_SIZE {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "msg too large"));
        }
        let mut buf = vec![0u8; len as usize];
        io.read_exact(&mut buf).await?;
        serde_json::from_slice(&buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    async fn write_lp<T, M>(io: &mut T, msg: &M) -> std::io::Result<()>
    where T: AsyncWrite + Unpin + Send, M: serde::Serialize {
        let data = serde_json::to_vec(msg).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        io.write_all(&(data.len() as u32).to_be_bytes()).await?;
        io.write_all(&data).await?;
        io.close().await
    }
}

// ── Behaviour ────────────────────────────────────────────────────────

#[derive(NetworkBehaviour)]
struct RelayBehaviour {
    identify: identify::Behaviour,
    relay_server: libp2p::relay::Behaviour,
    control: request_response::Behaviour<control_codec::ControlCodec>,
}

// ── Key management ───────────────────────────────────────────────────

fn load_or_generate_key(path: &Path) -> Result<Keypair> {
    if path.exists() {
        let pem = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read key from {}", path.display()))?;
        // Parse PKCS8 PEM → Ed25519 seed → libp2p Keypair
        // Same logic as daemon/src/p2p/identity.rs
        let der = decode_pem_to_der(&pem)?;
        let seed = extract_ed25519_seed(&der)?;
        let secret = libp2p::identity::ed25519::SecretKey::try_from_bytes(seed)
            .map_err(|e| anyhow::anyhow!("invalid Ed25519 seed: {e}"))?;
        let ed_kp = libp2p::identity::ed25519::Keypair::from(secret);
        Ok(Keypair::from(ed_kp))
    } else {
        tracing::info!("generating new relay Ed25519 key at {}", path.display());
        let kp = Keypair::generate_ed25519();
        // Save as PKCS8 PEM for persistence
        // For simplicity, save the raw key bytes
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let encoded = kp.to_protobuf_encoding()
            .map_err(|e| anyhow::anyhow!("failed to encode key: {e}"))?;
        std::fs::write(path, &encoded)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(kp)
    }
}

fn decode_pem_to_der(pem: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    let mut b64 = String::new();
    let mut in_block = false;
    for line in pem.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("-----BEGIN ") { in_block = true; continue; }
        if trimmed.starts_with("-----END ") { break; }
        if in_block { b64.push_str(trimmed); }
    }
    if b64.is_empty() { bail!("no PEM data found"); }
    base64::engine::general_purpose::STANDARD.decode(&b64).context("invalid base64 in PEM")
}

fn extract_ed25519_seed(der: &[u8]) -> Result<[u8; 32]> {
    if der.len() < 48 { bail!("DER too short for Ed25519 PKCS8"); }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&der[16..48]);
    Ok(seed)
}

// ── Public API ───────────────────────────────────────────────────────

/// A control request to send to a peer, with a channel for the response.
pub struct OutboundRequest {
    pub peer_id: PeerId,
    pub request: serde_json::Value,
    pub response_tx: tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
}

/// Handle for the relay's libp2p swarm.
pub struct RelaySwarm {
    pub local_peer_id: PeerId,
    /// Map PeerId → instance metadata (cluster_id, tunnels, etc.)
    pub peer_metadata: Arc<RwLock<HashMap<PeerId, PeerMetadata>>>,
    /// Channel to send control requests to the swarm event loop.
    outbound_tx: tokio::sync::mpsc::Sender<OutboundRequest>,
}

impl RelaySwarm {
    /// Send a control request to a peer and wait for the response.
    pub async fn send_request(
        &self,
        peer_id: PeerId,
        request: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.outbound_tx
            .send(OutboundRequest {
                peer_id,
                request,
                response_tx: tx,
            })
            .await
            .map_err(|_| "swarm channel closed".to_string())?;
        rx.await.map_err(|_| "response channel dropped".to_string())?
    }
}

#[derive(Debug, Clone)]
pub struct PeerMetadata {
    pub agent_version: String,
    pub listen_addrs: Vec<Multiaddr>,
}

impl RelaySwarm {
    /// Start the relay libp2p swarm.
    pub async fn start(
        p2p_port: u16,
        key_path: &Path,
        registry: Arc<DaemonRegistry>,
    ) -> Result<Self> {
        let keypair = load_or_generate_key(key_path)?;
        let local_peer_id = keypair.public().to_peer_id();
        tracing::info!(%local_peer_id, "relay p2p identity ready");

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_quic()
            .with_behaviour(|key| {
                let identify_cfg = identify::Config::new(
                    "/mac-mgmt-relay/1.0.0".to_string(),
                    key.public(),
                )
                .with_agent_version(format!("mac-mgmt-relay/{}", env!("CARGO_PKG_VERSION")));

                let relay_server = libp2p::relay::Behaviour::new(
                    key.public().to_peer_id(),
                    libp2p::relay::Config::default(),
                );

                let control = request_response::Behaviour::new(
                    [(control_codec::PROTOCOL_NAME, ProtocolSupport::Full)],
                    request_response::Config::default()
                        .with_request_timeout(Duration::from_secs(30)),
                );

                Ok(RelayBehaviour {
                    identify: identify::Behaviour::new(identify_cfg),
                    relay_server,
                    control,
                })
            })?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(120)))
            .build();

        // Listen on QUIC
        let quic_addr: Multiaddr = format!("/ip4/0.0.0.0/udp/{p2p_port}/quic-v1")
            .parse()
            .context("invalid QUIC listen address")?;
        swarm.listen_on(quic_addr)?;

        // Also listen on WSS for daemons behind restrictive firewalls
        let ws_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{p2p_port}/ws")
            .parse()
            .context("invalid WS listen address")?;
        swarm.listen_on(ws_addr)?;

        let peer_metadata = Arc::new(RwLock::new(HashMap::new()));
        let (outbound_tx, outbound_rx) = tokio::sync::mpsc::channel(256);

        // Spawn the event loop
        let pm_clone = Arc::clone(&peer_metadata);
        tokio::spawn(async move {
            relay_event_loop(swarm, registry, pm_clone, outbound_rx).await;
        });

        Ok(Self {
            outbound_tx,
            local_peer_id,
            peer_metadata,
        })
    }
}

use futures_util::StreamExt;

async fn relay_event_loop(
    mut swarm: Swarm<RelayBehaviour>,
    _registry: Arc<DaemonRegistry>,
    peer_metadata: Arc<RwLock<HashMap<PeerId, PeerMetadata>>>,
    mut outbound_rx: tokio::sync::mpsc::Receiver<OutboundRequest>,
) {
    // Track pending outbound request responses by request_id.
    let mut pending_responses: HashMap<
        request_response::OutboundRequestId,
        tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
    > = HashMap::new();

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                handle_relay_event(
                    event,
                    &peer_metadata,
                    &mut swarm,
                    &mut pending_responses,
                ).await;
            }
            Some(req) = outbound_rx.recv() => {
                let req_id = swarm.behaviour_mut().control.send_request(
                    &req.peer_id,
                    req.request,
                );
                pending_responses.insert(req_id, req.response_tx);
            }
        }
    }
}

async fn handle_relay_event(
    event: SwarmEvent<RelayBehaviourEvent>,
    peer_metadata: &Arc<RwLock<HashMap<PeerId, PeerMetadata>>>,
    swarm: &mut Swarm<RelayBehaviour>,
    pending_responses: &mut HashMap<
        request_response::OutboundRequestId,
        tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
    >,
) {
    match event {
        SwarmEvent::Behaviour(RelayBehaviourEvent::Identify(
            identify::Event::Received { peer_id, info, .. },
        )) => {
            tracing::info!(%peer_id, agent = %info.agent_version, "daemon identified");
            for addr in &info.listen_addrs {
                swarm.add_peer_address(peer_id, addr.clone());
            }
            peer_metadata.write().await.insert(
                peer_id,
                PeerMetadata {
                    agent_version: info.agent_version,
                    listen_addrs: info.listen_addrs,
                },
            );
        }
        SwarmEvent::Behaviour(RelayBehaviourEvent::RelayServer(event)) => {
            tracing::debug!(?event, "relay server event");
        }
        SwarmEvent::Behaviour(RelayBehaviourEvent::Control(
            request_response::Event::Message {
                peer,
                message: request_response::Message::Request { channel, request: _, .. },
                ..
            },
        )) => {
            tracing::debug!(%peer, "control request from peer (relay does not handle)");
            let resp = serde_json::json!({ "type": "error", "message": "relay does not handle control requests" });
            let _ = swarm.behaviour_mut().control.send_response(channel, resp);
        }
        SwarmEvent::Behaviour(RelayBehaviourEvent::Control(
            request_response::Event::Message {
                message: request_response::Message::Response { request_id, response },
                ..
            },
        )) => {
            if let Some(tx) = pending_responses.remove(&request_id) {
                let _ = tx.send(Ok(response));
            }
        }
        SwarmEvent::Behaviour(RelayBehaviourEvent::Control(
            request_response::Event::OutboundFailure { request_id, error, .. },
        )) => {
            if let Some(tx) = pending_responses.remove(&request_id) {
                let _ = tx.send(Err(format!("outbound request failed: {error}")));
            }
        }
        SwarmEvent::NewListenAddr { address, .. } => {
            tracing::info!(%address, "relay listening on");
        }
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            tracing::info!(%peer_id, "peer connected to relay");
        }
        SwarmEvent::ConnectionClosed { peer_id, .. } => {
            tracing::info!(%peer_id, "peer disconnected from relay");
            peer_metadata.write().await.remove(&peer_id);
        }
        _ => {}
    }
}
