//! Memvault integration bridge.
//!
//! This module connects the memvault p2p memory store to the daemon's
//! event loop and libp2p swarm.

mod blockstore_bridge;
mod handle;

pub use blockstore_bridge::BlockstoreBridge;
pub use handle::MemvaultHandle;
