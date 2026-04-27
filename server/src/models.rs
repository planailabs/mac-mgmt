use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Cluster {
    pub id: Uuid,
    pub name: String,
    pub pinned_version: Option<String>,
    pub nixpkgs_commit: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Token {
    pub id: Uuid,
    pub cluster_id: Option<Uuid>,
    pub organization_id: Option<Uuid>,
    pub token_hash: String,
    pub label: String,
    pub kind: String,
    pub revoked: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterConfig {
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub config_json: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Skill {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub hide_from_public_catalog: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct SkillChannel {
    pub id: Uuid,
    pub skill_id: Uuid,
    pub channel: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Bundle {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub hide_from_public_catalog: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct BundleItem {
    pub id: Uuid,
    pub bundle_id: Uuid,
    pub skill_channel_id: Uuid,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterSkill {
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub skill_channel_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterBundle {
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub bundle_id: Uuid,
    pub created_at: DateTime<Utc>,
}

// ── MCP Server models ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct McpServer {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub config_json: serde_json::Value,
    pub nix_packages: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub hide_from_public_catalog: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct McpServerBundle {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub hide_from_public_catalog: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct McpServerBundleItem {
    pub id: Uuid,
    pub bundle_id: Uuid,
    pub mcp_server_id: Uuid,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterMcpServer {
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub mcp_server_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterMcpBundle {
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub bundle_id: Uuid,
    pub created_at: DateTime<Utc>,
}

// ── Skill Center models ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct SkillCenter {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub federation_token: String,
    pub priority: i32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterRemoteSkill {
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub skill_center_id: Uuid,
    pub remote_skill_channel_id: Uuid,
    pub slug: String,
    pub channel: String,
    pub skill_name: String,
    pub created_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterRemoteBundle {
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub skill_center_id: Uuid,
    pub remote_bundle_id: Uuid,
    pub slug: String,
    pub bundle_name: String,
    pub created_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterRemoteMcpServer {
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub skill_center_id: Uuid,
    pub remote_mcp_server_id: Uuid,
    pub slug: String,
    pub mcp_name: String,
    pub created_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterRemoteMcpBundle {
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub skill_center_id: Uuid,
    pub remote_bundle_id: Uuid,
    pub slug: String,
    pub bundle_name: String,
    pub created_at: DateTime<Utc>,
}

// ── Heartbeat models ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct DaemonHeartbeat {
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub instance_id: String,
    pub version: String,
    pub services: serde_json::Value,
    pub reported_at: DateTime<Utc>,
}

// ── Rollout models ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct RolloutGroup {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct RolloutGroupMember {
    pub id: Uuid,
    pub group_id: Uuid,
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Rollout {
    pub id: Uuid,
    pub target_version: Option<String>,
    pub nixpkgs_commit: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct RolloutStage {
    pub id: Uuid,
    pub rollout_id: Uuid,
    pub group_id: Uuid,
    pub stage_order: i32,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ── User & Organization models ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct OrganizationMember {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct OrganizationCluster {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub cluster_id: Uuid,
    pub created_at: DateTime<Utc>,
}
