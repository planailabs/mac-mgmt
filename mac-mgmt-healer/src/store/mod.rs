//! Abstract store trait for the healer subsystem.
//!
//! The healer crate operates against this trait instead of a concrete database.
//! Enable `feature = "postgres"` to get a [`PgHealerStore`] implementation backed
//! by `sqlx::PgPool`.

#[cfg(feature = "postgres")]
pub mod pg;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::session::models::{HealerMessage, HealerSession, SessionState, StaffPing};

/// Type-erased store shared across the healer subsystem.
pub type DynStore = Arc<dyn HealerStore>;

// ── Trait ──────────────────────────────────────────────────────────────

#[async_trait]
pub trait HealerStore: Send + Sync + 'static {
    // -- Session lifecycle ------------------------------------------------

    async fn create_session(
        &self,
        cluster_id: Uuid,
        instance_id: &str,
        created_by: &str,
        initial_issues: &serde_json::Value,
        state_data: &serde_json::Value,
        provider: Option<&str>,
        model: Option<&str>,
        label: Option<&str>,
    ) -> Result<Uuid>;

    async fn set_label(&self, session_id: Uuid, label: &str) -> Result<()>;

    /// Transition a session to a new state.
    /// Implementations MUST also persist a `state_change` chat message with
    /// the reason (if present in `state_data`).
    async fn transition_state(
        &self,
        session_id: Uuid,
        new_state: &SessionState,
        state_data: &serde_json::Value,
    ) -> Result<()>;

    /// Transition to `failed` with an error message.
    /// Implementations MUST also persist a `state_change` chat message.
    async fn fail_session(
        &self,
        session_id: Uuid,
        error_message: &str,
        state_data: &serde_json::Value,
    ) -> Result<()>;

    async fn get_session(&self, session_id: Uuid) -> Result<Option<HealerSession>>;
    async fn list_sessions(&self, cluster_id: Uuid) -> Result<Vec<HealerSession>>;
    async fn find_resumable(&self) -> Result<Vec<HealerSession>>;

    async fn update_provider_model(
        &self,
        session_id: Uuid,
        provider: &str,
        model: &str,
    ) -> Result<()>;

    // -- Messages ---------------------------------------------------------

    async fn append_message(
        &self,
        session_id: Uuid,
        role: &str,
        content: &str,
        metadata: Option<&serde_json::Value>,
    ) -> Result<()>;

    async fn get_messages(&self, session_id: Uuid) -> Result<Vec<HealerMessage>>;

    async fn get_messages_after(
        &self,
        session_id: Uuid,
        after: DateTime<Utc>,
    ) -> Result<Vec<HealerMessage>>;

    // -- Staff pings ------------------------------------------------------

    async fn create_staff_ping(
        &self,
        session_id: Uuid,
        cluster_id: Uuid,
        instance_id: &str,
        category: &str,
        message: &str,
    ) -> Result<Uuid>;

    async fn list_staff_pings(&self, cluster_id: Uuid) -> Result<Vec<StaffPing>>;
    async fn list_session_pings(&self, session_id: Uuid) -> Result<Vec<StaffPing>>;
    async fn resolve_staff_ping(&self, ping_id: Uuid, resolved_by: &str) -> Result<()>;

    // -- Session guards ---------------------------------------------------

    async fn has_running_session(&self, instance_id: &str) -> Result<bool>;
    async fn has_recent_session(&self, instance_id: &str) -> Result<bool>;

    // -- Proxy tokens -----------------------------------------------------

    /// Mint a short-lived proxy token. Returns `(raw_token, expires_at)`.
    async fn mint_proxy_token(
        &self,
        cluster_id: Uuid,
        organization_id: Option<Uuid>,
    ) -> Result<(String, DateTime<Utc>)>;

    // -- Instance data (heartbeats, assessments, probes) ------------------

    async fn get_relay_proxy_url(&self, instance_id: &str) -> Result<Option<String>>;
    async fn has_recent_heartbeat(&self, instance_id: &str) -> Result<bool>;

    async fn get_probe_status(&self, instance_id: &str) -> Result<Option<ProbeStatus>>;
    async fn get_system_sample(&self, instance_id: &str) -> Result<Option<SystemSample>>;
    async fn get_inventory(&self, instance_id: &str) -> Result<Option<Inventory>>;
    async fn get_probe_history(
        &self,
        instance_id: &str,
        service: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ProbeHistoryEntry>>;

    async fn get_heartbeat_json(&self, instance_id: &str) -> Result<Option<serde_json::Value>>;
    async fn get_version_info(&self, instance_id: &str) -> Result<VersionInfo>;
    async fn get_cluster_instances(&self, cluster_id: Uuid) -> Result<Vec<ClusterInstance>>;
    async fn get_service_state(&self, instance_id: &str) -> Result<Option<ServiceState>>;

    // -- Cluster settings -------------------------------------------------

    async fn get_config(&self, cluster_id: Uuid) -> Result<Option<serde_json::Value>>;
    async fn save_config(&self, cluster_id: Uuid, config: &serde_json::Value) -> Result<()>;

    async fn list_skills(&self, cluster_id: Uuid) -> Result<Vec<SkillEntry>>;
    async fn add_skill(&self, cluster_id: Uuid, skill_channel_id: Uuid) -> Result<()>;
    async fn remove_skill(&self, cluster_id: Uuid, skill_channel_id: Uuid) -> Result<bool>;

    async fn list_mcp_servers(&self, cluster_id: Uuid) -> Result<Vec<McpServerEntry>>;
    async fn add_mcp_server(&self, cluster_id: Uuid, mcp_server_id: Uuid) -> Result<()>;
    async fn remove_mcp_server(&self, cluster_id: Uuid, mcp_server_id: Uuid) -> Result<bool>;
}

// ── Data transfer types ────────────────────────────────────────────────

pub struct ProbeStatus {
    pub services_extended: Option<serde_json::Value>,
    pub sample: Option<serde_json::Value>,
    pub reported_at: DateTime<Utc>,
}

pub struct SystemSample {
    pub sample: Option<serde_json::Value>,
    pub reported_at: DateTime<Utc>,
}

pub struct Inventory {
    pub inventory: serde_json::Value,
    pub security: serde_json::Value,
    pub collected_at: DateTime<Utc>,
}

pub struct ProbeHistoryEntry {
    pub service: String,
    pub kind: String,
    pub ok: bool,
    pub duration_ms: i64,
    pub collected_at: DateTime<Utc>,
    pub model: Option<String>,
    pub error_class: Option<String>,
    pub error_detail: Option<String>,
    pub tokens_in: Option<i32>,
    pub tokens_out: Option<i32>,
    pub first_token_ms: Option<i64>,
}

pub struct VersionInfo {
    pub heartbeat: Option<HeartbeatVersion>,
    pub daemon_versions: Vec<DaemonVersion>,
}

pub struct HeartbeatVersion {
    pub version: String,
    pub git_sha: Option<String>,
    pub reported_at: DateTime<Utc>,
}

pub struct DaemonVersion {
    pub version: String,
    pub system: String,
    pub store_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct ClusterInstance {
    pub instance_id: String,
    pub version: String,
    pub hostname: Option<String>,
    pub reported_at: DateTime<Utc>,
    pub services_extended: Option<serde_json::Value>,
}

pub struct ServiceState {
    pub services: serde_json::Value,
    pub services_extended: Option<serde_json::Value>,
    pub reported_at: DateTime<Utc>,
}

pub struct SkillEntry {
    pub id: Uuid,
    pub slug: String,
    pub channel: String,
}

pub struct McpServerEntry {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}
