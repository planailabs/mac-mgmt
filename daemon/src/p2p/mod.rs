//! Peer-to-peer networking module using libp2p.
//!
//! Provides cluster-scoped peer discovery (mDNS + circuit relay),
//! control protocol (session/metrics/proxy requests), tunnel data
//! streams, and AI proxy load distribution via gossipsub.

pub mod behaviour;
pub mod discovery;
pub mod handler;
pub mod identity;
pub mod protocols;
pub mod proxy_helpers;
pub mod relay_state;
pub mod rpc;
pub mod stream_framing;
pub mod transport;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, PeerId, Swarm, Transport, gossipsub, identify, mdns, request_response};
use tokio::sync::{RwLock, mpsc};

use behaviour::{ClusterBehaviour, ClusterBehaviourEvent};
use discovery::{BackendAdvertisement, PeerRegistry};
use protocols::{ai_proxy, control};
use relay_state::{RelayState, RelayEvent, RelayAction};

/// Commands the daemon event loop can send to the P2pManager.
#[derive(Debug)]
pub enum P2pCommand {
    /// Re-advertise tunnels after a tunnel change.
    AdvertiseTunnels,
    /// Notify that the set of active AI proxy jobs changed.
    UpdateActiveJobs(u32),
}

/// Events the P2pManager emits to the daemon event loop.
#[derive(Debug)]
pub enum P2pEvent {
    /// An AI proxy request from a peer.
    AiProxyRequest {
        peer: PeerId,
        channel: request_response::ResponseChannel<ai_proxy::AiProxyResponse>,
        request: ai_proxy::AiProxyRequest,
    },
    /// A peer discovered or lost.
    PeerUpdate {
        peer: PeerId,
        connected: bool,
    },
}

/// Configuration for the P2pManager.
pub struct P2pConfig {
    pub instance_id: String,
    pub cluster_psk: Option<Vec<u8>>,
    pub relay_multiaddr: Option<Multiaddr>,
    pub mdns_enabled: bool,
    pub p2p_port: u16,
    pub ai_proxy_distribution: bool,
    /// Handler state for processing incoming control requests.
    pub handler_state: Option<Arc<handler::HandlerState>>,
}

/// Manages the libp2p swarm for cluster p2p networking.
pub struct P2pManager {
    cmd_tx: mpsc::Sender<P2pCommand>,
    event_rx: mpsc::Receiver<P2pEvent>,
    /// Shared peer registry for AI proxy load balancing.
    pub peer_registry: Arc<RwLock<PeerRegistry>>,
    /// Local active job count (for advertisements).
    pub active_jobs: Arc<AtomicU32>,
    /// Our PeerId.
    pub local_peer_id: PeerId,
    /// Relay's proxy URL (learned from Identify).
    relay_proxy_url: Arc<RwLock<Option<String>>>,
}

