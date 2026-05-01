//! Memvault lifecycle handle.

use std::path::PathBuf;

use anyhow::Result;
use mac_mgmt_common::MemvaultConfig;
use tracing::info;

/// Handle to the running memvault subsystem.
pub struct MemvaultHandle {
    #[allow(dead_code)]
    store: memvault_store::MemvaultStore,
    #[allow(dead_code)]
    config: MemvaultConfig,
}

impl MemvaultHandle {
    /// Initialize the memvault subsystem.
    pub async fn init(config: &MemvaultConfig) -> Result<Self> {
        let data_dir = PathBuf::from(&config.data_dir);
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("blocks.redb");
        let store = memvault_store::MemvaultStore::open(&db_path)?;

        info!(data_dir = %config.data_dir, "memvault initialized");

        Ok(Self {
            store,
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
}
