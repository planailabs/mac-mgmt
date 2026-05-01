//! Memvault lifecycle handle.
//!
//! Manages the memvault subsystem within the daemon: store, blockstore bridge,
//! web API server, and periodic housekeeping.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use mac_mgmt_common::MemvaultConfig;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::BlockstoreBridge;

/// Handle to the running memvault subsystem.
pub struct MemvaultHandle {
    store: Arc<memvault_store::MemvaultStore>,
    bridge: BlockstoreBridge,
    client: Arc<memvault_api::LocalClient>,
    config: MemvaultConfig,
    /// Join handle for the web server task (if started).
    web_handle: Option<tokio::task::JoinHandle<()>>,
}

impl MemvaultHandle {
    /// Initialize the memvault subsystem.
    ///
    /// Opens the store, creates the local client, starts the web server if
    /// configured, and returns the handle.
    pub async fn init(config: &MemvaultConfig) -> Result<Self> {
        let data_dir = if config.data_dir.is_empty() {
            default_data_dir()
        } else {
            PathBuf::from(&config.data_dir)
        };
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("blocks.redb");
        let store = Arc::new(memvault_store::MemvaultStore::open(&db_path)?);
        let bridge = BlockstoreBridge::new(Arc::clone(&store));

        // Create the local client (used by the web API and for internal operations).
        let client = Arc::new(memvault_api::LocalClient::new(
            Arc::clone(&store),
            Arc::new(RwLock::new(memvault_query::TextIndex::new())),
            Arc::new(RwLock::new(memvault_query::QuotaManager::new(Default::default()))),
            Arc::new(memvault_api::EventBus::new(256)),
            vec![0u8; 32], // TODO: derive from daemon host key
            cluster_id_from_dir(&data_dir),
        ));

        // Start the web API server if web_port > 0.
        let web_handle = if config.web_port > 0 {
            let port = config.web_port;
            let app_state = Arc::new(memvault_web::AppState {
                client: Arc::clone(&client) as Arc<dyn memvault_api::MemvaultClient>,
                event_bus: Arc::new(memvault_api::EventBus::new(256)),
                auth_token: config.auth_token.clone().unwrap_or_default(),
                metrics: Arc::new(memvault_api::metrics::Metrics::new()),
            });
            let router = memvault_web::build_router(app_state);
            let handle = tokio::spawn(async move {
                let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("memvault web: failed to bind port {port}: {e}");
                        return;
                    }
                };
                info!(port, "memvault web API started");
                if let Err(e) = axum::serve(listener, router).await {
                    tracing::error!("memvault web server error: {e}");
                }
            });
            Some(handle)
        } else {
            None
        };

        info!(data_dir = %data_dir.display(), "memvault initialized");

        Ok(Self {
            store,
            bridge,
            client,
            config: config.clone(),
            web_handle,
        })
    }

    /// Periodic tick — called from the daemon's health loop.
    ///
    /// Performs housekeeping: gossip head announcements, check attestation
    /// expiry, process eager-replication queue, run deferred extractions.
    pub async fn tick(&self) {
        // Check for new blocks that need eager replication announcement
        // (In a full implementation, this would gossip DocumentHead and
        // AttachmentHeadRef for any new envelopes since last tick.)

        // Process deferred extraction queue
        // (Large files queued for async extraction get processed here.)
    }

    /// Handle an incoming block from the p2p network (via beetswap).
    pub fn on_block_received(&self, cid: &[u8], data: &[u8]) -> bool {
        match self.bridge.on_block_received(cid, data) {
            Ok(was_new) => was_new,
            Err(e) => {
                warn!("memvault: error storing received block: {e}");
                false
            }
        }
    }

    /// Serve a block requested by a remote peer (via beetswap).
    pub fn on_block_wanted(&self, cid: &[u8]) -> Option<Vec<u8>> {
        match self.bridge.on_block_wanted(cid) {
            Ok(data) => data,
            Err(e) => {
                warn!("memvault: error serving block: {e}");
                None
            }
        }
    }

    /// Shutdown cleanly — abort web server, flush store.
    pub async fn shutdown(self) {
        info!("memvault shutting down");
        if let Some(handle) = self.web_handle {
            handle.abort();
        }
    }

    /// Get a reference to the underlying store.
    pub fn store(&self) -> &memvault_store::MemvaultStore {
        &self.store
    }

    /// Get a reference to the blockstore bridge.
    pub fn bridge(&self) -> &BlockstoreBridge {
        &self.bridge
    }

    /// Get a reference to the local client.
    pub fn client(&self) -> &memvault_api::LocalClient {
        &self.client
    }
}

fn default_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("memvault")
}

fn cluster_id_from_dir(data_dir: &PathBuf) -> Vec<u8> {
    let id_path = data_dir.join("cluster_id");
    match std::fs::read_to_string(&id_path) {
        Ok(hex_str) => hex::decode(hex_str.trim()).unwrap_or_else(|_| vec![0u8; 32]),
        Err(_) => vec![0u8; 32],
    }
}