impl P2pManager {
    /// Create and start the P2p swarm.
    pub async fn new(
        host_key: &russh::keys::PrivateKey,
        config: P2pConfig,
    ) -> Result<Self> {
        let keypair =
            identity::keypair_from_russh(host_key).context("failed to convert host key")?;
        let local_peer_id = keypair.public().to_peer_id();
        tracing::info!(%local_peer_id, instance_id = %config.instance_id, "p2p identity ready");

        // Build agent version with PSK auth token if cluster PSK is configured.
        let psk_auth = config.cluster_psk.as_ref().map(|psk| {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(psk);
            hasher.update(local_peer_id.to_bytes());
            let hash = hasher.finalize();
            hex::encode(&hash[..8])
        });
        let agent_version = if let Some(ref auth) = psk_auth {
            format!(
                "mac-mgmt/{}/{}/{}",
                env!("CARGO_PKG_VERSION"),
                config.instance_id,
                auth
            )
        } else {
            format!(
                "mac-mgmt/{}/{}",
                env!("CARGO_PKG_VERSION"),
                config.instance_id
            )
        };
        let _mdns_enabled = config.mdns_enabled;
        let keypair_clone = keypair.clone();

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
                    .map(|(peer, muxer), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(muxer)));
                Ok(ws.boxed())
            })?
            .with_relay_client(
                libp2p::noise::Config::new,
                || libp2p::yamux::Config::default(),
            )?
            .with_behaviour(move |key, relay_client| {
                let identify_config = identify::Config::new(
                    "/mac-mgmt/1.0.0".to_string(),
                    key.public(),
                )
                .with_agent_version(agent_version.clone());

                let mdns_behaviour =
                    mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())
                        .expect("mDNS behaviour");

                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .build()
                    .expect("gossipsub config");
                let gossipsub_behaviour = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(keypair_clone.clone()),
                    gossipsub_config,
                )
                .expect("gossipsub behaviour");

                let ai_proxy_behaviour = request_response::Behaviour::new(
                    [(ai_proxy::PROTOCOL_NAME, request_response::ProtocolSupport::Full)],
                    request_response::Config::default()
                        .with_request_timeout(Duration::from_secs(600)),
                );

                Ok(ClusterBehaviour {
                    ping: libp2p::ping::Behaviour::new(
                        libp2p::ping::Config::new()
                            .with_interval(Duration::from_secs(15)),
                    ),
                    identify: identify::Behaviour::new(identify_config),
                    mdns: mdns_behaviour,
                    relay_client,
                    ai_proxy: ai_proxy_behaviour,
                    gossipsub: gossipsub_behaviour,
                    streams: libp2p_stream::Behaviour::new(),
                })
            })?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(3600)))
            .build();

        // Listen on QUIC (both IPv4 and IPv6)
        let quic_v4: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", config.p2p_port)
            .parse()
            .context("invalid QUIC listen address")?;
        swarm.listen_on(quic_v4).context("failed to listen on QUIC IPv4")?;
        let quic_v6: Multiaddr = format!("/ip6/::/udp/{}/quic-v1", config.p2p_port)
            .parse()
            .context("invalid QUIC IPv6 listen address")?;
        swarm.listen_on(quic_v6).context("failed to listen on QUIC IPv6")?;

        // Connect to relay if configured
        if let Some(ref relay_addr) = config.relay_multiaddr {
            tracing::info!(%relay_addr, "dialing relay node");
            if let Err(e) = swarm.dial(relay_addr.clone()) {
                tracing::warn!("failed to dial relay: {e}");
            }
        }

        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let (event_tx, event_rx) = mpsc::channel(256);
        let peer_registry = Arc::new(RwLock::new(PeerRegistry::default()));
        let active_jobs = Arc::new(AtomicU32::new(0));
        let relay_proxy_url = Arc::new(RwLock::new(None));

        // Extract stream control for opening RPC streams.
        let stream_control = swarm.behaviour().streams.new_control();

        // Accept incoming tunnel data substreams from the relay.
        let tunnel_protocol = libp2p::StreamProtocol::new("/mac-mgmt/tunnel/1.0.0");
        let mut incoming_tunnels = stream_control.clone().accept(tunnel_protocol).unwrap();
        if let Some(hs) = &config.handler_state {
            let handler = Arc::clone(hs);
            tokio::spawn(async move {
                while let Some((peer_id, stream)) = incoming_tunnels.next().await {
                    let h = Arc::clone(&handler);
                    tokio::spawn(async move {
                        handle_tunnel_stream(peer_id, stream, &h).await;
                    });
                }
            });
        }

        let peer_registry_clone = Arc::clone(&peer_registry);
        let active_jobs_clone = Arc::clone(&active_jobs);
        let relay_proxy_url_clone = Arc::clone(&relay_proxy_url);
        tokio::spawn(swarm_loop(
            swarm,
            cmd_rx,
            event_tx,
            peer_registry_clone,
            active_jobs_clone,
            relay_proxy_url_clone,
            stream_control,
            config,
        ));

        Ok(Self {
            cmd_tx,
            event_rx,
            peer_registry,
            active_jobs,
            local_peer_id,
            relay_proxy_url,
        })
    }

    /// Receive the next event from the p2p swarm.
    pub async fn recv_event(&mut self) -> Option<P2pEvent> {
        self.event_rx.recv().await
    }

    /// Get the relay's proxy URL (learned from Identify).
    pub fn relay_proxy_url(&self) -> Option<String> {
        self.relay_proxy_url.try_read().ok()?.clone()
    }

    /// Send a command to the swarm.
    pub async fn send_cmd(&self, cmd: P2pCommand) {
        let _ = self.cmd_tx.send(cmd).await;
    }
}

