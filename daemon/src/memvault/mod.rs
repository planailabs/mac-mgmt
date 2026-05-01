//! Memvault integration bridge.
//!
//! This module connects the memvault p2p memory store to the daemon's
//! event loop and libp2p swarm.

mod handle;

pub use handle::MemvaultHandle;
