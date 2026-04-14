//! Generic process supervisor with an RPC interface.
//!
//! The daemon tells this supervisor which processes to run via `Register`
//! requests; the supervisor spawns them, restarts them on crash, and streams
//! logs and crash notifications back over the same Unix socket.
//!
//! Kept deliberately small so the long-running supervisor rarely needs to be
//! re-executed when the daemon upgrades.

pub mod client;
pub mod protocol;
pub mod server;

pub use client::Client;
pub use protocol::{Notification, Request, Response, ServiceStatus, SpawnSpec};

use std::path::PathBuf;

/// Default socket path the daemon and supervisor agree on.
pub fn default_socket_path() -> PathBuf {
    let base = dirs_home()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/mac-mgmt");
    base.join("services.sock")
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