/// Main swarm event loop — runs as a background task.
async fn swarm_loop(
    mut swarm: Swarm<ClusterBehaviour>,
    mut cmd_rx: mpsc::Receiver<P2pCommand>,
    event_tx: mpsc::Sender<P2pEvent>,
    peer_registry: Arc<RwLock<PeerRegistry>>,
    active_jobs: Arc<AtomicU32>,
    relay_proxy_url: Arc<RwLock<Option<String>>>,
    stream_control: libp2p_stream::Control,
    config: P2pConfig,
) {
    let mut ad_interval = tokio::time::interval(Duration::from_secs(30));
    let mut evict_interval = tokio::time::interval(Duration::from_secs(15));
    let mut relay_tick = tokio::time::interval(Duration::from_secs(10));
    let mut authorized_cluster_peers: std::collections::HashSet<PeerId> = std::collections::HashSet::new();

    // Initialize relay state machine.
    let mut relay = RelayState::new(config.relay_multiaddr.clone());

    let cluster_topic = gossipsub::IdentTopic::new(format!(
        "mac-mgmt/cluster/{}", config.instance_id
    ));
    if let Err(e) = swarm.behaviour_mut().gossipsub.subscribe(&cluster_topic) {
        tracing::warn!("failed to subscribe to cluster topic: {e}");
    }

    loop {
        // If the relay has an RPC stream, poll it for incoming messages.
        let rpc_recv = async {
            if let Some(rpc) = relay.rpc() {
                rpc.recv().await
            } else {
                // No RPC stream — sleep forever (this arm won't fire).
                std::future::pending().await
            }
        };

        tokio::select! {
            event = swarm.select_next_some() => {
                // Convert swarm events to relay state events + handle general events.
                let relay_event = match &event {
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        Some(RelayEvent::ConnectionEstablished { peer_id: *peer_id })
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        authorized_cluster_peers.remove(peer_id);
                        Some(RelayEvent::ConnectionClosed { peer_id: *peer_id })
                    }
                    SwarmEvent::Behaviour(ClusterBehaviourEvent::Identify(
                        identify::Event::Received { peer_id, info, .. }
                    )) => {
                        // PSK auth check.
                        if let Some(psk) = &config.cluster_psk {
                            if !verify_psk_auth(&info.agent_version, *peer_id, psk) {
                                tracing::warn!(%peer_id, "PSK auth failed, disconnecting");
                                let _ = swarm.disconnect_peer_id(*peer_id);
                                continue;
                            }
                        }
                        authorized_cluster_peers.insert(*peer_id);

                        for addr in &info.listen_addrs {
                            swarm.add_peer_address(*peer_id, addr.clone());
                        }

                        Some(RelayEvent::Identified {
                            peer_id: *peer_id,
                            agent_version: info.agent_version.clone(),
                        })
                    }
                    _ => None,
                };

                // Feed relay state machine.
                if let Some(re) = relay_event {
                    let actions = relay.handle_event(re);
                    execute_relay_actions(
                        actions, &mut swarm, &relay_proxy_url, &mut authorized_cluster_peers,
                    ).await;
                }

                // Handle non-relay swarm events.
                handle_general_event(
                    event, &event_tx, &peer_registry, &mut swarm,
                    &authorized_cluster_peers,
                ).await;
            }

            rpc_msg = rpc_recv => {
                match rpc_msg {
                    Ok(rpc::RpcMessage::Request { payload }) => {
                        // Relay-initiated request (proxy, metrics, etc.)
                        if let Some(hs) = &config.handler_state {
                            handle_rpc_request(payload, hs, &mut relay).await;
                        }
                    }
                    Ok(rpc::RpcMessage::Response { .. }) => {
                        // Response to one of our requests — already dispatched by RpcStream.
                    }
                    Err(e) => {
                        tracing::warn!("RPC stream error: {e}, relay will reconnect");
                        let actions = relay.handle_event(RelayEvent::ConnectionClosed {
                            peer_id: relay.peer_id().unwrap_or(PeerId::random()),
                        });
                        execute_relay_actions(
                            actions, &mut swarm, &relay_proxy_url, &mut authorized_cluster_peers,
                        ).await;
                    }
                }
            }

            Some(cmd) = cmd_rx.recv() => {
                handle_command(cmd, &mut swarm, &cluster_topic, &active_jobs, &relay).await;
            }

            _ = ad_interval.tick() => {
                publish_advertisement(&mut swarm, &cluster_topic, &active_jobs).await;
            }
            _ = evict_interval.tick() => {
                peer_registry.write().await.evict_stale();
            }
            _ = relay_tick.tick() => {
                // Drive relay state machine tick (reconnect, re-register).
                let actions = relay.handle_event(RelayEvent::Tick);
                execute_relay_actions(
                    actions, &mut swarm, &relay_proxy_url, &mut authorized_cluster_peers,
                ).await;

                // If Identified but no RPC stream yet, try opening one.
                if matches!(relay, RelayState::Identified { .. }) {
                    if let Some(peer_id) = relay.peer_id() {
                        let mut ctrl = stream_control.clone();
                        match ctrl.open_stream(peer_id, rpc::RPC_PROTOCOL).await {
                            Ok(stream) => {
                                let actions = relay.handle_event(RelayEvent::RpcStreamOpened { stream });
                                execute_relay_actions(
                                    actions, &mut swarm, &relay_proxy_url, &mut authorized_cluster_peers,
                                ).await;
                            }
                            Err(e) => {
                                tracing::warn!("failed to open RPC stream: {e}");
                            }
                        }
                    }
                }

                // Re-register via RPC if needed.
                if relay.needs_reregister() {
                    if let Some(rpc) = relay.rpc() {
                        let reg = serde_json::json!({
                            "type": "register",
                            "instance_id": config.instance_id,
                            "hostname": hostname::get().ok().map(|h| h.to_string_lossy().to_string()),
                        });
                        if let Err(e) = rpc.send(reg).await {
                            tracing::warn!("re-register RPC failed: {e}");
                        } else {
                            send_tunnel_advertisement_rpc(rpc, &config.handler_state).await;
                            relay.mark_registered();
                        }
                    }
                }
            }
        }
    }
}

