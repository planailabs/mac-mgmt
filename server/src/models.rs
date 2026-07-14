use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
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
    pub nix_packages: Vec<String>,
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

// ── User & Organization models ──────────────────────────────────────

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
