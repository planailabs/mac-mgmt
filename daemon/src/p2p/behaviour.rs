//! Composite `NetworkBehaviour` for the daemon's libp2p swarm.

use libp2p::swarm::NetworkBehaviour;
use libp2p::{gossipsub, identify, mdns, relay, request_response};

use super::protocols::{ai_proxy, control};

/// Combined behaviour for the cluster p2p network.
#[derive(NetworkBehaviour)]
pub struct ClusterBehaviour {
    /// Peer identification — exchanges PeerId, listen addresses, agent version.
    pub identify: identify::Behaviour,
    /// mDNS local peer discovery for same-LAN peers.
    pub mdns: mdns::tokio::Behaviour,
    /// Circuit relay client for NAT traversal through the relay node.
    pub relay_client: relay::client::Behaviour,
    /// Control channel: session requests, metrics, proxy, file, shell.
    pub control: request_response::Behaviour<control::ControlCodec>,
    /// AI proxy request distribution across cluster peers.
    pub ai_proxy: request_response::Behaviour<ai_proxy::AiProxyCodec>,
    /// Pub/sub for tunnel advertisements and backend load announcements.
    pub gossipsub: gossipsub::Behaviour,
}
