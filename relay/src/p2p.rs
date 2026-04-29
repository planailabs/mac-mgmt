//! libp2p swarm for the relay node.
//!
//! Runs a circuit relay server so daemons behind NAT can reach each
//! other.  Also implements the control request-response protocol to
//! send session/metrics/proxy requests to daemons via libp2p.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use libp2p::identity::Keypair;
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{Multiaddr, PeerId, Swarm, Transport, identify};
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
    streams: libp2p_stream::Behaviour,
}

// ── Key management ───────────────────────────────────────────────────

fn load_or_generate_key(path: &Path) -> Result<Keypair> {
    if path.exists() {
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read key from {}", path.display()))?;
        let kp = Keypair::from_protobuf_encoding(&bytes)
            .map_err(|e| anyhow::anyhow!("failed to decode key: {e}"))?;
        Ok(kp)
    } else {
        tracing::info!("generating new relay Ed25519 key at {}", path.display());
        let kp = Keypair::generate_ed25519();
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

// ── Public API ───────────────────────────────────────────────────────

/// A control request to send to a peer, with a channel for the response.
pub struct OutboundRequest {
    pub peer_id: PeerId,
    pub request: serde_json::Value,
    pub response_tx: tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
}

/// Handle for the relay's libp2p swarm.
#[allow(dead_code)]
pub struct RelaySwarm {
    pub local_peer_id: PeerId,
    /// Map PeerId → instance metadata (cluster_id, tunnels, etc.)
    pub peer_metadata: Arc<RwLock<HashMap<PeerId, PeerMetadata>>>,
    /// Channel to send control requests to the swarm event loop.
    outbound_tx: tokio::sync::mpsc::Sender<OutboundRequest>,
    /// Control handle for opening raw substreams to peers.
    pub stream_control: libp2p_stream::Control,
}

/// Protocol for tunnel data substreams.
pub const TUNNEL_STREAM_PROTOCOL: libp2p::StreamProtocol =
    libp2p::StreamProtocol::new("/mac-mgmt/tunnel/1.0.0");

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

    /// Open a raw bidirectional substream to a peer for tunnel data.
    pub async fn open_tunnel_stream(
        &self,
        peer_id: PeerId,
    ) -> Result<libp2p::Stream, libp2p_stream::OpenStreamError> {
        self.stream_control
            .clone()
            .open_stream(peer_id, TUNNEL_STREAM_PROTOCOL)
            .await
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
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
        proxy_url: Option<&str>,
    ) -> Result<Self> {
        let keypair = load_or_generate_key(key_path)?;
        let local_peer_id = keypair.public().to_peer_id();
        tracing::info!(%local_peer_id, "relay p2p identity ready");

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default().nodelay(true),
                libp2p::noise::Config::new,
                || libp2p::yamux::Config::default(),
            )?
            .with_quic()
            .with_other_transport(|key| {
                let tcp = libp2p::tcp::tokio::Transport::new(
                    libp2p::tcp::Config::default().nodelay(true),
                );
                let ws = libp2p::websocket::Config::new(tcp)
                    .upgrade(libp2p::core::upgrade::Version::V1)
                    .authenticate(libp2p::noise::Config::new(key)?)
                    .multiplex(libp2p::yamux::Config::default())
                    .map(|(peer, muxer), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(muxer)));
                Ok(ws.boxed())
            })?
            .with_behaviour(|key| {
                let identify_cfg = identify::Config::new(
                    "/mac-mgmt-relay/1.0.0".to_string(),
                    key.public(),
                )
                .with_agent_version({
                    use base64::Engine;
                    let proxy_b64 = proxy_url
                        .map(|u| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(u))
                        .unwrap_or_default();
                    format!("mac-mgmt-relay/{}/{proxy_b64}", env!("CARGO_PKG_VERSION"))
                });

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
                    streams: libp2p_stream::Behaviour::new(),
                })
            })?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(120)))
            .build();

        // Listen on QUIC (IPv6 dual-stack covers IPv4 too)
        let quic_addr: Multiaddr = format!("/ip6/::/udp/{p2p_port}/quic-v1")
            .parse()
            .context("invalid QUIC listen address")?;
        swarm.listen_on(quic_addr)?;

        // Also listen on WS for daemons behind restrictive firewalls
        let ws_addr: Multiaddr = format!("/ip6/::/tcp/{p2p_port}/ws")
            .parse()
            .context("invalid WS listen address")?;
        swarm.listen_on(ws_addr)?;

        let peer_metadata = Arc::new(RwLock::new(HashMap::new()));
        let (outbound_tx, outbound_rx) = tokio::sync::mpsc::channel(256);

        // Extract the stream control handle before moving the swarm.
        let stream_control = swarm.behaviour().streams.new_control();

        // Spawn the event loop
        let pm_clone = Arc::clone(&peer_metadata);
        tokio::spawn(async move {
            relay_event_loop(swarm, registry, pm_clone, outbound_rx).await;
        });

        Ok(Self {
            outbound_tx,
            stream_control,
            local_peer_id,
            peer_metadata,
        })
    }
}

