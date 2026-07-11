//! Generic process supervisor with an RPC interface.
//!
//! The daemon tells this supervisor which processes to run via `Register`
//! requests; the supervisor spawns them, restarts them on crash, and streams
//! logs and crash notifications back over the same Unix socket.
//!
//! Kept deliberately small so the long-running supervisor rarely needs to be
//! re-executed when the daemon upgrades.

pub mod client;
mod procutil;
pub mod protocol;
pub mod server;
mod transport;

pub use client::Client;
pub use protocol::{Notification, Request, Response, ServiceStatus, SpawnSpec};

use std::path::PathBuf;

/// Default socket path the daemon and supervisor agree on.
///
/// `MAC_MGMT_RUNTIME_DIR` relocates it off the config/home dir: on plan-ai-usb
/// `HOME` is the FAT32 stick, where binding a unix socket fails with EPERM, so
/// the launcher pins this to a host-local runtime dir (the same one the agent's
/// `config::runtime_dir()` resolves). On Windows the path is only mapped to a
/// named-pipe name (see `transport`), so the relocation is inert there.
pub fn default_socket_path() -> PathBuf {
    if let Some(d) = std::env::var_os("MAC_MGMT_RUNTIME_DIR") {
        return PathBuf::from(d).join("services.sock");
    }
    let base = dirs_home()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/mac-mgmt");
    base.join("services.sock")
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
