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
pub mod stream_framing;
pub mod transport;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, PeerId, Swarm, gossipsub, identify, mdns, request_response};
use tokio::sync::{RwLock, mpsc};

use behaviour::{ClusterBehaviour, ClusterBehaviourEvent};
use discovery::{BackendAdvertisement, PeerRegistry};
use protocols::{ai_proxy, control};

/// Commands the daemon event loop can send to the P2pManager.
#[derive(Debug)]
pub enum P2pCommand {
    /// Re-advertise tunnels after a tunnel change.
    AdvertiseTunnels,
    /// Notify that the set of active AI proxy jobs changed.
    UpdateActiveJobs(u32),
    /// Register with the relay (sent after connecting).
    RegisterWithRelay {
        instance_id: String,
        cluster_id: Option<String>,
        hostname: Option<String>,
    },
    /// Send tunnel definitions to the relay.
    SendTunnelAdvertisement {
        tunnels: serde_json::Value,
        file_tunnels: serde_json::Value,
        shell_tunnels: serde_json::Value,
    },
}

/// Events the P2pManager emits to the daemon event loop.
#[derive(Debug)]
pub enum P2pEvent {
    /// A control request from a peer (relay or another daemon).
    ControlRequest {
        peer: PeerId,
        channel: request_response::ResponseChannel<control::ControlResponse>,
        request: control::ControlRequest,
    },
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
        // Format: "mac-mgmt/{version}/{instance_id}/{psk_auth}" where psk_auth is
        // HMAC-SHA256(psk, peer_id) truncated to 16 hex chars. Peers verify this
        // on Identify to reject connections from unauthorized clusters.
        let psk_auth = config.cluster_psk.as_ref().map(|psk| {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(psk);
            hasher.update(local_peer_id.to_bytes());
            let hash = hasher.finalize();
            hex::encode(&hash[..8]) // 16 hex chars
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
            .with_quic()
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

                let control_behaviour = request_response::Behaviour::new(
                    [(control::PROTOCOL_NAME, request_response::ProtocolSupport::Full)],
                    request_response::Config::default()
                        .with_request_timeout(Duration::from_secs(30)),
                );

                let ai_proxy_behaviour = request_response::Behaviour::new(
                    [(ai_proxy::PROTOCOL_NAME, request_response::ProtocolSupport::Full)],
                    request_response::Config::default()
                        .with_request_timeout(Duration::from_secs(600)),
                );

                Ok(ClusterBehaviour {
                    identify: identify::Behaviour::new(identify_config),
                    mdns: mdns_behaviour,
                    relay_client,
                    control: control_behaviour,
                    ai_proxy: ai_proxy_behaviour,
                    gossipsub: gossipsub_behaviour,
                    streams: libp2p_stream::Behaviour::new(),
                })
            })?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        // Listen on QUIC
        let quic_addr: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", config.p2p_port)
            .parse()
            .context("invalid QUIC listen address")?;
        swarm
            .listen_on(quic_addr)
            .context("failed to listen on QUIC")?;

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

        // Spawn the swarm event loop
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
    config: P2pConfig,
) {
    let mut ad_interval = tokio::time::interval(Duration::from_secs(30));
    let mut evict_interval = tokio::time::interval(Duration::from_secs(15));
    let mut relay_peer_id: Option<PeerId> = None;

    // Subscribe to cluster gossipsub topic
    let cluster_topic = gossipsub::IdentTopic::new(format!(
        "mac-mgmt/cluster/{}", config.instance_id
    ));
    if let Err(e) = swarm.behaviour_mut().gossipsub.subscribe(&cluster_topic) {
        tracing::warn!("failed to subscribe to cluster topic: {e}");
    }

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                handle_swarm_event(
                    event,
                    &event_tx,
                    &peer_registry,
                    &mut swarm,
                    &config.handler_state,
                    &config.cluster_psk,
                    &relay_proxy_url,
                    &mut relay_peer_id,
                ).await;
            }
            Some(cmd) = cmd_rx.recv() => {
                handle_command(cmd, &mut swarm, &cluster_topic, &active_jobs, &relay_peer_id).await;
            }
            _ = ad_interval.tick() => {
                // Periodically publish our backend advertisement
                publish_advertisement(&mut swarm, &cluster_topic, &active_jobs).await;
            }
            _ = evict_interval.tick() => {
                peer_registry.write().await.evict_stale();
            }
        }
    }
}

use futures_util::StreamExt;

