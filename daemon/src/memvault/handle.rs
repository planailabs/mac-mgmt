//! Memvault lifecycle handle.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use mac_mgmt_common::MemvaultConfig;
use tracing::info;

use super::BlockstoreBridge;

/// Handle to the running memvault subsystem.
pub struct MemvaultHandle {
    store: Arc<memvault_store::MemvaultStore>,
    #[allow(dead_code)]
    bridge: BlockstoreBridge,
    #[allow(dead_code)]
    config: MemvaultConfig,
}

impl MemvaultHandle {
    /// Initialize the memvault subsystem.
    pub async fn init(config: &MemvaultConfig) -> Result<Self> {
        let data_dir = PathBuf::from(&config.data_dir);
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("blocks.redb");
        let store = Arc::new(memvault_store::MemvaultStore::open(&db_path)?);
        let bridge = BlockstoreBridge::new(Arc::clone(&store));

        info!(data_dir = %config.data_dir, "memvault initialized");

        Ok(Self {
            store,
            bridge,
            config: config.clone(),
        })
    }

    /// Periodic tick — sync heads, check attestation expiry.
    pub async fn tick(&self) -> Result<()> {
        // Phase 1: basic housekeeping
        // - Announce own heads via gossipsub
        // - Check attestation expiry and request renewal
        Ok(())
    }

    /// Shutdown cleanly.
    pub async fn shutdown(self) -> Result<()> {
        info!("memvault shutting down");
        Ok(())
    }

    /// Get a reference to the underlying store.
    pub fn store(&self) -> &memvault_store::MemvaultStore {
        &self.store
    }

    /// Get a reference to the blockstore bridge.
    pub fn bridge(&self) -> &BlockstoreBridge {
        &self.bridge
    }
}