use futures_util::StreamExt;

async fn relay_event_loop(
    mut swarm: Swarm<RelayBehaviour>,
    registry: Arc<DaemonRegistry>,
    peer_metadata: Arc<RwLock<HashMap<PeerId, PeerMetadata>>>,
    mut outbound_rx: tokio::sync::mpsc::Receiver<OutboundRequest>,
) {
    // Map PeerId → instance_id for cleanup on disconnect.
    let mut peer_instance_map: HashMap<PeerId, (String, chrono::DateTime<chrono::Utc>)> =
        HashMap::new();
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
                    &registry,
                    &mut peer_instance_map,
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
    registry: &Arc<DaemonRegistry>,
    peer_instance_map: &mut HashMap<PeerId, (String, chrono::DateTime<chrono::Utc>)>,
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
                message: request_response::Message::Request { channel, request, .. },
                ..
            },
        )) => {
            let msg_type = request["type"].as_str().unwrap_or("");
            match msg_type {
                "register" => {
                    let instance_id = request["instance_id"].as_str().unwrap_or("").to_string();
                    let cluster_id = request["cluster_id"]
                        .as_str()
                        .and_then(|s| s.parse().ok());
                    let hostname = request["hostname"].as_str().map(String::from);
                    let agent_name = request["agent_name"].as_str().map(String::from);
                    let now = chrono::Utc::now();

                    if !instance_id.is_empty() {
                        tracing::info!(%peer, %instance_id, "daemon registered via p2p");
                        registry.register(crate::daemon_registry::DaemonConn {
                            instance_id: instance_id.clone(),
                            cluster_id,
                            cluster_name: None,
                            agent_name,
                            hostname,
                            connected_at: now,
                            tunnels: Vec::new(),
                            file_tunnels: serde_json::Value::Array(vec![]),
                            shell_tunnels: serde_json::Value::Array(vec![]),
                            peer_id: Some(peer),
                        });
                        peer_instance_map.insert(peer, (instance_id, now));
                    }

                    let resp = serde_json::json!({ "type": "ok" });
                    let _ = swarm.behaviour_mut().control.send_response(channel, resp);
                }
                "tunnel_advertisement" => {
                    if let Some((instance_id, _)) = peer_instance_map.get(&peer) {
                        let tunnels: Vec<crate::daemon_registry::ServiceTunnel> = request["tunnels"]
                            .as_array()
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|t| {
                                        Some(crate::daemon_registry::ServiceTunnel {
                                            name: t["name"].as_str()?.to_string(),
                                            tcp_port: t["port"].as_u64()? as u16,
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let file_tunnels = request["file_tunnels"].clone();
                        let shell_tunnels = request["shell_tunnels"].clone();
                        registry.update_all_tunnels(instance_id, tunnels, file_tunnels, shell_tunnels);
                        tracing::debug!(%peer, %instance_id, "tunnel advertisement received");
                    }
                    let resp = serde_json::json!({ "type": "ok" });
                    let _ = swarm.behaviour_mut().control.send_response(channel, resp);
                }
                _ => {
                    tracing::debug!(%peer, %msg_type, "unknown control request");
                    let resp = serde_json::json!({ "type": "error", "message": "unknown request type" });
                    let _ = swarm.behaviour_mut().control.send_response(channel, resp);
                }
            }
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
            if let Some((instance_id, connected_at)) = peer_instance_map.remove(&peer_id) {
                registry.unregister(&instance_id, connected_at);
            }
        }
        _ => {}
    }
}
