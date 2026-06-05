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

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, PeerId, Swarm, Transport, gossipsub, identify, mdns, request_response};
#[cfg(feature = "memvault")]
use libp2p::{kad, multiaddr::Protocol};
use tokio::sync::{RwLock, mpsc};

use behaviour::{ClusterBehaviour, ClusterBehaviourEvent};
use discovery::{BackendAdvertisement, PeerRegistry};
use protocols::ai_proxy;
use relay_state::{RelayAction, RelayEvent, RelayState};

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
    /// The relay's proxy_url was learned — daemon should send a heartbeat.
    RelayProxyUrlAcquired,
    /// An AI proxy request from a peer.
    AiProxyRequest {
        peer: PeerId,
        channel: request_response::ResponseChannel<ai_proxy::AiProxyResponse>,
        request: ai_proxy::AiProxyRequest,
    },
    /// A peer discovered or lost.
    PeerUpdate { peer: PeerId, connected: bool },
}

/// Configuration for the P2pManager.
pub struct P2pConfig {
    pub instance_id: String,
    pub cluster_psk: Option<Vec<u8>>,
    pub relay_multiaddr: Option<Multiaddr>,
    pub mdns_enabled: bool,
    pub p2p_port: u16,
    pub ai_proxy_distribution: bool,
    /// Server sync token for relay registration (relay validates to get cluster_id).
    pub server_token: Option<String>,
    /// Cluster ID (from server /api/self) for gossipsub topic subscription.
    pub cluster_id: Option<uuid::Uuid>,
    /// Handler state for processing incoming control requests.
    pub handler_state: Option<Arc<handler::HandlerState>>,
    /// External probe state: set to `true` when at least one listener is active.
    /// If `None`, the manager creates its own.
    pub swarm_listening: Option<Arc<AtomicBool>>,
    /// External probe state: set to `true` when the relay is registered.
    /// If `None`, the manager creates its own.
    pub relay_registered: Option<Arc<AtomicBool>>,
    /// memvault sync wiring. When `Some`, the swarm composes memvault's
    /// protocols, dials its bootstrap peers, and drives block sync over the
    /// shared cluster swarm. `None` leaves the (always-present) memvault
    /// sub-behaviour inert.
    #[cfg(feature = "memvault")]
    pub memvault: Option<MemvaultP2p>,
}

/// Everything the swarm needs to run memvault sync over the cluster swarm.
/// Assembled by the daemon's memvault subsystem (`crate::memvault`).
#[cfg(feature = "memvault")]
pub struct MemvaultP2p {
    /// The memvault blockstore (serve + ingest).
    pub store: Arc<memvault_store::MemvaultStore>,
    /// Sync parameters (cluster_id for head announcements).
    pub sync_config: memvault_swarm::SyncConfig,
    /// Join/attestation config assembled from the keystore.
    pub join_config: memvault_swarm::JoinConfig,
    /// libp2p multiaddrs to dial on startup so this node finds the cluster.
    pub bootstrap_peers: Vec<Multiaddr>,
    /// Locally-minted heads to announce, bridged off the LocalClient EventBus.
    pub head_rx: tokio::sync::mpsc::UnboundedReceiver<memvault_swarm::OutboundHead>,
}

/// [`memvault_swarm::MemvaultHost`] adapter over the daemon's cluster swarm.
/// Routes sends to the composed `memvault` sub-behaviour and shares the
/// daemon's gossipsub for head/admin announcements.
#[cfg(feature = "memvault")]
struct DaemonHost<'a>(&'a mut Swarm<ClusterBehaviour>);

#[cfg(feature = "memvault")]
impl memvault_swarm::MemvaultHost for DaemonHost<'_> {
    fn dial(&mut self, addr: Multiaddr) {
        if let Err(e) = self.0.dial(addr) {
            tracing::warn!(error = %e, "memvault dial failed");
        }
    }
    fn kad_add_address(&mut self, peer: &PeerId, addr: Multiaddr) {
        self.0.behaviour_mut().memvault.kad.add_address(peer, addr);
    }
    fn kad_set_server_mode(&mut self) {
        self.0
            .behaviour_mut()
            .memvault
            .kad
            .set_mode(Some(kad::Mode::Server));
    }
    fn kad_bootstrap(&mut self) {
        if let Err(e) = self.0.behaviour_mut().memvault.kad.bootstrap() {
            tracing::debug!(error = %e, "kademlia bootstrap skipped (no known peers)");
        }
    }
    fn send_block_request(&mut self, peer: &PeerId, req: memvault_net::BlockRequest) {
        self.0
            .behaviour_mut()
            .memvault
            .block_exchange
            .send_request(peer, req);
    }
    fn send_block_response(
        &mut self,
        channel: request_response::ResponseChannel<memvault_net::BlockResponse>,
        resp: memvault_net::BlockResponse,
    ) {
        let _ = self
            .0
            .behaviour_mut()
            .memvault
            .block_exchange
            .send_response(channel, resp);
    }
    fn send_join_request(&mut self, peer: &PeerId, req: memvault_net::JoinRequest) {
        let _ = self
            .0
            .behaviour_mut()
            .memvault
            .join
            .send_request(peer, req);
    }
    fn send_join_response(
        &mut self,
        channel: request_response::ResponseChannel<memvault_net::JoinResponse>,
        resp: memvault_net::JoinResponse,
    ) {
        let _ = self
            .0
            .behaviour_mut()
            .memvault
            .join
            .send_response(channel, resp);
    }
    fn gossip_publish(&mut self, topic: gossipsub::IdentTopic, data: Vec<u8>) {
        let _ = self.0.behaviour_mut().gossipsub.publish(topic, data);
    }
}