use futures_util::StreamExt;

/// Execute actions returned by the relay state machine.
async fn execute_relay_actions(
    actions: Vec<RelayAction>,
    swarm: &mut Swarm<ClusterBehaviour>,
    relay_proxy_url: &Arc<RwLock<Option<String>>>,
    authorized_cluster_peers: &mut std::collections::HashSet<PeerId>,
) {
    for action in actions {
        match action {
            RelayAction::Dial(addr) => {
                tracing::info!(%addr, "dialing relay");
                if let Err(e) = swarm.dial(addr) {
                    tracing::warn!("failed to dial relay: {e}");
                }
            }
            RelayAction::OpenRpcStream(_) => {
                // Handled in the relay_tick branch where we have &mut relay.
            }
            RelayAction::SetProxyUrl(url) => {
                *relay_proxy_url.write().await = url;
            }
            RelayAction::AuthorizePeer(peer_id) => {
                authorized_cluster_peers.insert(peer_id);
            }
            RelayAction::DeauthorizePeer(peer_id) => {
                authorized_cluster_peers.remove(&peer_id);
            }
            RelayAction::Log(level, msg) => {
                match level {
                    tracing::Level::INFO => tracing::info!("{msg}"),
                    tracing::Level::WARN => tracing::warn!("{msg}"),
                    tracing::Level::DEBUG => tracing::debug!("{msg}"),
                    _ => tracing::trace!("{msg}"),
                }
            }
        }
    }
}

