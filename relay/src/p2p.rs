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
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{Multiaddr, PeerId, Swarm, Transport, gossipsub, identify};
use tokio::sync::RwLock;

use crate::daemon_registry::DaemonRegistry;
use crate::ssh_bridge::SshBridge;

// ── Behaviour ────────────────────────────────────────────────────────

#[derive(NetworkBehaviour)]
struct RelayBehaviour {
    ping: libp2p::ping::Behaviour,
    identify: identify::Behaviour,
    relay_server: libp2p::relay::Behaviour,
    gossipsub: gossipsub::Behaviour,
    streams: libp2p_stream::Behaviour,
}

/// Commands for the event loop to manage gossipsub subscriptions.
enum GossipCmd {
    Subscribe(uuid::Uuid),
    Unsubscribe(uuid::Uuid),
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
        let encoded = kp
            .to_protobuf_encoding()
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

/// Map of PeerId → channel to send RPC requests to a daemon's RPC handler.
type DaemonRpcMap = Arc<RwLock<HashMap<PeerId, tokio::sync::mpsc::Sender<DaemonRpcRequest>>>>;

/// A request to send to a daemon via its RPC stream.
pub struct DaemonRpcRequest {
    pub payload: serde_json::Value,
    pub response_tx: tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
}

/// Handle for the relay's libp2p swarm.
#[allow(dead_code)]
pub struct RelaySwarm {
    pub local_peer_id: PeerId,
    pub peer_metadata: Arc<RwLock<HashMap<PeerId, PeerMetadata>>>,
    pub stream_control: libp2p_stream::Control,
    /// RPC channels to connected daemons, keyed by PeerId.
    daemon_rpc_map: DaemonRpcMap,
    /// Shared daemon registry for lookups.
    registry: Arc<DaemonRegistry>,
    /// Server API URL for token validation during daemon registration.
    server_api_url: String,
    /// SSH bridge (set after construction via `set_ssh_bridge`).
    ssh_bridge: Arc<std::sync::OnceLock<Arc<SshBridge>>>,
}

/// Protocol for tunnel data substreams.
pub const TUNNEL_STREAM_PROTOCOL: libp2p::StreamProtocol =
    libp2p::StreamProtocol::new("/mac-mgmt/tunnel/1.0.0");

impl RelaySwarm {
    /// Send a control request to a daemon via its RPC stream and wait for the response.
    pub async fn send_request(
        &self,
        peer_id: PeerId,
        request: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let rpc_map = self.daemon_rpc_map.read().await;
        let tx = rpc_map
            .get(&peer_id)
            .ok_or_else(|| format!("no RPC stream for peer {peer_id}"))?
            .clone();
        drop(rpc_map);

        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        tx.send(DaemonRpcRequest {
            payload: request,
            response_tx: resp_tx,
        })
        .await
        .map_err(|_| "daemon RPC channel closed".to_string())?;

        resp_rx
            .await
            .map_err(|_| "daemon RPC response dropped".to_string())?
    }

