//! Peer discovery and cluster membership tracking.
//!
//! Peers are discovered via:
//! - mDNS (same LAN)
//! - Relay circuit relay (remote, NAT traversal)
//!
//! Cluster membership is verified by checking the `cluster_psk` in the
//! Identify agent string after the Noise handshake.

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// How long a peer advertisement is considered fresh.
const STALE_TIMEOUT_SECS: u64 = 60;

/// Advertised backend info from a peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendAdvertisement {
    pub peer_id: String,
    pub backends: Vec<BackendInfo>,
    pub active_jobs: u32,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendInfo {
    pub name: String,
    pub base_url: String,
}

/// Tracks known cluster peers and their advertised backends.
#[derive(Debug, Default)]
pub struct PeerRegistry {
    peers: HashMap<PeerId, PeerEntry>,
}

#[derive(Debug)]
struct PeerEntry {
    advertisement: BackendAdvertisement,
    last_seen: Instant,
}

impl PeerRegistry {
    pub fn update(&mut self, peer_id: PeerId, ad: BackendAdvertisement) {
        self.peers.insert(
            peer_id,
            PeerEntry {
                advertisement: ad,
                last_seen: Instant::now(),
            },
        );
    }

    /// Remove stale entries.
    pub fn evict_stale(&mut self) {
        let cutoff = Instant::now() - std::time::Duration::from_secs(STALE_TIMEOUT_SECS);
        self.peers.retain(|_, e| e.last_seen > cutoff);
    }

    /// Find the peer with the fewest active jobs that has at least one backend.
    pub fn least_loaded_peer(&self) -> Option<(PeerId, &BackendAdvertisement)> {
        let cutoff = Instant::now() - std::time::Duration::from_secs(STALE_TIMEOUT_SECS);
        self.peers
            .iter()
            .filter(|(_, e)| e.last_seen > cutoff && !e.advertisement.backends.is_empty())
            .min_by_key(|(_, e)| e.advertisement.active_jobs)
            .map(|(id, e)| (*id, &e.advertisement))
    }

    /// Find the least-loaded peer, but only if its load is strictly less than `local_jobs`.
    pub fn least_loaded_peer_below(
        &self,
        local_jobs: u32,
    ) -> Option<(PeerId, &BackendAdvertisement)> {
        let cutoff = Instant::now() - std::time::Duration::from_secs(STALE_TIMEOUT_SECS);
        self.peers
            .iter()
            .filter(|(_, e)| {
                e.last_seen > cutoff
                    && !e.advertisement.backends.is_empty()
                    && e.advertisement.active_jobs < local_jobs
            })
            .min_by_key(|(_, e)| e.advertisement.active_jobs)
            .map(|(id, e)| (*id, &e.advertisement))
    }

    /// Iterate over all non-stale peers and their advertisements.
    pub fn fresh_peers(&self) -> impl Iterator<Item = (PeerId, &BackendAdvertisement)> {
        let cutoff = Instant::now() - std::time::Duration::from_secs(STALE_TIMEOUT_SECS);
        self.peers
            .iter()
            .filter(move |(_, e)| e.last_seen > cutoff)
            .map(|(id, e)| (*id, &e.advertisement))
    }

    pub fn peer_count(&self) -> usize {
        let cutoff = Instant::now() - std::time::Duration::from_secs(STALE_TIMEOUT_SECS);
        self.peers.values().filter(|e| e.last_seen > cutoff).count()
    }

    pub fn remove(&mut self, peer_id: &PeerId) {
        self.peers.remove(peer_id);
    }
}