/// Handle non-relay swarm events (mDNS, control requests, AI proxy, gossipsub).
async fn handle_general_event(
    event: SwarmEvent<ClusterBehaviourEvent>,
    event_tx: &mpsc::Sender<P2pEvent>,
    peer_registry: &Arc<RwLock<PeerRegistry>>,
    swarm: &mut Swarm<ClusterBehaviour>,
    authorized_cluster_peers: &std::collections::HashSet<PeerId>,
) {
    match event {
        SwarmEvent::Behaviour(ClusterBehaviourEvent::Mdns(mdns::Event::Discovered(peers))) => {
            for (peer_id, addr) in peers {
                tracing::info!(%peer_id, %addr, "mDNS discovered peer");
                swarm.add_peer_address(peer_id, addr);
                let _ = event_tx.send(P2pEvent::PeerUpdate { peer: peer_id, connected: true }).await;
            }
        }
        SwarmEvent::Behaviour(ClusterBehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
            for (peer_id, _addr) in peers {
                peer_registry.write().await.remove(&peer_id);
                let _ = event_tx.send(P2pEvent::PeerUpdate { peer: peer_id, connected: false }).await;
            }
        }
        // Control requests are now handled via the persistent RPC stream,
        // not via request_response. The control behaviour has been removed.
        SwarmEvent::Behaviour(ClusterBehaviourEvent::AiProxy(
            request_response::Event::Message {
                peer,
                message: request_response::Message::Request { channel, request, .. },
                ..
            },
        )) => {
            if !authorized_cluster_peers.contains(&peer) {
                tracing::warn!(%peer, "rejecting AI proxy request from unauthorized peer");
                return;
            }
            let _ = event_tx.send(P2pEvent::AiProxyRequest { peer, channel, request }).await;
        }
        SwarmEvent::Behaviour(ClusterBehaviourEvent::Gossipsub(
            gossipsub::Event::Message { message, .. },
        )) => {
            if let Ok(ad) = serde_json::from_slice::<BackendAdvertisement>(&message.data) {
                if let Ok(peer_id) = ad.peer_id.parse::<PeerId>() {
                    peer_registry.write().await.update(peer_id, ad);
                }
            }
        }
        SwarmEvent::NewListenAddr { address, .. } => {
            tracing::info!(%address, "listening on");
        }
        _ => {}
    }
}

/// Handle an incoming RPC request from the relay.
async fn handle_rpc_request(
    payload: serde_json::Value,
    handler_state: &Arc<handler::HandlerState>,
    relay: &mut RelayState,
) {
    let msg_type = payload["type"].as_str().unwrap_or("");
    let _request_id = payload["request_id"].as_str().unwrap_or("");

    // Parse into ControlRequest and handle.
    let request: Result<control::ControlRequest, _> = serde_json::from_value(payload.clone());
    match request {
        Ok(req) => {
            let response = handler::handle_control_request(handler_state, req).await;
            // Send response back on the RPC stream.
            if let Some(rpc) = relay.rpc() {
                let mut resp_json = serde_json::to_value(&response).unwrap_or_default();
                resp_json["id"] = payload["id"].clone(); // echo the request id
                if let Err(e) = rpc.send(resp_json).await {
                    tracing::warn!("failed to send RPC response: {e}");
                }
            }
        }
        Err(e) => {
            tracing::warn!("failed to parse relay RPC request ({msg_type}): {e}");
        }
    }
}

async fn handle_command(
    cmd: P2pCommand,
    swarm: &mut Swarm<ClusterBehaviour>,
    cluster_topic: &gossipsub::IdentTopic,
    active_jobs: &Arc<AtomicU32>,
    _relay: &RelayState,
) {
    match cmd {
        P2pCommand::AdvertiseTunnels => {
            publish_advertisement(swarm, cluster_topic, active_jobs).await;
        }
        P2pCommand::UpdateActiveJobs(count) => {
            active_jobs.store(count, Ordering::Relaxed);
        }
    }
}

/// Send tunnel advertisement via the RPC stream.
async fn send_tunnel_advertisement_rpc(
    rpc: &mut rpc::RpcStream,
    handler_state: &Option<Arc<handler::HandlerState>>,
) {
    let Some(hs) = handler_state else { return };

    let tunnel_defs = hs.tunnel_defs.read().await;
    let tunnels: Vec<serde_json::Value> = tunnel_defs
        .iter()
        .map(|(name, target)| serde_json::json!({ "name": name, "port": target.port }))
        .collect();
    drop(tunnel_defs);

    #[cfg(feature = "services")]
    let file_tunnels = {
        let reg = hs.file_tunnel_registry.read().await;
        serde_json::Value::Array(reg.to_json())
    };
    #[cfg(not(feature = "services"))]
    let file_tunnels = serde_json::Value::Array(vec![]);

    #[cfg(feature = "services")]
    let shell_tunnels = {
        let reg = hs.shell_tunnel_registry.read().await;
        serde_json::Value::Array(reg.to_json())
    };
    #[cfg(not(feature = "services"))]
    let shell_tunnels = serde_json::Value::Array(vec![]);

    let req = serde_json::json!({
        "type": "tunnel_advertisement",
        "tunnels": tunnels,
        "file_tunnels": file_tunnels,
        "shell_tunnels": shell_tunnels,
    });
    if let Err(e) = rpc.send(req).await {
        tracing::warn!("failed to send tunnel advertisement via RPC: {e}");
    }
}

