//! Trait abstracting how the healer reads instance telemetry (probes,
//! samples, inventory, heartbeats).
//!
//! * **Server mode** — [`PgInstanceDataSource`] queries the database
//!   (heartbeats/assessments written by daemon heartbeat POSTs).
//! * **Daemon mode** — `LocalInstanceDataSource` reads directly from the
//!   in-memory [`Assessor`] snapshots.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

// Re-export the data transfer types so consumers don't need to reach into store.
pub use crate::store::{
    ClusterInstance, DaemonVersion, HeartbeatVersion, Inventory, ProbeHistoryEntry, ProbeStatus,
    ServiceState, SystemSample, VersionInfo,
};

/// Type-erased instance data source shared across the healer subsystem.
pub type DynInstanceData = Arc<dyn InstanceDataSource>;

#[async_trait]
pub trait InstanceDataSource: Send + Sync + 'static {
    /// Latest health probe results for all services on an instance.
    async fn get_probe_status(&self, instance_id: &str) -> Result<Option<ProbeStatus>>;

    /// Latest dynamic system sample (CPU, memory, disk, network, GPU).
    async fn get_system_sample(&self, instance_id: &str) -> Result<Option<SystemSample>>;

    /// Latest hardware/software inventory.
    async fn get_inventory(&self, instance_id: &str) -> Result<Option<Inventory>>;

    /// Recent probe results, optionally filtered by service.
    async fn get_probe_history(
        &self,
        instance_id: &str,
        service: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ProbeHistoryEntry>>;

    /// Full heartbeat data as JSON.
    async fn get_heartbeat_json(&self, instance_id: &str) -> Result<Option<serde_json::Value>>;

    /// Running daemon version + available versions.
    async fn get_version_info(&self, instance_id: &str) -> Result<VersionInfo>;

    /// Detailed per-service state from the latest heartbeat.
    async fn get_service_state(&self, instance_id: &str) -> Result<Option<ServiceState>>;

    /// List all instances in a cluster. Returns empty in daemon (local) mode.
    async fn get_cluster_instances(&self, cluster_id: Uuid) -> Result<Vec<ClusterInstance>>;

    /// Check if the instance has sent a heartbeat recently (within ~2 minutes).
    /// Daemon mode: always returns true (we are the instance).
    async fn has_recent_heartbeat(&self, instance_id: &str) -> Result<bool>;

    /// Get the relay proxy URL for an instance (from heartbeat data).
    /// Daemon mode: returns None (not applicable).
    async fn get_relay_proxy_url(&self, instance_id: &str) -> Result<Option<String>>;
}