/// Route a memvault sub-behaviour event into the shared driver. Handles the
/// request_response Request/Response/failure arms; other events are ignored.
#[cfg(feature = "memvault")]
fn dispatch_memvault_event(
    driver: &mut memvault_swarm::MemvaultDriver,
    swarm: &mut Swarm<ClusterBehaviour>,
    ev: memvault_net::MemvaultBehaviourEvent,
) {
    use memvault_net::MemvaultBehaviourEvent as MvEv;
    use request_response::{Event as RrEvent, Message as RrMessage};
    let mut host = DaemonHost(swarm);
    match ev {
        MvEv::BlockExchange(RrEvent::Message {
            peer,
            message: RrMessage::Request { channel, request, .. },
            ..
        }) => driver.on_block_request(peer, channel, request, &mut host),
        MvEv::BlockExchange(RrEvent::Message {
            peer,
            message: RrMessage::Response { response, .. },
            ..
        }) => driver.on_block_response(peer, response, &mut host),
        MvEv::BlockExchange(RrEvent::OutboundFailure { peer, error, .. }) => {
            tracing::warn!(%peer, %error, "memvault block exchange outbound failure");
            driver.on_block_failure(peer);
        }
        MvEv::BlockExchange(RrEvent::InboundFailure { peer, error, .. }) => {
            tracing::warn!(%peer, %error, "memvault block exchange inbound failure");
            driver.on_block_failure(peer);
        }
        MvEv::Join(RrEvent::Message {
            peer,
            message: RrMessage::Request { channel, request, .. },
            ..
        }) => driver.on_join_request(peer, channel, request, &mut host),
        MvEv::Join(RrEvent::Message {
            peer,
            message: RrMessage::Response { response, .. },
            ..
        }) => driver.on_join_response(peer, response, &mut host),
        _ => {}
    }
}

/// Mirror shared swarm events (connection/identify/mdns/gossip) into the
/// memvault driver, in addition to the daemon's own handling of them.
#[cfg(feature = "memvault")]
fn mirror_event_to_memvault(
    driver: &mut memvault_swarm::MemvaultDriver,
    swarm: &mut Swarm<ClusterBehaviour>,
    event: &SwarmEvent<ClusterBehaviourEvent>,
) {
    let mut host = DaemonHost(swarm);
    match event {
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            driver.on_connection_established(*peer_id, &mut host);
        }
        SwarmEvent::ConnectionClosed { peer_id, .. } => {
            driver.on_connection_closed(*peer_id);
        }
        SwarmEvent::Behaviour(ClusterBehaviourEvent::Identify(identify::Event::Received {
            peer_id,
            info,
            ..
        })) => {
            driver.on_identify(*peer_id, &info.listen_addrs, &info.agent_version, &mut host);
        }
        SwarmEvent::Behaviour(ClusterBehaviourEvent::Mdns(mdns::Event::Discovered(peers))) => {
            driver.on_mdns_discovered(peers.iter().cloned(), &mut host);
        }
        SwarmEvent::Behaviour(ClusterBehaviourEvent::Gossipsub(gossipsub::Event::Message {
            propagation_source,
            message,
            ..
        })) => {
            let topic = message.topic.as_str();
            if memvault_net::gossip::is_heads_topic(topic)
                || memvault_net::gossip::is_admin_topic(topic)
            {
                driver.on_gossip(*propagation_source, message, &mut host);
            }
        }
        _ => {}
    }
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
    /// `true` once the swarm has at least one active listener.
    swarm_listening: Arc<AtomicBool>,
    /// `true` while the relay RPC connection is fully registered.
    relay_registered: Arc<AtomicBool>,
}