/// Handle an incoming tunnel data substream from the relay.
/// Reads a JSON handshake with the proxy request details, makes the
/// local HTTP request, and streams the response back using stream_framing.
async fn handle_tunnel_stream(
    peer_id: PeerId,
    mut stream: libp2p::Stream,
    handler_state: &Arc<handler::HandlerState>,
) {
    use futures_util::{AsyncReadExt, AsyncWriteExt};

    // Read the handshake frame (JSON with request details).
    let mut len_buf = [0u8; 4];
    if stream.read_exact(&mut len_buf).await.is_err() {
        return;
    }
    let len = u32::from_be_bytes(len_buf);
    if len > 1024 * 1024 {
        return;
    }
    let mut buf = vec![0u8; len as usize];
    if stream.read_exact(&mut buf).await.is_err() {
        return;
    }

    let Ok(handshake) = serde_json::from_slice::<serde_json::Value>(&buf) else {
        return;
    };

    let msg_type = handshake["type"].as_str().unwrap_or("");
    match msg_type {
        "proxy" => {
            handle_streamed_proxy(handshake, &mut stream, handler_state).await;
        }
        "metrics" => {
            handle_streamed_metrics(handshake, &mut stream, handler_state).await;
        }
        "ssh" => {
            handle_ssh_session(stream, handler_state).await;
            return; // stream consumed by SSH, don't close
        }
        _ => {
            tracing::warn!(%peer_id, %msg_type, "unknown tunnel handshake type");
        }
    }

    let _ = stream.close().await;
}