    /// Resolve a daemon's PeerId by instance_id prefix.
    pub fn registry_resolve_peer_id(&self, prefix: &str) -> Option<PeerId> {
        self.registry.resolve_peer_id(prefix)
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
        server_api_url: &str,
        ssh_identity: Arc<crate::ssh_identity::RelaySshIdentity>,
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
                let dns_tcp = libp2p::dns::tokio::Transport::system(tcp)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                let ws = libp2p::websocket::Config::new(dns_tcp)
                    .upgrade(libp2p::core::upgrade::Version::V1)
                    .authenticate(libp2p::noise::Config::new(key)?)
                    .multiplex(libp2p::yamux::Config::default())
                    .map(|(peer, muxer), _| {
                        (peer, libp2p::core::muxing::StreamMuxerBox::new(muxer))
                    });
                Ok(ws.boxed())
            })?
            .with_behaviour(|key| {
                let identify_cfg =
                    identify::Config::new("/mac-mgmt-relay/1.0.0".to_string(), key.public())
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

                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .build()
                    .expect("gossipsub config");
                let gossipsub_behaviour = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )
                .expect("gossipsub behaviour");

                Ok(RelayBehaviour {
                    ping: libp2p::ping::Behaviour::new(
                        libp2p::ping::Config::new()
                            .with_interval(std::time::Duration::from_secs(15)),
                    ),
                    identify: identify::Behaviour::new(identify_cfg),
                    relay_server,
                    gossipsub: gossipsub_behaviour,
                    streams: libp2p_stream::Behaviour::new(),
                })
            })?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(3600)))
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

        // IPv4 listeners (for hosts without dual-stack)
        let quic_v4: Multiaddr = format!("/ip4/0.0.0.0/udp/{p2p_port}/quic-v1")
            .parse()
            .context("invalid IPv4 QUIC listen address")?;
        swarm.listen_on(quic_v4)?;
        let ws_v4: Multiaddr = format!("/ip4/0.0.0.0/tcp/{p2p_port}/ws")
            .parse()
            .context("invalid IPv4 WS listen address")?;
        swarm.listen_on(ws_v4)?;

        let peer_metadata = Arc::new(RwLock::new(HashMap::new()));
        let daemon_rpc_map: DaemonRpcMap = Arc::new(RwLock::new(HashMap::new()));

        // Extract the stream control handle before moving the swarm.
        let stream_control = swarm.behaviour().streams.new_control();

        // Channel for RPC handlers to request gossipsub subscribe/unsubscribe.
        let (gossip_tx, gossip_rx) = tokio::sync::mpsc::channel::<GossipCmd>(64);

        // Shared cell for the SSH bridge — set after RelaySwarm construction.
        let ssh_bridge_cell: Arc<std::sync::OnceLock<Arc<SshBridge>>> =
            Arc::new(std::sync::OnceLock::new());

        // Accept incoming RPC streams from daemons.
        let rpc_protocol = libp2p::StreamProtocol::new("/mac-mgmt/rpc/1.0.0");
        let mut incoming_streams = stream_control.clone().accept(rpc_protocol).unwrap();
        let registry_for_rpc = Arc::clone(&registry);
        let rpc_map_for_accept = Arc::clone(&daemon_rpc_map);
        let server_api_url_for_rpc = server_api_url.to_string();
        let gossip_tx_for_rpc = gossip_tx.clone();
        let ssh_bridge_for_rpc = Arc::clone(&ssh_bridge_cell);
        let ssh_identity_for_rpc = Arc::clone(&ssh_identity);
        tokio::spawn(async move {
            while let Some((peer_id, stream)) = incoming_streams.next().await {
                tracing::info!(%peer_id, "daemon opened RPC stream");
                let registry = Arc::clone(&registry_for_rpc);
                let rpc_map = Arc::clone(&rpc_map_for_accept);
                let server_url = server_api_url_for_rpc.clone();
                let gtx = gossip_tx_for_rpc.clone();
                let ssh_bridge = ssh_bridge_for_rpc.get().cloned();
                let ssh_id = Arc::clone(&ssh_identity_for_rpc);

                let (req_tx, req_rx) = tokio::sync::mpsc::channel(64);
                rpc_map.write().await.insert(peer_id, req_tx);

                let rpc_map_cleanup = Arc::clone(&rpc_map);
                tokio::spawn(async move {
                    handle_daemon_rpc(
                        peer_id, stream, registry, req_rx, server_url, gtx, ssh_bridge, ssh_id,
                    )
                    .await;
                    rpc_map_cleanup.write().await.remove(&peer_id);
                });
            }
        });

        // Spawn the swarm event loop
        let pm_clone = Arc::clone(&peer_metadata);
        let registry_for_self = Arc::clone(&registry);
        tokio::spawn(async move {
            relay_event_loop(swarm, registry, pm_clone, gossip_rx).await;
        });

        Ok(Self {
            registry: registry_for_self,
            daemon_rpc_map,
            stream_control,
            local_peer_id,
            peer_metadata,
            server_api_url: server_api_url.to_string(),
            ssh_bridge: ssh_bridge_cell,
        })
    }

    /// Set the SSH bridge after construction. Must be called before any
    /// daemon registers with ssh_enabled.
    pub fn set_ssh_bridge(&self, bridge: Arc<SshBridge>) {
        let _ = self.ssh_bridge.set(bridge);
    }
}

