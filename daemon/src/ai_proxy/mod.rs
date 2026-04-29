pub mod auth;
pub mod backend;
pub mod server;
pub mod types;
pub mod usage;

use mac_mgmt_common::{AiProxyConfig, OllamaConfig, UnslothConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::RwLock;

/// SHA2-256 multihash code per the multiformats table.
const SHA2_256: u64 = 0x12;

/// A request to forward to a peer via libp2p.
#[cfg(feature = "relay")]
pub struct PeerProxyRequest {
    pub peer_id: libp2p::PeerId,
    pub body: serde_json::Value,
    pub response_tx: tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
}

/// Shared state for the AI proxy, held behind Arc for sharing with Rocket.
pub struct AiProxyState {
    pub keys: Arc<RwLock<Vec<KeyEntry>>>,
    pub backends: Arc<RwLock<BackendMap>>,
    pub usage_tracker: Arc<usage::UsageTracker>,
    pub client: reqwest::Client,
    /// Number of currently active inference requests (for load balancing).
    pub active_jobs: Arc<AtomicU32>,
    /// Peer registry for load-aware routing (set when p2p is active).
    #[cfg(feature = "relay")]
    pub p2p_peer_registry: Option<Arc<RwLock<crate::p2p::discovery::PeerRegistry>>>,
    /// Sender for AI proxy requests to peers via libp2p.
    #[cfg(feature = "relay")]
    pub p2p_request_tx: Option<tokio::sync::mpsc::Sender<PeerProxyRequest>>,
}

/// RAII guard that decrements the active job count on drop.
pub struct ActiveJobGuard {
    counter: Arc<AtomicU32>,
}

impl Drop for ActiveJobGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Pre-computed key entry for O(1) lookup by multihash.
pub struct KeyEntry {
    /// Hex-encoded multihash of the API key (from config).
    pub key_hash: String,
    pub name: String,
    pub token_budget: i64,
    pub budget_window: std::time::Duration,
    pub enabled: bool,
}

/// Known backend endpoints.
pub struct BackendMap {
    pub ollama: Option<BackendEndpoint>,
    pub unsloth: Option<BackendEndpoint>,
}

#[derive(Clone)]
pub struct BackendEndpoint {
    pub host: String,
    pub port: u16,
}

impl BackendEndpoint {
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

impl AiProxyState {
    pub fn new(
        config: &AiProxyConfig,
        ollama: &OllamaConfig,
        unsloth: &UnslothConfig,
        usage_tracker: Arc<usage::UsageTracker>,
    ) -> Self {
        let keys = build_key_entries(&config.keys);
        let backends = build_backend_map(ollama, unsloth);

        Self {
            keys: Arc::new(RwLock::new(keys)),
            backends: Arc::new(RwLock::new(backends)),
            usage_tracker,
            client: reqwest::Client::new(),
            active_jobs: Arc::new(AtomicU32::new(0)),
            #[cfg(feature = "relay")]
            p2p_peer_registry: None,
            #[cfg(feature = "relay")]
            p2p_request_tx: None,
        }
    }

    /// Increment the active job counter. Returns a guard that decrements on drop.
    pub fn track_job(&self) -> ActiveJobGuard {
        self.active_jobs.fetch_add(1, Ordering::Relaxed);
        ActiveJobGuard {
            counter: Arc::clone(&self.active_jobs),
        }
    }

    pub async fn reload_config(
        &self,
        config: &AiProxyConfig,
        ollama: &OllamaConfig,
        unsloth: &UnslothConfig,
    ) {
        *self.keys.write().await = build_key_entries(&config.keys);
        *self.backends.write().await = build_backend_map(ollama, unsloth);
    }
}

/// Compute the hex-encoded SHA2-256 multihash of a raw API key.
pub fn multihash_key(key: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(key.as_bytes());
    let mh = multihash::Multihash::<32>::wrap(SHA2_256, &digest)
        .expect("SHA2-256 digest fits in 32 bytes");
    hex::encode(mh.to_bytes())
}

fn build_key_entries(keys: &[mac_mgmt_common::AiProxyKeyConfig]) -> Vec<KeyEntry> {
    keys.iter()
        .map(|k| {
            let window = humantime::parse_duration(&k.budget_window)
                .unwrap_or(std::time::Duration::from_secs(86400));
            KeyEntry {
                key_hash: k.key_hash.clone(),
                name: k.name.clone(),
                token_budget: k.token_budget,
                budget_window: window,
                enabled: k.enabled,
            }
        })
        .collect()
}

fn build_backend_map(ollama: &OllamaConfig, unsloth: &UnslothConfig) -> BackendMap {
    BackendMap {
        ollama: if ollama.enabled {
            Some(BackendEndpoint {
                host: ollama.host.clone(),
                port: ollama.port,
            })
        } else {
            None
        },
        unsloth: if unsloth.enabled {
            Some(BackendEndpoint {
                host: unsloth.host.clone(),
                port: unsloth.port,
            })
        } else {
            None
        },
    }
}

/// Handle for the main daemon to interact with the proxy after spawning.
#[derive(Clone)]
pub struct AiProxyHandle {
    state: Arc<AiProxyState>,
}

impl AiProxyHandle {
    pub fn new(state: Arc<AiProxyState>) -> Self {
        Self { state }
    }

    pub async fn reload_config(
        &self,
        config: &AiProxyConfig,
        ollama: &OllamaConfig,
        unsloth: &UnslothConfig,
    ) {
        self.state.reload_config(config, ollama, unsloth).await;
    }
}