async fn handle_swarm_event(
    event: SwarmEvent<ClusterBehaviourEvent>,
    event_tx: &mpsc::Sender<P2pEvent>,
    peer_registry: &Arc<RwLock<PeerRegistry>>,
    swarm: &mut Swarm<ClusterBehaviour>,
    handler_state: &Option<Arc<handler::HandlerState>>,
    cluster_psk: &Option<Vec<u8>>,
    relay_proxy_url: &Arc<RwLock<Option<String>>>,
    relay_peer_id: &mut Option<PeerId>,
) {
    match event {
        SwarmEvent::Behaviour(ClusterBehaviourEvent::Mdns(mdns::Event::Discovered(peers))) => {
            for (peer_id, addr) in peers {
                tracing::info!(%peer_id, %addr, "mDNS discovered peer");
                swarm.add_peer_address(peer_id, addr);
                let _ = event_tx
                    .send(P2pEvent::PeerUpdate {
                        peer: peer_id,
                        connected: true,
                    })
                    .await;
            }
        }
        SwarmEvent::Behaviour(ClusterBehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
            for (peer_id, _addr) in peers {
                tracing::info!(%peer_id, "mDNS peer expired");
                peer_registry.write().await.remove(&peer_id);
                let _ = event_tx
                    .send(P2pEvent::PeerUpdate {
                        peer: peer_id,
                        connected: false,
                    })
                    .await;
            }
        }
        SwarmEvent::Behaviour(ClusterBehaviourEvent::Identify(identify::Event::Received {
            peer_id,
            info,
            ..
        })) => {
            tracing::debug!(%peer_id, agent = %info.agent_version, "identify received");

            // Verify PSK auth token if we have a cluster PSK configured.
            if let Some(psk) = cluster_psk {
                if !verify_psk_auth(&info.agent_version, peer_id, psk) {
                    tracing::warn!(%peer_id, "PSK auth failed, disconnecting peer");
                    let _ = swarm.disconnect_peer_id(peer_id);
                    return;
                }
            }

            // If this is a relay (agent starts with "mac-mgmt-relay/"), parse proxy_url
            // and track the relay PeerId.
            if info.agent_version.starts_with("mac-mgmt-relay/") {
                *relay_peer_id = Some(peer_id);
                if let Some(url) = parse_relay_proxy_url(&info.agent_version) {
                    tracing::info!(%peer_id, %url, "relay proxy URL learned from Identify");
                    *relay_proxy_url.write().await = Some(url);
                }
            }

            for addr in info.listen_addrs {
                swarm.add_peer_address(peer_id, addr);
            }
        }
        SwarmEvent::Behaviour(ClusterBehaviourEvent::Control(
            request_response::Event::Message {
                peer,
                message:
                    request_response::Message::Request {
                        channel, request, ..
                    },
                ..
            },
        )) => {
            // Handle control requests directly if handler state is available.
            if let Some(hs) = handler_state {
                let hs = Arc::clone(hs);
                let response = handler::handle_control_request(&hs, request).await;
                let _ = swarm.behaviour_mut().control.send_response(channel, response);
            } else {
                let _ = event_tx
                    .send(P2pEvent::ControlRequest {
                        peer,
                        channel,
                        request,
                    })
                    .await;
            }
        }
        SwarmEvent::Behaviour(ClusterBehaviourEvent::AiProxy(
            request_response::Event::Message {
                peer,
                message:
                    request_response::Message::Request {
                        channel, request, ..
                    },
                ..
            },
        )) => {
            let _ = event_tx
                .send(P2pEvent::AiProxyRequest {
                    peer,
                    channel,
                    request,
                })
                .await;
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
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            tracing::info!(%peer_id, "connection established");
        }
        SwarmEvent::ConnectionClosed { peer_id, .. } => {
            tracing::debug!(%peer_id, "connection closed");
        }
        _ => {}
    }
}

async fn handle_command(
    cmd: P2pCommand,
    swarm: &mut Swarm<ClusterBehaviour>,
    cluster_topic: &gossipsub::IdentTopic,
    active_jobs: &Arc<AtomicU32>,
    relay_peer_id: &Option<PeerId>,
) {
    match cmd {
        P2pCommand::AdvertiseTunnels => {
            publish_advertisement(swarm, cluster_topic, active_jobs).await;
        }
        P2pCommand::UpdateActiveJobs(count) => {
            active_jobs.store(count, Ordering::Relaxed);
        }
        P2pCommand::RegisterWithRelay {
            instance_id,
            cluster_id,
            hostname,
        } => {
            if let Some(relay) = relay_peer_id {
                let req = control::ControlRequest::Register {
                    instance_id,
                    cluster_id,
                    hostname,
                    agent_name: None,
                };
                let _req_id = swarm.behaviour_mut().control.send_request(relay, req);
                tracing::info!(%relay, "sent registration to relay");
            }
        }
        P2pCommand::SendTunnelAdvertisement {
            tunnels,
            file_tunnels,
            shell_tunnels,
        } => {
            if let Some(relay) = relay_peer_id {
                let req = control::ControlRequest::TunnelAdvertisement {
                    tunnels,
                    file_tunnels,
                    shell_tunnels,
                };
                let _req_id = swarm.behaviour_mut().control.send_request(relay, req);
                tracing::debug!(%relay, "sent tunnel advertisement to relay");
            }
        }
    }
}

/// Parse the relay's proxy_url from its Identify agent version string.
///
/// Agent version format: `mac-mgmt-relay/{version}/{proxy_url_base64}`
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
///
/// Agent version format: `mac-mgmt/{version}/{instance_id}/{auth_hex}`
/// where auth_hex = hex(SHA256(psk || peer_id_bytes))[..16].
fn verify_psk_auth(agent_version: &str, peer_id: PeerId, psk: &[u8]) -> bool {
    let parts: Vec<&str> = agent_version.splitn(4, '/').collect();
    let auth_token = match parts.as_slice() {
        [_, _, _, auth] => *auth,
        _ => return false, // no auth token in agent string
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
        backends: vec![], // TODO: populated from service manager
        active_jobs: active_jobs.load(Ordering::Relaxed),
        models: vec![], // TODO: populated from service manager
    };

    if let Ok(data) = serde_json::to_vec(&ad) {
        if let Err(e) = swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic.clone(), data)
        {
            // This is expected when there are no subscribers
            tracing::trace!("gossipsub publish failed (likely no subscribers): {e}");
        }
    }
}
