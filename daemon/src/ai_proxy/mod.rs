pub mod auth;
pub mod backend;
pub mod server;
pub mod types;
pub mod usage;

use mac_mgmt_common::{AiProxyConfig, OllamaConfig, UnslothConfig};
use std::sync::Arc;
use tokio::sync::RwLock;

/// SHA2-256 multihash code per the multiformats table.
const SHA2_256: u64 = 0x12;

/// Shared state for the AI proxy, held behind Arc for sharing with Rocket.
pub struct AiProxyState {
    pub keys: Arc<RwLock<Vec<KeyEntry>>>,
    pub backends: Arc<RwLock<BackendMap>>,
    pub usage_tracker: Arc<usage::UsageTracker>,
    pub client: reqwest::Client,
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