/// Handle a streamed proxy request over a tunnel substream.
async fn handle_streamed_proxy(
    handshake: serde_json::Value,
    stream: &mut libp2p::Stream,
    handler_state: &Arc<handler::HandlerState>,
) {
    use crate::p2p::stream_framing;

    let tunnel_name = handshake["tunnel_name"].as_str().unwrap_or("");
    let method = handshake["method"].as_str().unwrap_or("GET");
    let path = handshake["path"].as_str().unwrap_or("/");

    // WebSocket proxy: bridge the tunnel substream to a local WS connection.
    if method == "WEBSOCKET" {
        let tunnel_defs = handler_state.tunnel_defs.read().await;
        let Some(target) = tunnel_defs.get(tunnel_name).cloned() else {
            let err = serde_json::json!({ "status": 404, "error": "tunnel not found" });
            let _ = stream_framing::write_json(stream, &err).await;
            let _ = stream_framing::write_end(stream).await;
            return;
        };
        drop(tunnel_defs);

        let ws_url = format!("ws://{}:{}{path}", target.host, target.port);
        tracing::debug!("WS proxy: connecting to {ws_url}");

        let connect_result = {
            use tokio_tungstenite::tungstenite::client::IntoClientRequest;
            let mut request = ws_url.into_client_request().unwrap();
            if handler_state.fake_origin_local {
                proxy_helpers::apply_fake_origin(request.headers_mut(), &target);
            }
            tokio_tungstenite::connect_async(request).await
        };
        let (local_ws, _) = match connect_result {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!("WS proxy connect failed: {e}");
                let err = serde_json::json!({ "status": 502, "error": format!("ws connect failed: {e}") });
                let _ = stream_framing::write_json(stream, &err).await;
                let _ = stream_framing::write_end(stream).await;
                return;
            }
        };

        // Bridge: tunnel substream (framed messages from relay) ↔ local WS.
        // Uses the same tag+length framing protocol as relay's ws_bridge:
        //   0x01 + len + data = Text, 0x02 + len + data = Binary, 0x03 = Close.
        use futures_util::{SinkExt, StreamExt, AsyncWriteExt as _};
        use tokio_tungstenite::tungstenite;

        const TAG_TEXT: u8 = 0x01;
        const TAG_BINARY: u8 = 0x02;
        const TAG_CLOSE: u8 = 0x03;

        let (mut ws_sink, mut ws_stream) = local_ws.split();

        loop {
            tokio::select! {
                // Tunnel substream → local WS (read framed messages)
                tag_result = async {
                    use futures_util::AsyncReadExt;
                    let mut tag = [0u8; 1];
                    stream.read_exact(&mut tag).await.map(|_| tag[0])
                } => {
                    let Ok(tag) = tag_result else { break };
                    match tag {
                        TAG_TEXT => {
                            use futures_util::AsyncReadExt;
                            let mut len_buf = [0u8; 4];
                            if stream.read_exact(&mut len_buf).await.is_err() { break; }
                            let len = u32::from_be_bytes(len_buf) as usize;
                            if len > 16 * 1024 * 1024 { break; }
                            let mut data = vec![0u8; len];
                            if stream.read_exact(&mut data).await.is_err() { break; }
                            let text = String::from_utf8_lossy(&data);
                            if ws_sink.send(tungstenite::Message::text(text.as_ref())).await.is_err() { break; }
                        }
                        TAG_BINARY => {
                            use futures_util::AsyncReadExt;
                            let mut len_buf = [0u8; 4];
                            if stream.read_exact(&mut len_buf).await.is_err() { break; }
                            let len = u32::from_be_bytes(len_buf) as usize;
                            if len > 16 * 1024 * 1024 { break; }
                            let mut data = vec![0u8; len];
                            if stream.read_exact(&mut data).await.is_err() { break; }
                            if ws_sink.send(tungstenite::Message::Binary(data.into())).await.is_err() { break; }
                        }
                        TAG_CLOSE => break,
                        _ => break,
                    }
                }
                // Local WS → tunnel substream (write framed messages)
                msg = ws_stream.next() => {
                    match msg {
                        Some(Ok(tungstenite::Message::Text(text))) => {
                            let bytes = text.as_bytes();
                            if stream.write_all(&[TAG_TEXT]).await.is_err()
                                || stream.write_all(&(bytes.len() as u32).to_be_bytes()).await.is_err()
                                || stream.write_all(bytes).await.is_err()
                            { break; }
                            let _ = stream.flush().await;
                        }
                        Some(Ok(tungstenite::Message::Binary(data))) => {
                            if stream.write_all(&[TAG_BINARY]).await.is_err()
                                || stream.write_all(&(data.len() as u32).to_be_bytes()).await.is_err()
                                || stream.write_all(&data).await.is_err()
                            { break; }
                            let _ = stream.flush().await;
                        }
                        Some(Ok(tungstenite::Message::Ping(data))) => {
                            // Respond locally.
                            let _ = ws_sink.send(tungstenite::Message::Pong(data)).await;
                        }
                        Some(Ok(tungstenite::Message::Pong(_))) => {}
                        Some(Ok(tungstenite::Message::Close(_))) | None => {
                            let _ = stream.write_all(&[TAG_CLOSE]).await;
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        let _ = ws_sink.send(tungstenite::Message::Close(None)).await;
        return;
    }

    let headers: Vec<(String, String)> = handshake["headers"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let pair = v.as_array()?;
                    Some((
                        pair.first()?.as_str()?.to_string(),
                        pair.get(1)?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let body_b64 = handshake["body"].as_str().map(String::from);

    let tunnel_defs = handler_state.tunnel_defs.read().await;
    let Some(target) = tunnel_defs.get(tunnel_name).cloned() else {
        let err = serde_json::json!({ "status": 404, "error": "tunnel not found" });
        let _ = stream_framing::write_json(stream, &err).await;
        let _ = stream_framing::write_end(stream).await;
        return;
    };
    drop(tunnel_defs);

    let req = proxy_helpers::build_proxy_request(
        &handler_state.client,
        &target,
        method,
        path,
        handler_state.fake_origin_local,
    );
    let req = proxy_helpers::apply_headers_vec(req, &headers, handler_state.fake_origin_local, &target);
    let req = proxy_helpers::apply_body_b64(req, body_b64);

    match req.timeout(std::time::Duration::from_secs(300)).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let resp_headers: Vec<(String, String)> = resp
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();

            // Send response header frame.
            let header = serde_json::json!({
                "status": status,
                "headers": resp_headers,
            });
            if stream_framing::write_json(stream, &header).await.is_err() {
                return;
            }

            // Stream body chunks.
            use futures_util::StreamExt;
            let mut byte_stream = resp.bytes_stream();
            while let Some(chunk) = byte_stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        if stream_framing::write_binary(stream, &bytes).await.is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        tracing::debug!("proxy stream chunk error: {e}");
                        break;
                    }
                }
            }

            let _ = stream_framing::write_end(stream).await;
        }
        Err(e) => {
            let err = serde_json::json!({ "status": 502, "error": format!("proxy error: {e}") });
            let _ = stream_framing::write_json(stream, &err).await;
            let _ = stream_framing::write_end(stream).await;
        }
    }
}

/// Handle a streamed metrics request over a tunnel substream.
async fn handle_streamed_metrics(
    handshake: serde_json::Value,
    stream: &mut libp2p::Stream,
    handler_state: &Arc<handler::HandlerState>,
) {
    use crate::p2p::stream_framing;

    let path = handshake["path"].as_str().unwrap_or("/metrics");
    let url = format!("http://127.0.0.1:{}{path}", handler_state.metrics_port);

    match handler_state
        .client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let content_type = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("text/plain")
                .to_string();
            let body = resp.text().await.unwrap_or_default();

            let header = serde_json::json!({
                "status": status,
                "content_type": content_type,
            });
            if stream_framing::write_json(stream, &header).await.is_err() {
                return;
            }
            let _ = stream_framing::write_binary(stream, body.as_bytes()).await;
            let _ = stream_framing::write_end(stream).await;
        }
        Err(e) => {
            let err = serde_json::json!({ "status": 502, "error": format!("metrics error: {e}") });
            let _ = stream_framing::write_json(stream, &err).await;
            let _ = stream_framing::write_end(stream).await;
        }
    }
}

