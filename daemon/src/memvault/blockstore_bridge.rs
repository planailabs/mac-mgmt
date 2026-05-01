//! Bridge between beetswap (libp2p block exchange) and the memvault store.
//!
//! When the swarm receives bitswap WANT messages, this bridge serves blocks
//! from the local MemvaultStore. When blocks arrive via bitswap, they are
//! written to the store.

use memvault_store::MemvaultStore;
use std::sync::Arc;

/// Bridge between the p2p block exchange and local storage.
pub struct BlockstoreBridge {
    store: Arc<MemvaultStore>,
}

impl BlockstoreBridge {
    pub fn new(store: Arc<MemvaultStore>) -> Self {
        Self { store }
    }

    /// Handle an incoming block (received via bitswap from a peer).
    /// Stores it locally if not already present.
    pub fn on_block_received(&self, cid: &[u8], data: &[u8]) -> Result<bool, anyhow::Error> {
        if self.store.has_block(cid)? {
            return Ok(false); // already have it
        }
        self.store.put_block(cid, data)?;
        Ok(true)
    }

    /// Handle a WANT request (peer wants a block we might have).
    /// Returns the block data if we have it.
    pub fn on_block_wanted(&self, cid: &[u8]) -> Result<Option<Vec<u8>>, anyhow::Error> {
        Ok(self.store.get_block(cid)?)
    }

    /// Check if we have a block locally.
    pub fn has_block(&self, cid: &[u8]) -> Result<bool, anyhow::Error> {
        Ok(self.store.has_block(cid)?)
    }
}