impl P2pManager {
    /// Create and start the P2p swarm.
    pub async fn new(host_key: &russh::keys::PrivateKey, config: P2pConfig) -> Result<Self> {
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
                let mut ws = libp2p::websocket::Config::new(dns_tcp);
                // In debug builds, skip TLS certificate verification for WSS
                // so the daemon can connect to relays with self-signed certs.
                #[cfg(debug_assertions)]
                {
                    let tls_config = build_insecure_ws_tls_config();
                    ws.set_tls_config(tls_config);
                }
                let ws = ws
                    .upgrade(libp2p::core::upgrade::Version::V1)
                    .authenticate(libp2p::noise::Config::new(key)?)
                    .multiplex(libp2p::yamux::Config::default())
                    .map(|(peer, muxer), _| {
                        (peer, libp2p::core::muxing::StreamMuxerBox::new(muxer))
                    });
                Ok(ws.boxed())
            })?
            .with_relay_client(libp2p::noise::Config::new, || {
                libp2p::yamux::Config::default()
            })?
            .with_behaviour(move |key, relay_client| {
                let identify_config =
                    identify::Config::new("/mac-mgmt/1.0.0".to_string(), key.public())
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
                    [(
                        ai_proxy::PROTOCOL_NAME,
                        request_response::ProtocolSupport::Full,
                    )],
                    request_response::Config::default()
                        .with_request_timeout(Duration::from_secs(600)),
                );

                Ok(ClusterBehaviour {
                    ping: libp2p::ping::Behaviour::new(
                        libp2p::ping::Config::new().with_interval(Duration::from_secs(15)),
                    ),
                    identify: identify::Behaviour::new(identify_config),
                    mdns: mdns_behaviour,
                    relay_client,
                    ai_proxy: ai_proxy_behaviour,
                    gossipsub: gossipsub_behaviour,
                    streams: libp2p_stream::Behaviour::new(),
                    // Composed memvault protocols. Inert (built but never driven)
                    // unless `config.memvault` is set and the driver is started.
                    #[cfg(feature = "memvault")]
                    memvault: memvault_net::MemvaultBehaviour::new(key.public().to_peer_id()),
                })
            })?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(3600)))
            .build();

        // Listen on QUIC and TCP (both IPv4 and IPv6).
        // Failures are non-fatal: the swarm can still dial the relay and
        // operate without local listeners (common behind NAT). An orphaned
        // socket from a container runtime or a previous crash can hold
        // the port; logging a warning lets the daemon continue rather than
        // losing all p2p connectivity.
        let quic_v4: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", config.p2p_port)
            .parse()
            .context("invalid QUIC listen address")?;
        if let Err(e) = swarm.listen_on(quic_v4) {
            tracing::warn!(
                "failed to listen on QUIC IPv4 (port {}): {e}",
                config.p2p_port
            );
        }
        let quic_v6: Multiaddr = format!("/ip6/::/udp/{}/quic-v1", config.p2p_port)
            .parse()
            .context("invalid QUIC IPv6 listen address")?;
        if let Err(e) = swarm.listen_on(quic_v6) {
            tracing::warn!(
                "failed to listen on QUIC IPv6 (port {}): {e}",
                config.p2p_port
            );
        }

        // TCP listeners on the same port — fallback for peers that cannot
        // reach us over QUIC/UDP (e.g. when dialing via WSS relay).
        let tcp_v4: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", config.p2p_port)
            .parse()
            .context("invalid TCP listen address")?;
        if let Err(e) = swarm.listen_on(tcp_v4) {
            tracing::warn!(
                "failed to listen on TCP IPv4 (port {}): {e}",
                config.p2p_port
            );
        }
        let tcp_v6: Multiaddr = format!("/ip6/::/tcp/{}", config.p2p_port)
            .parse()
            .context("invalid TCP listen address")?;
        if let Err(e) = swarm.listen_on(tcp_v6) {
            tracing::warn!(
                "failed to listen on TCP IPv6 (port {}): {e}",
                config.p2p_port
            );
        }

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
        let swarm_listening = config
            .swarm_listening
            .clone()
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
        let relay_registered = config
            .relay_registered
            .clone()
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

        // Extract stream control for opening RPC streams.
        let stream_control = swarm.behaviour().streams.new_control();

        // Shared relay PeerId — set when identified, cleared on disconnect.
        // The tunnel acceptor uses this to reject streams from non-relay peers.
        let relay_peer_id: Arc<RwLock<Option<PeerId>>> = Arc::new(RwLock::new(None));

        // Accept incoming tunnel data substreams from the relay.
        let tunnel_protocol = libp2p::StreamProtocol::new("/mac-mgmt/tunnel/1.0.0");
        let mut incoming_tunnels = stream_control.clone().accept(tunnel_protocol).unwrap();
        if let Some(hs) = &config.handler_state {
            let handler = Arc::clone(hs);
            let relay_pid = Arc::clone(&relay_peer_id);
            tokio::spawn(async move {
                while let Some((peer_id, stream)) = incoming_tunnels.next().await {
                    let allowed = *relay_pid.read().await;
                    if allowed != Some(peer_id) {
                        tracing::warn!(%peer_id, "rejected tunnel stream from non-relay peer");
                        continue;
                    }
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
        let relay_peer_id_clone = Arc::clone(&relay_peer_id);
        let swarm_listening_clone = Arc::clone(&swarm_listening);
        let relay_registered_clone = Arc::clone(&relay_registered);
        tokio::spawn(swarm_loop(
            swarm,
            cmd_rx,
            event_tx,
            peer_registry_clone,
            active_jobs_clone,
            relay_proxy_url_clone,
            relay_peer_id_clone,
            swarm_listening_clone,
            relay_registered_clone,
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
            swarm_listening,
            relay_registered,
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

    /// Shared flag: `true` once at least one swarm listener is active.
    pub fn swarm_listening(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.swarm_listening)
    }

    /// Shared flag: `true` while the relay RPC connection is registered.
    pub fn relay_registered(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.relay_registered)
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
    relay_peer_id: Arc<RwLock<Option<PeerId>>>,
    swarm_listening: Arc<AtomicBool>,
    relay_registered: Arc<AtomicBool>,
    stream_control: libp2p_stream::Control,
    mut config: P2pConfig,
) {
    let mut ad_interval = tokio::time::interval(Duration::from_secs(30));
    let mut evict_interval = tokio::time::interval(Duration::from_secs(15));
    let mut relay_tick = tokio::time::interval(Duration::from_secs(10));
    let mut authorized_cluster_peers: std::collections::HashSet<PeerId> =
        std::collections::HashSet::new();

    // Initialize relay state machine.
    let mut relay = RelayState::new(config.relay_multiaddr.clone());

    // memvault sync: compose the driver over the shared cluster swarm. Subscribe
    // gossipsub to memvault's heads/admin topics, dial bootstrap peers, and run
    // the same MemvaultDriver memctl uses.
    //
    // The timers + head channel are declared unconditionally so the `select!`
    // arms (tokio's `select!` does not accept `#[cfg]` on branches) compile in
    // both feature configurations; the handler bodies and the driver itself are
    // feature-gated, so when memvault is disabled the arms are inert no-ops.
    let mut mv_resync = tokio::time::interval(Duration::from_secs(5 * 60));
    let mut mv_join_retry = tokio::time::interval(Duration::from_secs(15));
    mv_resync.tick().await; // consume immediate ticks
    mv_join_retry.tick().await;

    // Optional periodic Kademlia bootstrap timer; armed from the driver's
    // configured interval in the memvault init below (None disables it).
    #[cfg_attr(not(feature = "memvault"), allow(unused_mut))]
    let mut kad_bootstrap_timer: Option<tokio::time::Interval> = None;

    #[cfg(feature = "memvault")]
    let (mut mv_driver, mut mv_head_rx) = match config.memvault.take() {
        Some(mv) => {
            // Cluster-scoped memvault topics: mesh only with same-cluster
            // peers (matches publish_head's heads_topic_for(cluster_id)).
            for topic in [
                memvault_net::gossip::heads_topic_for(&mv.sync_config.cluster_id),
                memvault_net::gossip::admin_topic_for(&mv.sync_config.cluster_id),
            ] {
                if let Err(e) = swarm.behaviour_mut().gossipsub.subscribe(&topic) {
                    tracing::warn!(error = %e, "failed to subscribe to memvault topic");
                }
            }
            for addr in &mv.bootstrap_peers {
                // Seed Kademlia directly when the multiaddr carries a /p2p/<peer>
                // component, so bootstrap() has routing entries even before
                // Identify completes (dial alone only adds to kad post-Identify).
                if let Some(peer) = addr.iter().find_map(|p| match p {
                    Protocol::P2p(id) => Some(id),
                    _ => None,
                }) {
                    swarm
                        .behaviour_mut()
                        .memvault
                        .kad
                        .add_address(&peer, addr.clone());
                    tracing::info!(%addr, %peer, "registered memvault bootstrap peer in kademlia");
                }
                match swarm.dial(addr.clone()) {
                    Ok(()) => tracing::info!(%addr, "dialing memvault bootstrap peer"),
                    Err(e) => tracing::warn!(%addr, error = %e, "memvault bootstrap dial failed"),
                }
            }

            let mut driver =
                memvault_swarm::MemvaultDriver::new(mv.store, mv.sync_config, mv.join_config);
            // on_start applies Kademlia server mode + the initial bootstrap
            // (both driven by SyncConfig); we just arm the periodic timer.
            driver.on_start(&mut DaemonHost(&mut swarm));
            kad_bootstrap_timer = match driver.kad_bootstrap_interval() {
                Some(d) => {
                    let mut t = tokio::time::interval(d);
                    t.tick().await; // consume immediate; on_start already bootstrapped
                    Some(t)
                }
                None => None,
            };

            tracing::info!("memvault sync active on cluster swarm");
            (Some(driver), Some(mv.head_rx))
        }
        None => (None, None),
    };
    // Inert placeholder so the head `select!` arm type-checks without the feature.
    #[cfg(not(feature = "memvault"))]
    let mut mv_head_rx: Option<tokio::sync::mpsc::UnboundedReceiver<()>> = None;

    // Subscribe to cluster gossipsub topic if cluster_id is known.
    // May also be updated from the relay registration response.
    let mut cluster_topic: Option<gossipsub::IdentTopic> = None;
    if let Some(cid) = &config.cluster_id {
        let topic = gossipsub::IdentTopic::new(format!("mac-mgmt/cluster/{cid}"));
        if let Err(e) = swarm.behaviour_mut().gossipsub.subscribe(&topic) {
            tracing::warn!("failed to subscribe to cluster topic: {e}");
        } else {
            tracing::info!(%cid, "subscribed to cluster gossipsub topic");
        }
        cluster_topic = Some(topic);
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
                // memvault sync: consume memvault-only sub-behaviour events
                // (block-exchange / join request-response) and mirror shared
                // events (connection / identify / mdns / gossip) into the driver.
                #[cfg(feature = "memvault")]
                let event = if let Some(driver) = mv_driver.as_mut() {
                    match event {
                        SwarmEvent::Behaviour(ClusterBehaviourEvent::Memvault(mv_ev)) => {
                            dispatch_memvault_event(driver, &mut swarm, mv_ev);
                            continue;
                        }
                        other => {
                            mirror_event_to_memvault(driver, &mut swarm, &other);
                            other
                        }
                    }
                } else {
                    event
                };

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
                        actions, &mut swarm, &relay_proxy_url, &relay_peer_id,
                        &mut authorized_cluster_peers, &event_tx, &relay_registered,
                    ).await;
                }

                // Handle non-relay swarm events.
                handle_general_event(
                    event, &event_tx, &peer_registry, &mut swarm,
                    &authorized_cluster_peers, &swarm_listening,
                ).await;
            }

            rpc_msg = rpc_recv => {
                match rpc_msg {
                    Ok(rpc::RpcMessage::Request { payload }) => {
                        // Relay-initiated requests now use tunnel substreams,
                        // not the RPC stream. Log unexpected requests.
                        let msg_type = payload["type"].as_str().unwrap_or("?");
                        tracing::debug!("unexpected RPC request from relay: {msg_type}");
                    }
                    Ok(rpc::RpcMessage::Response { .. }) => {
                        // Response to one of our requests — already dispatched by RpcStream.
                    }
                    Err(e) => {
                        tracing::warn!("RPC stream error: {e}, relay will reconnect");
                        // Actually disconnect the libp2p connection so the stale
                        // transport is cleaned up before we redial. Without this,
                        // the old connection stays alive and the new connection's
                        // noise handshake times out as a duplicate.
                        if let Some(peer_id) = relay.peer_id() {
                            let _ = swarm.disconnect_peer_id(peer_id);
                        }
                        let actions = relay.handle_event(RelayEvent::ConnectionClosed {
                            peer_id: relay.peer_id().unwrap_or(PeerId::random()),
                        });
                        execute_relay_actions(
                            actions, &mut swarm, &relay_proxy_url, &relay_peer_id,
                            &mut authorized_cluster_peers, &event_tx, &relay_registered,
                        ).await;
                    }
                }
            }

            Some(cmd) = cmd_rx.recv() => {
                if let Some(ref topic) = cluster_topic {
                    handle_command(cmd, &mut swarm, topic, &active_jobs).await;
                }
            }

            _ = ad_interval.tick() => {
                if let Some(ref topic) = cluster_topic {
                    publish_advertisement(&mut swarm, topic, &active_jobs).await;
                }
            }
            _ = evict_interval.tick() => {
                peer_registry.write().await.evict_stale();
            }

            // ── memvault: outbound head announcements from local writes ──
            head = async {
                match mv_head_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                #[cfg(feature = "memvault")]
                if let (Some(driver), Some(outbound)) = (mv_driver.as_mut(), head) {
                    driver.on_local_head(outbound, &mut DaemonHost(&mut swarm));
                }
                #[cfg(not(feature = "memvault"))]
                let _ = head;
            }
            // ── memvault: periodic RBSR resync ──
            _ = mv_resync.tick() => {
                #[cfg(feature = "memvault")]
                if let Some(driver) = mv_driver.as_mut() {
                    driver.tick_resync(&mut DaemonHost(&mut swarm));
                }
            }
            // ── memvault: /join/1.0 retry while a token is pending ──
            _ = mv_join_retry.tick() => {
                #[cfg(feature = "memvault")]
                if let Some(driver) = mv_driver.as_mut() {
                    driver.tick_join_retry(&mut DaemonHost(&mut swarm));
                }
            }
            // ── memvault: periodic Kademlia bootstrap (None disables) ──
            _ = async {
                match kad_bootstrap_timer.as_mut() {
                    Some(t) => { t.tick().await; }
                    None => std::future::pending::<()>().await,
                }
            } => {
                #[cfg(feature = "memvault")]
                if let Some(driver) = mv_driver.as_ref() {
                    driver.tick_kad_bootstrap(&mut DaemonHost(&mut swarm));
                }
            }
            _ = relay_tick.tick() => {
                // Drive relay state machine tick (reconnect, re-register).
                let actions = relay.handle_event(RelayEvent::Tick);
                execute_relay_actions(
                    actions, &mut swarm, &relay_proxy_url, &relay_peer_id,
                    &mut authorized_cluster_peers, &event_tx, &relay_registered,
                ).await;

                // If Identified but no RPC stream yet, try opening one.
                if matches!(relay, RelayState::Identified { .. }) {
                    if let Some(peer_id) = relay.peer_id() {
                        let mut ctrl = stream_control.clone();
                        match ctrl.open_stream(peer_id, rpc::RPC_PROTOCOL).await {
                            Ok(stream) => {
                                let actions = relay.handle_event(RelayEvent::RpcStreamOpened { stream });
                                execute_relay_actions(
                                    actions, &mut swarm, &relay_proxy_url, &relay_peer_id,
                                    &mut authorized_cluster_peers, &event_tx, &relay_registered,
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
                        let ssh_enabled = config.handler_state.as_ref()
                            .is_some_and(|hs| hs.ssh_allowed.load(Ordering::Relaxed));
                        let reg = serde_json::json!({
                            "type": "register",
                            "instance_id": config.instance_id,
                            "hostname": hostname::get().ok().map(|h| h.to_string_lossy().to_string()),
                            "token": config.server_token,
                            "ssh_enabled": ssh_enabled,
                        });
                        match rpc.call(reg).await {
                            Ok(resp) => {
                                if resp["type"].as_str() == Some("error") {
                                    tracing::warn!("relay rejected registration: {}", resp["error"]);
                                } else {
                                    // Subscribe to cluster gossipsub topic if we learned the cluster_id.
                                    if let Some(cid) = resp["cluster_id"].as_str() {
                                        let new_topic = gossipsub::IdentTopic::new(
                                            format!("mac-mgmt/cluster/{cid}"),
                                        );
                                        if cluster_topic.as_ref().map(|t| t.hash()) != Some(new_topic.hash()) {
                                            // Unsubscribe from old topic if different.
                                            if let Some(old) = cluster_topic.take() {
                                                let _ = swarm.behaviour_mut().gossipsub.unsubscribe(&old);
                                            }
                                            if let Err(e) = swarm.behaviour_mut().gossipsub.subscribe(&new_topic) {
                                                tracing::warn!("failed to subscribe to cluster topic: {e}");
                                            } else {
                                                tracing::info!(%cid, "subscribed to cluster gossipsub topic");
                                            }
                                            cluster_topic = Some(new_topic);
                                        }
                                    }
                                    // Parse relay's ephemeral SSH public key (if provided).
                                    if let Some(pubkey_str) = resp["relay_ssh_pubkey"].as_str() {
                                        if let Some(b64) = pubkey_str.split_whitespace().nth(1) {
                                            match russh::keys::parse_public_key_base64(b64) {
                                                Ok(key) => {
                                                    tracing::info!("received relay SSH public key");
                                                    if let Some(hs) = &config.handler_state {
                                                        *hs.relay_ssh_key.write().await = Some(key);
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::warn!("failed to parse relay SSH key: {e}");
                                                }
                                            }
                                        }
                                    }
                                    send_tunnel_advertisement_rpc(rpc, &config.handler_state).await;
                                    relay.mark_registered();
                                    relay_registered.store(true, Ordering::Relaxed);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("register RPC failed: {e}");
                            }
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
    relay_peer_id: &Arc<RwLock<Option<PeerId>>>,
    authorized_cluster_peers: &mut std::collections::HashSet<PeerId>,
    event_tx: &mpsc::Sender<P2pEvent>,
    relay_registered: &Arc<AtomicBool>,
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
                let acquired = url.is_some();
                *relay_proxy_url.write().await = url;
                if acquired {
                    let _ = event_tx.send(P2pEvent::RelayProxyUrlAcquired).await;
                }
            }
            RelayAction::AuthorizePeer(peer_id) => {
                *relay_peer_id.write().await = Some(peer_id);
                authorized_cluster_peers.insert(peer_id);
            }
            RelayAction::DeauthorizePeer(peer_id) => {
                *relay_peer_id.write().await = None;
                authorized_cluster_peers.remove(&peer_id);
                relay_registered.store(false, Ordering::Relaxed);
            }
            RelayAction::Log(level, msg) => match level {
                tracing::Level::INFO => tracing::info!("{msg}"),
                tracing::Level::WARN => tracing::warn!("{msg}"),
                tracing::Level::DEBUG => tracing::debug!("{msg}"),
                _ => tracing::trace!("{msg}"),
            },
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
    swarm_listening: &Arc<AtomicBool>,
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
                peer_registry.write().await.remove(&peer_id);
                let _ = event_tx
                    .send(P2pEvent::PeerUpdate {
                        peer: peer_id,
                        connected: false,
                    })
                    .await;
            }
        }
        // Control requests are now handled via the persistent RPC stream,
        // not via request_response. The control behaviour has been removed.
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
            if !authorized_cluster_peers.contains(&peer) {
                tracing::warn!(%peer, "rejecting AI proxy request from unauthorized peer");
                return;
            }
            let _ = event_tx
                .send(P2pEvent::AiProxyRequest {
                    peer,
                    channel,
                    request,
                })
                .await;
        }
        SwarmEvent::Behaviour(ClusterBehaviourEvent::Gossipsub(gossipsub::Event::Message {
            message,
            ..
        })) => {
            if let Ok(ad) = serde_json::from_slice::<BackendAdvertisement>(&message.data) {
                if let Ok(peer_id) = ad.peer_id.parse::<PeerId>() {
                    peer_registry.write().await.update(peer_id, ad);
                }
            }
        }
        SwarmEvent::NewListenAddr { address, .. } => {
            tracing::info!(%address, "listening on");
            swarm_listening.store(true, Ordering::Relaxed);
        }
        _ => {}
    }
}

async fn handle_command(
    cmd: P2pCommand,
    swarm: &mut Swarm<ClusterBehaviour>,
    cluster_topic: &gossipsub::IdentTopic,
    active_jobs: &Arc<AtomicU32>,
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

    let ssh_enabled = hs.ssh_allowed.load(Ordering::Relaxed);

    let req = serde_json::json!({
        "type": "tunnel_advertisement",
        "tunnels": tunnels,
        "file_tunnels": file_tunnels,
        "shell_tunnels": shell_tunnels,
        "ssh_enabled": ssh_enabled,
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
        "file_list" | "file_read" | "file_write" => match msg_type {
            "file_list" => handle_file_list_stream(handshake, &mut stream, handler_state).await,
            "file_read" => handle_file_read_stream(handshake, &mut stream, handler_state).await,
            "file_write" => handle_file_write_stream(handshake, &mut stream, handler_state).await,
            _ => unreachable!(),
        },
        #[cfg(feature = "services")]
        "shell" => {
            handle_streamed_shell(handshake, &mut stream, handler_state).await;
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
    use mac_mgmt_common::framing as stream_framing;

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
        use futures_util::{AsyncWriteExt as _, SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite;

        use mac_mgmt_common::framing::{TAG_BINARY, TAG_END as TAG_CLOSE, TAG_JSON as TAG_TEXT};

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

    // Check tunnel overrides before proxying.
    {
        let overrides = handler_state.tunnel_overrides.read().await;
        if let Some(ovrs) = overrides.get(tunnel_name) {
            for ovr in ovrs {
                if ovr.path.is_match(path) {
                    if let Some(resp) = (ovr.override_fn)(path, &headers) {
                        let location = resp
                            .headers
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case("location"))
                            .map(|(_, v)| format!(" -> {v}"))
                            .unwrap_or_default();
                        tracing::debug!(
                            "tunnel override fired: tunnel={tunnel_name} path={path} status={}{location}",
                            resp.status
                        );
                        let resp_headers: Vec<[&str; 2]> = resp
                            .headers
                            .iter()
                            .map(|(k, v)| [k.as_str(), v.as_str()])
                            .collect();
                        let header = serde_json::json!({
                            "status": resp.status,
                            "headers": resp_headers,
                        });
                        let _ = stream_framing::write_json(stream, &header).await;
                        if !resp.body.is_empty() {
                            let _ = stream_framing::write_binary(stream, &resp.body).await;
                        }
                        let _ = stream_framing::write_end(stream).await;
                        return;
                    }
                    tracing::trace!(
                        "tunnel override matched path but passed through: tunnel={tunnel_name} path={path}"
                    );
                }
            }
        }
    }

    let req = proxy_helpers::build_proxy_request(
        &handler_state.client,
        &target,
        method,
        path,
        handler_state.fake_origin_local,
    );
    let req =
        proxy_helpers::apply_headers_vec(req, &headers, handler_state.fake_origin_local, &target);
    let req = proxy_helpers::apply_body_b64(req, body_b64);

    match req
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await
    {
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
    use mac_mgmt_common::framing as stream_framing;

    let path = handshake["path"].as_str().unwrap_or("/metrics");
    let url = format!("http://[::1]:{}{path}", handler_state.metrics_port);

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

/// Handle a file list request over a tunnel substream.
#[cfg(feature = "services")]
async fn handle_file_list_stream(
    handshake: serde_json::Value,
    stream: &mut libp2p::Stream,
    handler_state: &Arc<handler::HandlerState>,
) {
    use mac_mgmt_common::framing as stream_framing;

    let tunnel_name = handshake["tunnel_name"].as_str().unwrap_or("");
    let path = handshake["path"].as_str();

    let registry = handler_state.file_tunnel_registry.read().await;
    let Some(tunnel) = registry.get(tunnel_name) else {
        let resp = serde_json::json!({ "status": 404, "error": "unknown file tunnel" });
        let _ = stream_framing::write_json(stream, &resp).await;
        let _ = stream_framing::write_end(stream).await;
        return;
    };

    let (status, data) = crate::file_tunnels::handle_list(tunnel, path);
    let resp = serde_json::json!({ "status": status, "body": data });
    let _ = stream_framing::write_json(stream, &resp).await;
    let _ = stream_framing::write_end(stream).await;
}

/// Handle a file read request over a tunnel substream.
#[cfg(feature = "services")]
async fn handle_file_read_stream(
    handshake: serde_json::Value,
    stream: &mut libp2p::Stream,
    handler_state: &Arc<handler::HandlerState>,
) {
    use mac_mgmt_common::framing as stream_framing;

    let tunnel_name = handshake["tunnel_name"].as_str().unwrap_or("");
    let path = handshake["path"].as_str();

    let registry = handler_state.file_tunnel_registry.read().await;
    let Some(tunnel) = registry.get(tunnel_name) else {
        let resp = serde_json::json!({ "status": 404, "error": "unknown file tunnel" });
        let _ = stream_framing::write_json(stream, &resp).await;
        let _ = stream_framing::write_end(stream).await;
        return;
    };

    match crate::file_tunnels::read_file(tunnel, path) {
        Ok((content, mtime)) => {
            let header = serde_json::json!({
                "status": 200,
                "size": content.len(),
                "mtime": mtime.unwrap_or(0),
            });
            if stream_framing::write_json(stream, &header).await.is_err() {
                return;
            }
            // Stream content in chunks.
            for chunk in content.chunks(1024 * 1024) {
                if stream_framing::write_binary(stream, chunk).await.is_err() {
                    return;
                }
            }
            let _ = stream_framing::write_end(stream).await;
        }
        Err((status, error)) => {
            let resp = serde_json::json!({ "status": status, "error": error });
            let _ = stream_framing::write_json(stream, &resp).await;
            let _ = stream_framing::write_end(stream).await;
        }
    }
}

/// Handle a file write request over a tunnel substream.
#[cfg(feature = "services")]
async fn handle_file_write_stream(
    handshake: serde_json::Value,
    stream: &mut libp2p::Stream,
    handler_state: &Arc<handler::HandlerState>,
) {
    use mac_mgmt_common::framing as stream_framing;

    let tunnel_name = handshake["tunnel_name"].as_str().unwrap_or("");
    let path = handshake["path"].as_str();
    let expected_mtime = handshake["expected_mtime"].as_i64();

    // Decode base64 data from handshake.
    let content = handshake["data"]
        .as_str()
        .and_then(|d| {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.decode(d).ok()
        })
        .unwrap_or_default();

    let registry = handler_state.file_tunnel_registry.read().await;
    let Some(tunnel) = registry.get(tunnel_name) else {
        let resp = serde_json::json!({ "status": 404, "error": "unknown file tunnel" });
        let _ = stream_framing::write_json(stream, &resp).await;
        let _ = stream_framing::write_end(stream).await;
        return;
    };

    match crate::file_tunnels::write_file(tunnel, path, &content, expected_mtime) {
        Ok(mtime) => {
            let resp = serde_json::json!({ "status": 200, "body": { "mtime": mtime } });
            let _ = stream_framing::write_json(stream, &resp).await;
            let _ = stream_framing::write_end(stream).await;
        }
        Err((status, error)) => {
            let resp = serde_json::json!({ "status": status, "body": { "error": error } });
            let _ = stream_framing::write_json(stream, &resp).await;
            let _ = stream_framing::write_end(stream).await;
        }
    }
}

#[cfg(not(feature = "services"))]
async fn handle_file_list_stream(
    _: serde_json::Value,
    stream: &mut libp2p::Stream,
    _: &Arc<handler::HandlerState>,
) {
    use mac_mgmt_common::framing as stream_framing;
    let _ = stream_framing::write_json(
        stream,
        &serde_json::json!({ "status": 501, "error": "services not enabled" }),
    )
    .await;
    let _ = stream_framing::write_end(stream).await;
}
#[cfg(not(feature = "services"))]
async fn handle_file_read_stream(
    _: serde_json::Value,
    stream: &mut libp2p::Stream,
    _: &Arc<handler::HandlerState>,
) {
    use mac_mgmt_common::framing as stream_framing;
    let _ = stream_framing::write_json(
        stream,
        &serde_json::json!({ "status": 501, "error": "services not enabled" }),
    )
    .await;
    let _ = stream_framing::write_end(stream).await;
}
#[cfg(not(feature = "services"))]
async fn handle_file_write_stream(
    _: serde_json::Value,
    stream: &mut libp2p::Stream,
    _: &Arc<handler::HandlerState>,
) {
    use mac_mgmt_common::framing as stream_framing;
    let _ = stream_framing::write_json(
        stream,
        &serde_json::json!({ "status": 501, "error": "services not enabled" }),
    )
    .await;
    let _ = stream_framing::write_end(stream).await;
}

/// Handle a shell command execution over a tunnel substream.
/// Uses stream_framing to send output lines and exit code.
#[cfg(feature = "services")]
async fn handle_streamed_shell(
    handshake: serde_json::Value,
    stream: &mut libp2p::Stream,
    handler_state: &Arc<handler::HandlerState>,
) {
    use mac_mgmt_common::framing as stream_framing;

    let command_name = handshake["command_name"].as_str().unwrap_or("");
    let user_arg = handshake["user_arg"].as_str();

    // Read everything from registry, then drop it before async work.
    let (virtual_handler, exec_timeout, cmd_result) = {
        let registry = handler_state.shell_tunnel_registry.read().await;
        let Some(tunnel) = registry.get(command_name) else {
            let err = serde_json::json!({ "exit_code": -1, "error": format!("unknown command: {command_name}") });
            let _ = stream_framing::write_json(stream, &err).await;
            let _ = stream_framing::write_end(stream).await;
            return;
        };

        let virtual_handler = registry.get_virtual(command_name).cloned();

        if let Err(e) = crate::shell_tunnels::validate_args(tunnel, user_arg) {
            let err = serde_json::json!({ "exit_code": -1, "error": e });
            let _ = stream_framing::write_json(stream, &err).await;
            let _ = stream_framing::write_end(stream).await;
            return;
        }

        let timeout = tunnel
            .def
            .timeout_secs
            .unwrap_or(crate::shell_tunnels::DEFAULT_EXEC_SECS);
        let cmd = crate::shell_tunnels::build_command(tunnel, user_arg);
        (virtual_handler, timeout, cmd)
    };

    if let Some(handler) = &virtual_handler {
        let output = handler(user_arg);
        for (strm, data) in &output.lines {
            let msg = serde_json::json!({ "stream": strm, "data": data });
            if stream_framing::write_json(stream, &msg).await.is_err() {
                return;
            }
        }
        let msg = serde_json::json!({ "exit_code": output.exit_code });
        let _ = stream_framing::write_json(stream, &msg).await;
        let _ = stream_framing::write_end(stream).await;
        return;
    }

    let mut cmd = cmd_result;

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let err = serde_json::json!({ "exit_code": -1, "error": format!("spawn failed: {e}") });
            let _ = stream_framing::write_json(stream, &err).await;
            let _ = stream_framing::write_end(stream).await;
            return;
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let mut stdout_reader = tokio::io::BufReader::new(stdout).lines();
    let mut stderr_reader = tokio::io::BufReader::new(stderr).lines();

    use tokio::io::AsyncBufReadExt;
    let exec_timeout = exec_timeout;
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(exec_timeout));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line {
                    Ok(Some(data)) => {
                        let msg = serde_json::json!({ "stream": "stdout", "data": data });
                        if stream_framing::write_json(stream, &msg).await.is_err() {
                            let _ = child.kill().await;
                            return;
                        }
                    }
                    Ok(None) => {
                        while let Ok(Some(data)) = stderr_reader.next_line().await {
                            let msg = serde_json::json!({ "stream": "stderr", "data": data });
                            if stream_framing::write_json(stream, &msg).await.is_err() {
                                let _ = child.kill().await;
                                return;
                            }
                        }
                        break;
                    }
                    Err(_) => break,
                }
            }
            line = stderr_reader.next_line() => {
                match line {
                    Ok(Some(data)) => {
                        let msg = serde_json::json!({ "stream": "stderr", "data": data });
                        if stream_framing::write_json(stream, &msg).await.is_err() {
                            let _ = child.kill().await;
                            return;
                        }
                    }
                    Ok(None) => {
                        while let Ok(Some(data)) = stdout_reader.next_line().await {
                            let msg = serde_json::json!({ "stream": "stdout", "data": data });
                            if stream_framing::write_json(stream, &msg).await.is_err() {
                                let _ = child.kill().await;
                                return;
                            }
                        }
                        break;
                    }
                    Err(_) => break,
                }
            }
            _ = &mut timeout => {
                let _ = child.kill().await;
                let msg = serde_json::json!({ "exit_code": -1, "error": format!("timeout ({exec_timeout}s)") });
                let _ = stream_framing::write_json(stream, &msg).await;
                let _ = stream_framing::write_end(stream).await;
                return;
            }
        }
    }

    let exit_code = match child.wait().await {
        Ok(status) => status.code().unwrap_or(-1),
        Err(_) => -1,
    };

    let msg = serde_json::json!({ "exit_code": exit_code });
    let _ = stream_framing::write_json(stream, &msg).await;
    let _ = stream_framing::write_end(stream).await;
}

#[cfg(not(feature = "services"))]
async fn handle_streamed_shell(
    _handshake: serde_json::Value,
    stream: &mut libp2p::Stream,
    _handler_state: &Arc<handler::HandlerState>,
) {
    use mac_mgmt_common::framing as stream_framing;
    let err = serde_json::json!({ "exit_code": -1, "error": "services feature not enabled" });
    let _ = stream_framing::write_json(stream, &err).await;
    let _ = stream_framing::write_end(stream).await;
}

/// Handle an SSH session over a tunnel substream.
/// The substream acts as the transport for the russh SSH server.
async fn handle_ssh_session(stream: libp2p::Stream, handler_state: &Arc<handler::HandlerState>) {
    use tokio_util::compat::FuturesAsyncReadCompatExt;

    // Load authorized SSH keys from all sources.
    let authorized_keys = {
        if !handler_state.ssh_allowed.load(Ordering::Relaxed) {
            tracing::warn!("SSH session rejected: SSH access disabled");
            return;
        }
        // 1. File-based authorized_keys
        let mut keys = crate::remote_ssh::ssh_server::load_authorized_keys();
        // 2. Server-synced SSH keys (cluster_ssh_keys table)
        keys.extend(handler_state.server_ssh_keys.read().await.iter().cloned());
        // 3. Relay's ephemeral SSH key
        if let Some(relay_key) = handler_state.relay_ssh_key.read().await.as_ref() {
            keys.push(relay_key.clone());
        }
        keys
    };

    let config = std::sync::Arc::new(russh::server::Config {
        keys: vec![crate::host_keys::load_or_generate().expect("failed to load host key for SSH")],
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

/// Build a `libp2p_websocket::tls::Config` that skips certificate verification.
/// Debug builds only — allows connecting to relays with self-signed certs.
///
/// SAFETY: `libp2p_websocket::tls::Config` has `pub(crate)` fields so we can't
/// construct it directly. We use transmute since the struct layout is:
///   { client: futures_rustls::TlsConnector, server: Option<futures_rustls::TlsAcceptor> }
#[cfg(debug_assertions)]
fn build_insecure_ws_tls_config() -> libp2p::websocket::tls::Config {
    use std::sync::Arc;

    #[derive(Debug)]
    struct AcceptAnyCert;

    impl rustls::client::danger::ServerCertVerifier for AcceptAnyCert {
        fn verify_server_cert(
            &self,
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &[rustls::pki_types::CertificateDer<'_>],
            _: &rustls::pki_types::ServerName<'_>,
            _: &[u8],
            _: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            _: &[u8],
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self,
            _: &[u8],
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    let provider = rustls::crypto::ring::default_provider();
    let client_config = rustls::ClientConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_no_client_auth();

    let connector = futures_rustls::TlsConnector::from(Arc::new(client_config));

    // Transmute a struct with identical layout to libp2p_websocket::tls::Config.
    // The struct has two fields: client (TlsConnector), server (Option<TlsAcceptor>).
    #[repr(C)]
    struct TlsConfigRepr {
        client: futures_rustls::TlsConnector,
        server: Option<futures_rustls::TlsAcceptor>,
    }

    let repr = TlsConfigRepr {
        client: connector,
        server: None,
    };

    tracing::warn!("using insecure TLS config for WSS (debug build)");
    // SAFETY: TlsConfigRepr has the same fields and types as
    // libp2p_websocket::tls::Config. Both are non-repr(C) Rust structs
    // with the same field types in the same order.
    unsafe { std::mem::transmute(repr) }
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

fn cluster_id_cache_path() -> std::path::PathBuf {
    crate::config::config_dir().join("cluster-id")
}

/// Load cached cluster_id from disk.
pub fn load_cached_cluster_id() -> Option<uuid::Uuid> {
    let contents = std::fs::read_to_string(cluster_id_cache_path()).ok()?;
    contents.trim().parse().ok()
}

fn cache_cluster_id(id: &uuid::Uuid) {
    if let Err(e) = std::fs::write(cluster_id_cache_path(), id.to_string()) {
        tracing::warn!("failed to cache cluster_id: {e}");
    }
}

/// Fetch the daemon's cluster_id from the server's /api/self endpoint.
/// Caches the result; falls back to cache if the server is unreachable.
pub async fn fetch_cluster_id(server_url: &str, token: &str) -> Option<uuid::Uuid> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{server_url}/api/self"))
        .bearer_auth(token)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok();
    if let Some(resp) = resp {
        if resp.status().is_success() {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if let Some(cid) = body["cluster_id"].as_str().and_then(|s| s.parse().ok()) {
                    cache_cluster_id(&cid);
                    return Some(cid);
                }
            }
        }
    }
    // Server unreachable or returned error — try cache.
    let cached = load_cached_cluster_id();
    if cached.is_some() {
        tracing::info!("using cached cluster_id (server unreachable)");
    }
    cached
}

/// Verify the PSK auth token in a peer's agent version string.
///
/// Note: this scheme broadcasts a deterministic SHA-256(psk || peer_id)
/// digest in libp2p's Identify agent_version, which is observable to anyone
/// on the same network. A high-entropy cluster_psk is therefore required —
/// a passphrase-strength PSK can be brute-forced offline. A future protocol
/// revision should bind the PSK to the Noise handshake (e.g. via prologue
/// or post-handshake MAC over the session key) so possession proofs aren't
/// reusable. The comparison below uses constant-time equality so the
/// verifier itself doesn't add a second timing-side-channel on top.
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
    constant_time_eq(auth_token.as_bytes(), expected.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
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