use futures_util::StreamExt;

/// Build the gossipsub topic for a cluster.
fn cluster_topic(cluster_id: &uuid::Uuid) -> gossipsub::IdentTopic {
    gossipsub::IdentTopic::new(format!("mac-mgmt/cluster/{cluster_id}"))
}

async fn relay_event_loop(
    mut swarm: Swarm<RelayBehaviour>,
    _registry: Arc<DaemonRegistry>,
    peer_metadata: Arc<RwLock<HashMap<PeerId, PeerMetadata>>>,
    mut gossip_rx: tokio::sync::mpsc::Receiver<GossipCmd>,
) {
    // Track how many daemons per cluster are connected.
    // Subscribe when count goes from 0→1, unsubscribe when 1→0.
    let mut cluster_peer_count: HashMap<uuid::Uuid, usize> = HashMap::new();

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::Behaviour(RelayBehaviourEvent::Identify(
                        identify::Event::Received { peer_id, info, .. },
                    )) => {
                        tracing::info!(%peer_id, agent = %info.agent_version, "peer identified");
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
                    SwarmEvent::Behaviour(RelayBehaviourEvent::Gossipsub(
                        gossipsub::Event::Message { message, .. },
                    )) => {
                        // The relay doesn't process gossipsub messages itself —
                        // it participates in the mesh to bridge daemons.
                        tracing::trace!(
                            topic = %message.topic,
                            "gossipsub message relayed ({} bytes)", message.data.len(),
                        );
                    }
                    SwarmEvent::Behaviour(RelayBehaviourEvent::RelayServer(event)) => {
                        tracing::debug!(?event, "relay server event");
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
            Some(cmd) = gossip_rx.recv() => {
                match cmd {
                    GossipCmd::Subscribe(cid) => {
                        let count = cluster_peer_count.entry(cid).or_insert(0);
                        *count += 1;
                        if *count == 1 {
                            let topic = cluster_topic(&cid);
                            match swarm.behaviour_mut().gossipsub.subscribe(&topic) {
                                Ok(_) => tracing::info!(%cid, "subscribed to cluster gossipsub topic"),
                                Err(e) => tracing::warn!(%cid, "failed to subscribe to cluster topic: {e}"),
                            }
                        }
                    }
                    GossipCmd::Unsubscribe(cid) => {
                        if let Some(count) = cluster_peer_count.get_mut(&cid) {
                            *count = count.saturating_sub(1);
                            if *count == 0 {
                                cluster_peer_count.remove(&cid);
                                let topic = cluster_topic(&cid);
                                if swarm.behaviour_mut().gossipsub.unsubscribe(&topic) {
                                    tracing::info!(%cid, "unsubscribed from cluster gossipsub topic");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Handle a persistent RPC stream from a single daemon.
/// Reads length-prefixed JSON frames, dispatches register/tunnel_advertisement,
/// sends responses back.
async fn handle_daemon_rpc(
    peer_id: PeerId,
    stream: libp2p::Stream,
    registry: Arc<DaemonRegistry>,
    mut req_rx: tokio::sync::mpsc::Receiver<DaemonRpcRequest>,
    server_api_url: String,
    gossip_tx: tokio::sync::mpsc::Sender<GossipCmd>,
    ssh_bridge: Option<Arc<SshBridge>>,
    ssh_identity: Arc<crate::ssh_identity::RelaySshIdentity>,
) {
    use futures_util::{AsyncReadExt, AsyncWriteExt};
    let (mut reader, mut writer) = stream.split();
    let mut instance_id: Option<String> = None;
    let mut connected_at: Option<chrono::DateTime<chrono::Utc>> = None;
    let mut registered_cluster_id: Option<uuid::Uuid> = None;
    let mut next_outbound_id: u64 = 1;
    let mut pending_outbound: HashMap<
        u64,
        tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
    > = HashMap::new();

    loop {
        // Read a frame from the daemon OR handle an outbound request from the relay.
        let mut len_buf = [0u8; 4];
        let frame_data = tokio::select! {
            result = reader.read_exact(&mut len_buf) => {
                if result.is_err() { break; }
                let len = u32::from_be_bytes(len_buf);
                if len > 16 * 1024 * 1024 { break; }
                let mut buf = vec![0u8; len as usize];
                if reader.read_exact(&mut buf).await.is_err() { break; }
                Some(buf)
            }
            Some(outbound) = req_rx.recv() => {
                // Relay wants to send a request to this daemon.
                let id = next_outbound_id;
                next_outbound_id += 1;
                let mut payload = outbound.payload;
                payload["id"] = serde_json::Value::Number(id.into());
                let data = serde_json::to_vec(&payload).unwrap_or_default();
                // Sending an oversized frame makes the daemon tear down the
                // whole RPC stream — fail just this request instead.
                if data.len() > mac_mgmt_common::framing::MAX_FRAME_SIZE as usize {
                    tracing::warn!(%peer_id, len = data.len(), "dropping oversized RPC request");
                    let _ = outbound
                        .response_tx
                        .send(Err("request too large for RPC frame".to_string()));
                    continue;
                }
                let _ = writer.write_all(&(data.len() as u32).to_be_bytes()).await;
                let _ = writer.write_all(&data).await;
                let _ = writer.flush().await;
                pending_outbound.insert(id, outbound.response_tx);
                continue;
            }
        };

        let Some(buf) = frame_data else { break };

        let Ok(frame) = serde_json::from_slice::<serde_json::Value>(&buf) else {
            tracing::warn!(%peer_id, "invalid JSON in RPC frame");
            continue;
        };

        // Check if this is a response to an outbound request we sent.
        let frame_id = frame["id"].as_u64().unwrap_or(0);
        if frame_id > 0 {
            if let Some(tx) = pending_outbound.remove(&frame_id) {
                let _ = tx.send(Ok(frame));
                continue;
            }
        }

        let msg_type = frame["type"].as_str().unwrap_or("");
        let req_id = frame["id"].clone();

        let resp = match msg_type {
            "register" => {
                let iid = frame["instance_id"].as_str().unwrap_or("").to_string();
                let hostname = frame["hostname"].as_str().map(String::from);
                let agent_name = frame["agent_name"].as_str().map(String::from);
                let now = chrono::Utc::now();

                // Validate token against server — registration without a valid token is rejected.
                let token = frame["token"].as_str().unwrap_or("");
                if token.is_empty() {
                    tracing::warn!(%peer_id, %iid, "daemon registration rejected: no token");
                    serde_json::json!({ "type": "error", "error": "token required", "id": req_id })
                } else {
                    match crate::auth::validate_token(&server_api_url, token).await {
                        Ok(info) if !iid.is_empty() => {
                            let cid = info.cluster_id;
                            let cname = info.cluster_name;
                            let ssh_enabled = frame["ssh_enabled"].as_bool().unwrap_or(false);
                            let accepted = registry.register(crate::daemon_registry::DaemonConn {
                                instance_id: iid.clone(),
                                cluster_id: cid,
                                cluster_name: cname.clone(),
                                agent_name,
                                hostname,
                                connected_at: now,
                                tunnels: Vec::new(),
                                file_tunnels: serde_json::Value::Array(vec![]),
                                shell_tunnels: serde_json::Value::Array(vec![]),
                                peer_id: Some(peer_id),
                                ssh_enabled,
                                ssh_port: None,
                            });
                            if !accepted {
                                serde_json::json!({ "type": "error", "error": "relay at capacity", "id": req_id })
                            } else {
                                tracing::info!(%peer_id, %iid, cluster_id = ?cid, "daemon registered via RPC stream");
                                instance_id = Some(iid.clone());
                                connected_at = Some(now);
                                // Start SSH bridge listener if enabled.
                                if ssh_enabled {
                                    if let Some(bridge) = &ssh_bridge {
                                        bridge.on_ssh_enabled(&iid);
                                    }
                                }
                                // Subscribe to cluster gossipsub topic.
                                if let Some(cid) = cid {
                                    registered_cluster_id = Some(cid);
                                    let _ = gossip_tx.send(GossipCmd::Subscribe(cid)).await;
                                }
                                // Read back the allocated SSH port (set by SshBridge).
                                let ssh_port = {
                                    let daemons = registry.list_ssh_targets();
                                    daemons
                                        .iter()
                                        .find(|t| t.instance_id == iid)
                                        .and_then(|t| t.ssh_port)
                                };
                                serde_json::json!({
                                    "type": "ok",
                                    "id": req_id,
                                    "cluster_id": cid.map(|c| c.to_string()),
                                    "ssh_port": ssh_port,
                                    "relay_ssh_pubkey": ssh_identity.public_key_openssh,
                                })
                            }
                        }
                        Ok(_) => {
                            serde_json::json!({ "type": "error", "error": "empty instance_id", "id": req_id })
                        }
                        Err(status) => {
                            tracing::warn!(%peer_id, %iid, ?status, "daemon registration rejected: token validation failed");
                            serde_json::json!({ "type": "error", "error": "token validation failed", "id": req_id })
                        }
                    }
                }
            }
            "tunnel_advertisement" => {
                if let Some(iid) = &instance_id {
                    let tunnels: Vec<crate::daemon_registry::ServiceTunnel> = frame["tunnels"]
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
                    let file_tunnels = frame["file_tunnels"].clone();
                    let shell_tunnels = frame["shell_tunnels"].clone();
                    registry.update_all_tunnels(iid, tunnels, file_tunnels, shell_tunnels);

                    // Handle SSH state changes.
                    let ssh_enabled = frame["ssh_enabled"].as_bool().unwrap_or(false);
                    let prev = registry.update_ssh_enabled(iid, ssh_enabled);
                    if let Some(bridge) = &ssh_bridge {
                        if ssh_enabled && !prev {
                            // SSH just enabled.
                            bridge.on_ssh_enabled(iid);
                        } else if ssh_enabled && !registry.has_ssh_port(iid) {
                            // SSH was already enabled but port not yet allocated
                            // (can happen when ssh_bridge wasn't ready during
                            // initial registration).
                            bridge.on_ssh_enabled(iid);
                        } else if !ssh_enabled && prev {
                            bridge.on_ssh_disabled(iid);
                        }
                    }

                    tracing::debug!(%peer_id, %iid, "tunnel advertisement via RPC");
                }
                serde_json::json!({ "type": "ok", "id": req_id })
            }
            _ => {
                tracing::debug!(%peer_id, %msg_type, "unknown RPC request");
                serde_json::json!({ "type": "error", "message": "unknown request", "id": req_id })
            }
        };

        // Write response frame.
        let data = serde_json::to_vec(&resp).unwrap_or_default();
        let _ = writer.write_all(&(data.len() as u32).to_be_bytes()).await;
        let _ = writer.write_all(&data).await;
        let _ = writer.flush().await;
    }

    tracing::info!(%peer_id, "RPC stream closed");
    // Clean up registration, SSH bridge, and gossipsub subscription.
    if let (Some(iid), Some(cat)) = (instance_id, connected_at) {
        let had_ssh_port = registry.unregister(&iid, cat);
        if had_ssh_port.is_some() {
            if let Some(bridge) = &ssh_bridge {
                bridge.on_daemon_disconnect(&iid);
            }
        }
    }
    if let Some(cid) = registered_cluster_id {
        let _ = gossip_tx.send(GossipCmd::Unsubscribe(cid)).await;
    }
}
