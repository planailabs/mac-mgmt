//! Trait abstracting how the healer accesses an instance's file tunnels,
//! shell commands, and logs.
//!
//! * **Server mode** — [`RelayInstanceAccess`] routes through the relay proxy.
//! * **Daemon mode** — `LocalInstanceAccess` (in the daemon crate) calls
//!   file/shell tunnel registries directly.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;

/// Type-erased instance access shared across the healer subsystem.
pub type DynInstanceAccess = Arc<dyn InstanceAccess>;

// ── Shared result types ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ShellOutput {
    pub lines: Vec<ShellLine>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ShellLine {
    pub stream: String,
    pub data: String,
}

pub struct FileReadResult {
    pub content: Vec<u8>,
    pub mtime: Option<i64>,
}

// ── Trait ───────────────────────────────────────────────────────────────

#[async_trait]
pub trait InstanceAccess: Send + Sync + 'static {
    /// List files in a file tunnel directory.
    async fn file_list(&self, tunnel_name: &str, path: Option<&str>) -> Result<serde_json::Value>;

    /// Read a file from a file tunnel.
    async fn file_read(&self, tunnel_name: &str, path: &str) -> Result<FileReadResult>;

    /// Write a file to a file tunnel. `expected_mtime` enables optimistic concurrency.
    async fn file_write(
        &self,
        tunnel_name: &str,
        path: &str,
        content: &[u8],
        expected_mtime: Option<i64>,
    ) -> Result<serde_json::Value>;

    /// Execute a predefined shell command.
    async fn shell_exec(&self, command_name: &str, user_arg: Option<&str>) -> Result<ShellOutput>;

    /// Fetch recent log lines, optionally filtered by service.
    async fn log_fetch(
        &self,
        n: Option<usize>,
        service: Option<&str>,
        after: Option<usize>,
    ) -> Result<serde_json::Value>;

    /// Check if the instance is reachable.
    async fn is_online(&self) -> bool;

    /// Wait for the instance to become reachable (up to `max_secs`).
    async fn wait_until_online(&self, max_secs: u64) -> Result<()>;
}

// ── Cluster access (server-only, optional) ─────────────────────────────

/// Provides access to other instances in the same cluster.
/// Only available in server mode where the relay can reach peer nodes.
#[async_trait]
pub trait ClusterAccess: Send + Sync + 'static {
    /// Get an [`InstanceAccess`] handle for a peer instance.
    async fn instance_access(&self, instance_prefix: &str) -> Result<DynInstanceAccess>;
}

pub type DynClusterAccess = Arc<dyn ClusterAccess>;
