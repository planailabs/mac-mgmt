//! Cloudflare API types derived from the OpenAPI specification.

use serde::{Deserialize, Serialize};

// ── Accounts ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: Option<String>,
}

// ── Zones ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub status: String, // initializing, pending, active, moved
    #[serde(rename = "type")]
    pub zone_type: Option<String>, // full, partial, secondary
    pub paused: Option<bool>,
    pub name_servers: Option<Vec<String>>,
    pub original_name_servers: Option<Vec<String>>,
    pub account: Option<ZoneAccount>,
    pub created_on: Option<String>,
    pub modified_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneAccount {
    pub id: String,
    pub name: Option<String>,
}

// ── DNS Records ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub id: String,
    #[serde(rename = "type")]
    pub record_type: String, // A, AAAA, CNAME, MX, TXT, NS, CAA, SRV, etc.
    pub name: String,
    pub content: Option<String>,
    pub data: Option<serde_json::Value>,
    pub ttl: Option<u32>, // 1 = auto
    pub proxied: Option<bool>,
    pub proxiable: Option<bool>,
    pub comment: Option<String>,
    pub tags: Option<Vec<String>>,
    pub created_on: Option<String>,
    pub modified_on: Option<String>,
    pub priority: Option<u16>, // for MX, SRV
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDnsRecord {
    #[serde(rename = "type")]
    pub record_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxied: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDnsRecord {
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub record_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxied: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u16>,
}

// ── DNSSEC ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnssecStatus {
    pub status: String, // active, disabled
    #[serde(default)]
    pub dnssec_multi_signer: Option<bool>,
    #[serde(default)]
    pub dnssec_presigned: Option<bool>,
    #[serde(default)]
    pub modified_on: Option<String>,
}

// ── SSL/TLS ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslMode {
    pub value: String, // auto, custom
}

// ── Registrar ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainCheck {
    pub name: String,
    #[serde(default)]
    pub registrable: bool,
    pub tier: Option<String>, // standard, premium
    pub pricing: Option<DomainPricing>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainPricing {
    pub currency: String,
    pub registration_cost: Option<String>,
    pub renewal_cost: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registration {
    pub domain_name: Option<String>,
    pub status: Option<String>,
    pub auto_renew: Option<bool>,
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
    pub locked: Option<bool>,
    pub privacy_mode: Option<String>,
}

// ── Pages ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagesProject {
    pub name: String,
    pub id: Option<String>,
    pub subdomain: Option<String>,
    pub production_branch: Option<String>,
    pub created_on: Option<String>,
    pub status: Option<String>,
    pub source: Option<PagesSource>,
    pub build_config: Option<PagesBuildConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagesSource {
    #[serde(rename = "type")]
    pub source_type: Option<String>, // "github" or "gitlab"
    pub config: Option<PagesSourceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagesSourceConfig {
    pub owner: Option<String>,
    pub repo_name: Option<String>,
    pub production_branch: Option<String>,
    pub pr_comments_enabled: Option<bool>,
    pub production_deployments_enabled: Option<bool>,
    pub preview_deployment_setting: Option<String>, // "all", "none", "custom"
    pub preview_branch_includes: Option<Vec<String>>,
    pub preview_branch_excludes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagesBuildConfig {
    pub build_command: Option<String>,
    pub destination_dir: Option<String>,
    pub root_dir: Option<String>,
    pub build_caching: Option<bool>,
}

/// Custom domain attached to a Pages project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagesCustomDomain {
    pub id: Option<String>,
    pub name: String,
    pub status: Option<String>, // "active", "pending", "verifying", "moving", etc.
    #[serde(default)]
    pub verification_type: Option<String>,
    #[serde(default)]
    pub validation_data: Option<serde_json::Value>,
}

/// Request body for updating a Pages project (PATCH).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdatePagesProject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<PagesSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_config: Option<PagesBuildConfig>,
}