/// Handle an SSH session over a tunnel substream.
/// The substream acts as the transport for the russh SSH server.
async fn handle_ssh_session(
    stream: libp2p::Stream,
    handler_state: &Arc<handler::HandlerState>,
) {
    use tokio_util::compat::FuturesAsyncReadCompatExt;

    // Load authorized SSH keys.
    let authorized_keys = {
        let keys = handler_state.ssh_allowed.load(Ordering::Relaxed);
        if !keys {
            tracing::warn!("SSH session rejected: SSH access disabled");
            return;
        }
        crate::remote_ssh::ssh_server::load_authorized_keys()
    };

    let config = std::sync::Arc::new(russh::server::Config {
        keys: vec![
            crate::host_keys::load_or_generate()
                .expect("failed to load host key for SSH"),
        ],
        ..Default::default()
    });

    let session = crate::remote_ssh::ssh_server::SshSession::new(authorized_keys);

    // Wrap the libp2p stream (futures AsyncRead/Write) into tokio AsyncRead/Write.
    let compat_stream = stream.compat();

    tracing::info!("SSH session started via libp2p tunnel");
    match russh::server::run_stream(config, compat_stream, session).await {
        Ok(running) => {
            // Wait for the session to finish.
            let _ = running.await;
            tracing::info!("SSH session ended");
        }
        Err(e) => {
            tracing::warn!("SSH session failed: {e}");
        }
    }
}

/// Parse the relay's proxy_url from its Identify agent version string.
fn parse_relay_proxy_url(agent_version: &str) -> Option<String> {
    let parts: Vec<&str> = agent_version.splitn(3, '/').collect();
    let b64 = match parts.as_slice() {
        [_, _, b64] if !b64.is_empty() => *b64,
        _ => return None,
    };
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(b64)
        .ok()?;
    String::from_utf8(bytes).ok()
}

/// Verify the PSK auth token in a peer's agent version string.
fn verify_psk_auth(agent_version: &str, peer_id: PeerId, psk: &[u8]) -> bool {
    let parts: Vec<&str> = agent_version.splitn(4, '/').collect();
    let auth_token = match parts.as_slice() {
        [_, _, _, auth] => *auth,
        _ => return false,
    };
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(psk);
    hasher.update(peer_id.to_bytes());
    let hash = hasher.finalize();
    let expected = hex::encode(&hash[..8]);
    auth_token == expected
}

async fn publish_advertisement(
    swarm: &mut Swarm<ClusterBehaviour>,
    topic: &gossipsub::IdentTopic,
    active_jobs: &Arc<AtomicU32>,
) {
    let local_peer = *swarm.local_peer_id();
    let ad = BackendAdvertisement {
        peer_id: local_peer.to_string(),
        backends: vec![],
        active_jobs: active_jobs.load(Ordering::Relaxed),
        models: vec![],
    };
    if let Ok(data) = serde_json::to_vec(&ad) {
        if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), data) {
            tracing::trace!("gossipsub publish failed (likely no subscribers): {e}");
        }
    }
}
