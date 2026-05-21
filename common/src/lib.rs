pub mod framing;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;

pub mod config_migrate;
pub mod custom_service;
pub mod model_source;

// ── Secret wrapper for sensitive config values ──────────────────────────

/// A string value that should be treated as sensitive.
///
/// - `Debug` and `Display` print `[REDACTED]` instead of the value.
/// - Serialization is transparent (round-trips the raw string).
/// - JSON Schema includes `"x-secret": true` for downstream redaction.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Access the underlying secret value.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Consume and return the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Resolve `env:VAR` and `secret:NAME` references in-place.
    ///
    /// - `env:VAR` looks up `VAR` in the provided `env_vars` map (loaded from `.env` file).
    /// - `secret:NAME` looks up `NAME` in the provided `vault` map (fetched from server).
    /// - Literal values are left unchanged.
    pub fn resolve(
        &mut self,
        env_vars: &std::collections::HashMap<String, String>,
        vault: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        if let Some(var) = self.0.strip_prefix("env:") {
            self.0 = env_vars
                .get(var)
                .ok_or_else(|| format!("env var '{var}' not found in .env file"))?
                .clone();
        } else if let Some(name) = self.0.strip_prefix("secret:") {
            self.0 = vault
                .get(name)
                .ok_or_else(|| format!("secret '{name}' not found in vault"))?
                .clone();
        }
        Ok(())
    }
}

impl Default for Secret {
    fn default() -> Self {
        Self(String::new())
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl From<String> for Secret {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Secret {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl schemars::JsonSchema for Secret {
    fn schema_name() -> Cow<'static, str> {
        "Secret".into()
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed(concat!(module_path!(), "::Secret"))
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "x-secret": true
        })
    }
}
#[cfg(feature = "sentry")]
pub mod sentry_ext;
#[cfg(feature = "tracing-init")]
pub mod tracing_init;

// ── Wire-format types (daemon ↔ server protocol) ─────────────────────

/// Push notification sent from server to daemon via SSE.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PushEvent {
    Ping,
    SyncConfig,
    SyncSkills,
    SyncMcpServers,
    SyncSshKeys,
    SelfUpdate,
    SyncNixpkgs,
    /// Request an immediate system-assessment snapshot + probe run.
    RequestAssessment,
    /// Unified package sync (MCP + skill + manual packages).
    SyncPackages,
}

/// Daemon → server heartbeat body (`POST /api/heartbeat`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatBody {
    pub instance_id: String,
    pub version: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub environment: String,
    pub services: serde_json::Value,
    /// Exposed TCP tunnels for browser proxying through the relay.
    #[serde(default)]
    pub tunnels: serde_json::Value,
    /// Deprecated: use `relay_proxy_url` instead to derive the hostname.
    /// Kept for backwards compatibility with older daemons.
    #[serde(default)]
    pub relay_proxy_hostname: Option<String>,
    /// Full relay API URL (e.g. "https://relay.plan.ai" or "http://localhost:8080").
    /// Used by the server to call file-tunnel endpoints without extra config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_proxy_url: Option<String>,
    /// Current nixpkgs commit pin (if any) the daemon is using.
    #[serde(default)]
    pub nixpkgs_commit: Option<String>,
    /// Git commit the daemon binary was built from. Populated from the
    /// GIT_SHA env or `git rev-parse HEAD` in build.rs. Optional for
    /// backwards compatibility with older daemons.
    #[serde(default)]
    pub git_sha: Option<String>,
    /// Ed25519 public key bytes (base64-encoded SSH wire format).
    /// The server verifies that SHA-256(public_key) == instance_id.
    #[serde(default)]
    pub public_key: String,
    /// Ed25519 signature over "{instance_id}:{signed_at}" (base64-encoded).
    #[serde(default)]
    pub signature: String,
    /// Unix timestamp (seconds) included in the signed message.
    #[serde(default)]
    pub signed_at: i64,
    /// Lightweight dynamic sample collected each heartbeat (CPU, mem, disk-free, net).
    /// Optional for back-compat with older daemons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<DynamicSample>,
    /// Rolled-up extended service state (latest probe summary per service).
    /// Optional for back-compat.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services_extended: Vec<ServiceExtState>,
    /// Exposed file tunnels for remote config editing through the relay.
    /// Optional for back-compat with older daemons.
    #[serde(default)]
    pub file_tunnels: serde_json::Value,
    /// Predefined shell commands exposed by services for remote execution.
    /// Optional for back-compat with older daemons.
    #[serde(default)]
    pub shell_tunnels: serde_json::Value,
    /// Per-service dynamic samples (loaded models, active sessions, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_samples: Vec<ServiceSample>,
}

/// Small dynamic sample sent with each heartbeat. GDPR allowlist: no user data,
/// no network identifiers beyond aggregate counters, no process args.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DynamicSample {
    /// 1-minute load average.
    pub cpu_load_1m: f32,
    /// Resident memory in use, bytes.
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
    /// Swap used, bytes. 0 if disabled.
    #[serde(default)]
    pub swap_used_bytes: u64,
    /// Free bytes per mount, limited to root and /nix/store.
    #[serde(default)]
    pub disk_free: Vec<DiskFree>,
    /// Cumulative RX/TX bytes across all non-loopback interfaces.
    #[serde(default)]
    pub net_rx_bytes: u64,
    #[serde(default)]
    pub net_tx_bytes: u64,
    /// Process count (total).
    #[serde(default)]
    pub process_count: u32,
    /// Thermal pressure — platform-reported, e.g. "nominal", "fair", "serious", "critical".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal_state: Option<String>,
    /// Per-GPU state at sample time. Empty when no GPU was detected or no
    /// vendor tooling is available.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpus: Vec<GpuSample>,
}

/// Per-GPU static inventory. Collected at the 6h inventory cadence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// Vendor-local index, 0-based. Matches the index used in `GpuSample`.
    pub index: u32,
    /// "nvidia" | "amd" | "apple" | "intel" | "unknown".
    pub vendor: String,
    pub name: String,
    /// Total VRAM in bytes. 0 when unreported (integrated GPUs on macOS).
    #[serde(default)]
    pub vram_total_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_version: Option<String>,
    /// PCI bus ID on Linux ("0000:01:00.0") or the Apple subsystem name on macOS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pci_bus_id: Option<String>,
}

/// Per-GPU current state. Populated opportunistically — fields that the
/// vendor tool doesn't report stay `None` rather than faking a value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSample {
    pub index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utilization_pct: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_used_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_c: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_watts: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskFree {
    pub mount: String,
    pub free_bytes: u64,
    pub total_bytes: u64,
}

/// Rolled-up latest-probe summary for a single service, attached to heartbeat.
/// Full probe bodies go to `/api/assessment/probe`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceExtState {
    pub name: String,
    pub healthy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_probe_ok: Option<bool>,
    /// Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_probe_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_probe_duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_probe_kind: Option<String>,
}

/// Full assessment body sent to `POST /api/assessment`. Signed like heartbeat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assessment {
    pub instance_id: String,
    /// Unix seconds.
    pub collected_at: i64,
    pub inventory: Inventory,
    /// System-level security findings (SIP, firewall, FDE, etc.).
    pub security: Vec<SecurityFinding>,
    /// Per-service static inventory (version, installed models, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_inventories: Vec<ServiceInventory>,
    /// Per-service security findings (auth config, exposed APIs, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_security: Vec<ServiceSecurity>,
    /// Ed25519 signature over "{instance_id}:{collected_at}".
    pub public_key: String,
    pub signature: String,
}

/// Static system facts — refreshed every ~6h or on RequestAssessment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Inventory {
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub arch: String,
    /// Seconds since boot at collection time.
    pub uptime_secs: u64,
    pub cpu_model: String,
    pub cpu_cores_physical: u32,
    pub cpu_cores_logical: u32,
    pub mem_total_bytes: u64,
    /// Non-loopback interface names + link state. No MACs, no IPs.
    #[serde(default)]
    pub interfaces: Vec<NetInterface>,
    /// Disks by mount point + size. Root and /nix/store only.
    #[serde(default)]
    pub disks: Vec<DiskInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nix_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nixpkgs_commit: Option<String>,
    /// Coarse supervisor label (e.g. "launchd", "systemd").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supervisor: Option<String>,
    /// GPUs detected on this host. Empty when no GPU was found or no vendor
    /// tooling (nvidia-smi / rocm-smi / system_profiler) is available.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpus: Vec<GpuInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetInterface {
    pub name: String,
    pub up: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub mount: String,
    pub fs_type: String,
    pub total_bytes: u64,
}

// ── Security findings (shared between system-level and per-service) ──

/// Severity level for a security finding.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FindingSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

/// A single security finding — both system-level (SIP, firewall) and
/// per-service (auth config, exposed APIs) use the same shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    /// Machine-readable identifier (e.g. "macos_sip", "openclaw_auth_none").
    pub id: String,
    pub severity: FindingSeverity,
    /// Human-readable description.
    pub message: String,
    /// `true` = check passed (good), `false` = problem found.
    pub pass: bool,
}

// ── Per-service inventory / sample / security ──

/// Display-type hint for an [`InventoryEntry`] value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum InventoryValueType {
    String,
    Number,
    Bool,
    Json,
}

/// A single key-value fact about a service. Used for both static inventory
/// (version, installed models) and dynamic samples (loaded models, active sessions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryEntry {
    /// Machine-readable key (e.g. "version", "installed_models").
    pub id: String,
    /// Human-readable label (e.g. "Version", "Installed Models").
    pub name: String,
    /// The value — string, number, bool, array, or object.
    pub value: serde_json::Value,
    /// Hint for how to display the value.
    #[serde(rename = "type")]
    pub value_type: InventoryValueType,
}

/// Per-service static inventory facts, collected at the 6h assessment cadence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInventory {
    pub service: String,
    pub entries: Vec<InventoryEntry>,
}

/// Per-service dynamic sample, piggybacked on each heartbeat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSample {
    pub service: String,
    pub entries: Vec<InventoryEntry>,
}

/// Per-service security findings, collected at the 6h assessment cadence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSecurity {
    pub service: String,
    pub findings: Vec<SecurityFinding>,
}

/// Single probe result sent to `POST /api/assessment/probe`. Signed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeReport {
    pub instance_id: String,
    pub collected_at: i64,
    pub service: String,
    /// "liveness" | "functional" | "regression".
    pub kind: String,
    pub ok: bool,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_token_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Hex-encoded SHA-256 of the canary response for regression detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canary_digest: Option<String>,
    /// Error class (not raw message). E.g. "timeout", "bad_response", "pull_failed".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    /// Full error message — redacted for non-admin viewers server-side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_detail: Option<String>,
    pub public_key: String,
    pub signature: String,
}

/// Server → daemon update target response (`GET /api/update`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTarget {
    pub target_version: Option<String>,
    /// Nix store path containing `bin/mac-mgmt` for the daemon's system,
    /// resolved by the server from `daemon_versions`. The daemon realises
    /// it via `nix-store --realise` and self-replaces from `bin/mac-mgmt`.
    #[serde(default)]
    pub store_path: Option<String>,
}

/// Server → daemon nixpkgs pin response (`GET /api/nixpkgs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NixpkgsPin {
    /// Full git commit SHA. None means "use the rolling default source".
    pub commit: Option<String>,
}

/// Single MCP server entry in the sync response (`GET /api/mcp-servers`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerEntry {
    pub config: serde_json::Value,
    #[serde(default)]
    pub nix_packages: Vec<String>,
}

// ── Unified package manager types ────────────────────────────────────

/// Source that requires a nix package.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PackageSource {
    /// Package required by an MCP server (identified by slug).
    McpServer { slug: String },
    /// Package required directly by a skill channel (identified by skill slug).
    Skill { slug: String },
    /// Package manually added to the cluster.
    Manual,
}

/// Unified package sync response (`GET /api/packages`).
/// Maps nixpkgs attribute name → list of sources requiring it.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageSyncResponse {
    pub packages: std::collections::HashMap<String, Vec<PackageSource>>,
}

// ── Federation types (skill center ↔ management server protocol) ────

/// Push notification sent from skill center to management server via SSE.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FederationEvent {
    Ping,
    CatalogChanged,
}

/// Full catalog returned by a skill center's federation API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FederationCatalog {
    pub skill_channels: Vec<FederationSkillChannel>,
    pub bundles: Vec<FederationBundle>,
    pub mcp_servers: Vec<FederationMcpServer>,
    pub mcp_bundles: Vec<FederationMcpBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationSkillChannel {
    pub id: uuid::Uuid,
    pub skill_slug: String,
    pub skill_name: String,
    pub skill_description: String,
    pub channel: String,
    pub hidden: bool,
    #[serde(default)]
    pub nix_packages: Vec<String>,
    /// MCP server slugs this skill channel depends on (transitive deps).
    #[serde(default)]
    pub mcp_server_slugs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationBundle {
    pub id: uuid::Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub hidden: bool,
    pub skills: Vec<FederationBundleSkill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationBundleSkill {
    pub skill_channel_id: uuid::Uuid,
    pub skill_slug: String,
    pub channel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationMcpServer {
    pub id: uuid::Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub hidden: bool,
    pub config: serde_json::Value,
    pub nix_packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationMcpBundle {
    pub id: uuid::Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub hidden: bool,
    pub servers: Vec<FederationMcpBundleServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationMcpBundleServer {
    pub mcp_server_id: uuid::Uuid,
    pub slug: String,
}

/// Request body for `POST /api/federation/resolve-skills`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveSkillsRequest {
    pub skills: Vec<SkillResolveEntry>,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResolveEntry {
    pub slug: String,
    pub channel: String,
}

/// Request body for `POST /api/federation/resolve-mcp-servers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveMcpServersRequest {
    pub slugs: Vec<String>,
}

/// Single SSH key entry in the sync response (`GET /api/ssh-keys`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKeySyncEntry {
    pub public_key: String,
}

/// Daemon status response from the local metrics server (`GET /status`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub version: String,
    pub uptime_secs: u64,
    pub services: Vec<ServiceStatus>,
}

/// Per-service status in the status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub healthy: bool,
    pub upgrade_pending: bool,
    pub busy: bool,
    /// Lifecycle phase: "stopped", "starting", "healthy", "unhealthy".
    #[serde(default)]
    pub phase: String,
}

const VALID_FLAVOURS: &[&str] = &["cpu", "rocm", "cuda", "vulkan"];
const VALID_LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Ollama,
    Lms,
    Unsloth,
    Cloud,
    Litellm,
    None,
}

impl Default for LlmProvider {
    fn default() -> Self {
        Self::None
    }
}

impl LlmProvider {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ollama => "ollama",
            Self::Lms => "lms",
            Self::Unsloth => "unsloth",
            Self::Cloud => "cloud",
            Self::Litellm => "litellm",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AgentProvider {
    Openclaw,
    Opencode,
    Hermes,
    None,
}

impl Default for AgentProvider {
    fn default() -> Self {
        Self::None
    }
}

impl AgentProvider {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Openclaw => "openclaw",
            Self::Opencode => "opencode",
            Self::Hermes => "hermes",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CloudProvider {
    Anthropic,
    Openai,
    Google,
    Mistral,
    Groq,
    Xai,
    Deepseek,
    Openrouter,
    Together,
    Bedrock,
}

impl Default for CloudProvider {
    fn default() -> Self {
        Self::Anthropic
    }
}

impl CloudProvider {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Openai => "openai",
            Self::Google => "google",
            Self::Mistral => "mistral",
            Self::Groq => "groq",
            Self::Xai => "xai",
            Self::Deepseek => "deepseek",
            Self::Openrouter => "openrouter",
            Self::Together => "together",
            Self::Bedrock => "bedrock",
        }
    }

    pub fn env_var(&self) -> &str {
        match self {
            Self::Anthropic => "ANTHROPIC_API_KEY",
            Self::Openai => "OPENAI_API_KEY",
            Self::Google => "GEMINI_API_KEY",
            Self::Mistral => "MISTRAL_API_KEY",
            Self::Groq => "GROQ_API_KEY",
            Self::Xai => "XAI_API_KEY",
            Self::Deepseek => "DEEPSEEK_API_KEY",
            Self::Openrouter => "OPENROUTER_API_KEY",
            Self::Together => "TOGETHER_API_KEY",
            Self::Bedrock => "AWS_ACCESS_KEY_ID",
        }
    }
}

impl CloudProvider {
    /// Standard API base URL for this provider.
    pub fn base_url(&self) -> &str {
        match self {
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Openai => "https://api.openai.com/v1",
            Self::Google => "https://generativelanguage.googleapis.com/v1beta",
            Self::Mistral => "https://api.mistral.ai/v1",
            Self::Groq => "https://api.groq.com/openai/v1",
            Self::Xai => "https://api.x.ai/v1",
            Self::Deepseek => "https://api.deepseek.com/v1",
            Self::Openrouter => "https://openrouter.ai/api/v1",
            Self::Together => "https://api.together.xyz/v1",
            Self::Bedrock => "https://bedrock-runtime.us-east-1.amazonaws.com",
        }
    }

    pub fn default_model(&self) -> &str {
        match self {
            Self::Anthropic => "anthropic/claude-sonnet-4-6",
            Self::Openai => "openai/gpt-5.4",
            Self::Google => "google/gemini-3-flash-preview",
            Self::Mistral => "mistral/mistral-large-latest",
            Self::Groq => "groq/llama-4-scout-17b-16e-instruct",
            Self::Xai => "xai/grok-3-mini",
            Self::Deepseek => "deepseek/deepseek-chat",
            Self::Openrouter => "openrouter/auto",
            Self::Together => "together/meta-llama/Llama-4-Maverick-17B-128E-Instruct-Turbo",
            Self::Bedrock => "amazon-bedrock/us.anthropic.claude-sonnet-4-6-v1:0",
        }
    }
}

impl std::fmt::Display for CloudProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug)]
pub struct ValidationError(String);

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ValidationError {}

// ── Daemon settings (daemon-only, not in ClusterConfig) ────────────────

fn default_update_interval() -> String {
    "1h".to_string()
}

fn default_health_interval() -> String {
    "1m".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DaemonSettings {
    #[schemars(
        description = "How often to check for updates, sync skills and MCP servers (e.g. \"30s\", \"5m\", \"1h\")"
    )]
    #[serde(default = "default_update_interval")]
    pub update_interval: String,
    #[schemars(
        description = "How often to run health checks on managed services (e.g. \"1m\", \"30s\")"
    )]
    #[serde(default = "default_health_interval")]
    pub health_interval: String,
    #[schemars(description = "Log verbosity: error, warn, info, debug, or trace")]
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[schemars(
        description = "Time window for upgrades in HH:MM-HH:MM format (e.g. \"02:00-05:00\"). Omit to allow anytime.",
        extend("x-advanced" = true),
    )]
    #[serde(default)]
    pub upgrade_window: Option<String>,
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            update_interval: default_update_interval(),
            health_interval: default_health_interval(),
            log_level: default_log_level(),
            upgrade_window: None,
        }
    }
}

// ── Notifications (daemon-only) ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NotificationsConfig {
    #[schemars(
        description = "Apprise notification URLs (e.g. tgram://bot/chat, ntfy://host/topic)"
    )]
    #[serde(default)]
    pub urls: Vec<String>,
    #[schemars(
        description = "Which events trigger notifications (omit for all). Options: daemon_started, daemon_stopped, service_crashed, service_unhealthy, service_recovered, upgrade_installed, upgrade_failed"
    )]
    #[serde(default)]
    pub events: Option<Vec<String>>,
}

// ── Ollama ──────────────────────────────────────────────────────────────

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    11434
}

fn default_models() -> Vec<String> {
    vec![
        "phi4-mini".to_string(),
        "qwen3.5".to_string(),
        "Flux_AI/Flux_AI".to_string(),
    ]
}

fn default_model() -> String {
    "phi4-mini".to_string()
}

fn default_flavour() -> String {
    "cpu".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OllamaConfig {
    #[schemars(description = "Whether this provider is installed and started")]
    #[serde(default)]
    pub enabled: bool,
    #[schemars(description = "Ollama listen address")]
    #[serde(default = "default_host")]
    pub host: String,
    #[schemars(description = "Ollama listen port")]
    #[serde(default = "default_port")]
    pub port: u16,
    #[schemars(description = "Models to pull on startup; at least one required", extend("x-model-source" = "ollama"))]
    #[serde(default = "default_models")]
    pub models: Vec<String>,
    #[schemars(description = "Default model for OpenClaw to use", extend("x-model-source" = "ollama"))]
    #[serde(default = "default_model")]
    pub default_model: String,
    #[schemars(description = "Package flavour: cpu, rocm (AMD), cuda (NVIDIA), or vulkan")]
    #[serde(default = "default_flavour")]
    pub flavour: String,
    #[schemars(description = "Context length passed as OLLAMA_CONTEXT_LENGTH env var", extend("x-advanced" = true))]
    #[serde(default = "default_context_length")]
    pub context_length: u32,
}

fn default_context_length() -> u32 {
    16384
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: default_host(),
            port: default_port(),
            models: default_models(),
            default_model: default_model(),
            flavour: default_flavour(),
            context_length: default_context_length(),
        }
    }
}

// ── LM Studio (lms) ────────────────────────────────────────────────────

fn default_lms_port() -> u16 {
    1234
}

fn default_lms_models() -> Vec<String> {
    Vec::new()
}

fn default_lms_model() -> String {
    "qwen2.5-coder-7b-instruct".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LmsConfig {
    #[schemars(description = "Whether this provider is installed and started")]
    #[serde(default)]
    pub enabled: bool,
    #[schemars(description = "LM Studio listen address")]
    #[serde(default = "default_host")]
    pub host: String,
    #[schemars(description = "LM Studio listen port")]
    #[serde(default = "default_lms_port")]
    pub port: u16,
    #[schemars(description = "Model identifiers to load on startup via `lms load`", extend("x-model-source" = "lms"))]
    #[serde(default = "default_lms_models")]
    pub models: Vec<String>,
    #[schemars(description = "Default model identifier for OpenClaw to use", extend("x-model-source" = "lms"))]
    #[serde(default = "default_lms_model")]
    pub default_model: String,
}

impl Default for LmsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: default_host(),
            port: default_lms_port(),
            models: default_lms_models(),
            default_model: default_lms_model(),
        }
    }
}

impl LmsConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.port == 0 {
            return Err(ValidationError("lms.port must be > 0".into()));
        }
        if self.models.iter().any(|m| m.is_empty()) {
            return Err(ValidationError("lms.models contains an empty string".into()));
        }
        Ok(())
    }
}

// ── Unsloth ────────────────────────────────────────────────────────────

fn default_unsloth_port() -> u16 {
    8888
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnslothConfig {
    #[schemars(description = "Whether Unsloth Studio is installed and started")]
    #[serde(default)]
    pub enabled: bool,
    #[schemars(description = "Unsloth Studio listen address")]
    #[serde(default = "default_host")]
    pub host: String,
    #[schemars(description = "Unsloth Studio listen port")]
    #[serde(default = "default_unsloth_port")]
    pub port: u16,
    #[schemars(description = "Default model identifier for agents to use", extend("x-model-source" = "custom-only"))]
    #[serde(default)]
    pub default_model: String,
}

impl Default for UnslothConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: default_host(),
            port: default_unsloth_port(),
            default_model: String::new(),
        }
    }
}

impl UnslothConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.port == 0 {
            return Err(ValidationError("unsloth.port must be > 0".into()));
        }
        Ok(())
    }
}

// ── LiteLLM proxy ─────────────────────────────────────────────────────

fn default_litellm_port() -> u16 {
    4100
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LitellmConfig {
    #[schemars(description = "Whether LiteLLM proxy is enabled")]
    #[serde(default)]
    pub enabled: bool,
    #[schemars(description = "LiteLLM proxy listen address")]
    #[serde(default = "default_host")]
    pub host: String,
    #[schemars(description = "LiteLLM proxy listen port")]
    #[serde(default = "default_litellm_port")]
    pub port: u16,
    #[schemars(description = "Master key for LiteLLM proxy authentication")]
    #[serde(default)]
    pub master_key: Option<Secret>,
}

impl Default for LitellmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: default_host(),
            port: default_litellm_port(),
            master_key: None,
        }
    }
}

impl LitellmConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.port == 0 {
            return Err(ValidationError("litellm.port must be > 0".into()));
        }
        Ok(())
    }
}

// ── Cloud LLM providers ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CloudApiType {
    AnthropicMessages,
    OpenaiCompletions,
    OpenaiResponses,
    GoogleGenerativeAi,
    BedrockConverseStream,
    Ollama,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CloudAuthMode {
    ApiKey,
    AwsSdk,
    Oauth,
    Token,
}

fn default_cloud_model() -> String {
    "anthropic/claude-sonnet-4-6".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CloudConfig {
    #[schemars(description = "Whether this cloud provider entry is active")]
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[schemars(description = "Cloud LLM provider")]
    #[serde(default)]
    pub provider: CloudProvider,
    #[schemars(description = "API key for the cloud provider")]
    #[serde(default)]
    pub api_key: Option<Secret>,
    #[schemars(description = "Models available via this cloud provider", extend("x-model-source" = "cloud"))]
    #[serde(default)]
    pub models: Vec<String>,
    #[schemars(description = "Default model (e.g. anthropic/claude-sonnet-4-6, openai/gpt-5.4)", extend("x-model-source" = "cloud"))]
    #[serde(default = "default_cloud_model")]
    pub default_model: String,
    #[schemars(description = "Custom base URL (for proxies, Bedrock, etc.)", extend("x-advanced" = true))]
    #[serde(default)]
    pub base_url: Option<String>,
    #[schemars(description = "API type override for custom providers", extend("x-advanced" = true))]
    #[serde(default)]
    pub api: Option<CloudApiType>,
    #[schemars(description = "Authentication mode", extend("x-advanced" = true))]
    #[serde(default)]
    pub auth: Option<CloudAuthMode>,
}

impl CloudConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.default_model.is_empty() {
            return Err(ValidationError("cloud.default_model must not be empty".into()));
        }
        Ok(())
    }
}

// ── OpenClaw ────────────────────────────────────────────────────────────

fn default_gateway_port() -> u16 {
    18789
}

fn default_gateway_host() -> String {
    "127.0.0.1".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenClawGatewayConfig {
    #[schemars(description = "OpenClaw gateway listen port")]
    #[serde(default = "default_gateway_port")]
    pub port: u16,
    #[schemars(description = "OpenClaw gateway listen address")]
    #[serde(default = "default_gateway_host")]
    pub host: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenClawSkillsConfig {
    #[schemars(description = "Automatically update skills on the update interval")]
    #[serde(default = "default_true")]
    pub auto_update: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenClawTelegramConfig {
    #[schemars(description = "Telegram bot token from @BotFather")]
    pub bot_token: Secret,
    #[schemars(description = "Allowed Telegram chat IDs. If empty, all chats are allowed.")]
    #[serde(default)]
    pub allowed_chat_ids: Vec<i64>,
    #[schemars(description = "Enable the Telegram integration")]
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenClawConfig {
    #[schemars(description = "Whether the OpenClaw agent is installed and started")]
    #[serde(default)]
    pub enabled: bool,
    #[schemars(description = "Gateway settings merged into ~/.openclaw/openclaw.json")]
    #[serde(default)]
    pub gateway: Option<OpenClawGatewayConfig>,
    #[schemars(description = "Skills auto-update behavior")]
    #[serde(default)]
    pub skills: Option<OpenClawSkillsConfig>,
    #[schemars(description = "Telegram bot integration")]
    #[serde(default)]
    pub telegram: Option<OpenClawTelegramConfig>,
    #[schemars(
        description = "Arbitrary key-value pairs merged into openclaw.json after typed fields"
    )]
    #[serde(default)]
    pub extra_config: Option<serde_json::Value>,
}

impl Default for OpenClawConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            gateway: None,
            skills: None,
            telegram: None,
            extra_config: None,
        }
    }
}

// ── OpenCode ───────────────────────────────────────────────────────────

fn default_opencode_port() -> u16 {
    18790
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpencodeConfig {
    #[schemars(description = "Whether the OpenCode agent is installed and started")]
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[schemars(description = "OpenCode server listen port")]
    #[serde(default = "default_opencode_port")]
    pub port: u16,
    #[schemars(description = "OpenCode server listen address")]
    #[serde(default = "default_host")]
    pub host: String,
    #[schemars(
        description = "Arbitrary key-value pairs merged into the opencode config after typed fields"
    )]
    #[serde(default)]
    pub extra_config: Option<serde_json::Value>,
}

impl Default for OpencodeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_opencode_port(),
            host: default_host(),
            extra_config: None,
        }
    }
}

// ── Hermes ────────────────────────────────────────────────────────────

fn default_hermes_port() -> u16 {
    8642
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HermesGatewayConfig {
    #[schemars(description = "Hermes API server listen port")]
    #[serde(default = "default_hermes_port")]
    pub port: u16,
    #[schemars(description = "Hermes API server listen address")]
    #[serde(default = "default_gateway_host")]
    pub host: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HermesTelegramConfig {
    #[schemars(description = "Telegram bot token from @BotFather")]
    pub bot_token: Secret,
    #[schemars(description = "Allowed Telegram chat IDs. If empty, all chats are allowed.")]
    #[serde(default)]
    pub allowed_chat_ids: Vec<i64>,
    #[schemars(description = "Enable the Telegram integration")]
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HermesConfig {
    #[schemars(description = "Whether the Hermes agent is installed and started")]
    #[serde(default)]
    pub enabled: bool,
    #[schemars(description = "Gateway/API server settings")]
    #[serde(default)]
    pub gateway: Option<HermesGatewayConfig>,
    #[schemars(description = "Telegram bot integration")]
    #[serde(default)]
    pub telegram: Option<HermesTelegramConfig>,
    #[schemars(description = "Working directory for agent terminal sessions (maps to terminal.cwd in config.yaml)")]
    #[serde(default)]
    pub cwd: Option<String>,
    #[schemars(
        description = "Arbitrary key-value pairs merged into ~/.hermes/config.yaml after typed fields"
    )]
    #[serde(default)]
    pub extra_config: Option<serde_json::Value>,
    #[schemars(description = "Extra environment variables written to ~/.hermes/.env")]
    #[serde(default)]
    pub extra_env: Option<std::collections::HashMap<String, Secret>>,
    #[schemars(description = "Web dashboard settings")]
    #[serde(default)]
    pub dashboard: Option<HermesDashboardConfig>,
    #[schemars(description = "Hermes WebUI (nesquena/hermes-webui) settings")]
    #[serde(default)]
    pub webui: Option<HermesWebuiConfig>,
}

impl Default for HermesConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            gateway: None,
            telegram: None,
            cwd: None,
            extra_config: None,
            extra_env: None,
            dashboard: None,
            webui: None,
        }
    }
}

// ── Hermes Dashboard ──────────────────────────────────────────────────

fn default_hermes_dashboard_port() -> u16 {
    9119
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HermesDashboardConfig {
    #[schemars(description = "Whether the Hermes dashboard is installed and started")]
    #[serde(default)]
    pub enabled: bool,
    #[schemars(description = "Dashboard listen port")]
    #[serde(default = "default_hermes_dashboard_port")]
    pub port: u16,
    #[schemars(description = "Dashboard listen address")]
    #[serde(default = "default_gateway_host")]
    pub host: String,
}

impl Default for HermesDashboardConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_hermes_dashboard_port(),
            host: default_gateway_host(),
        }
    }
}

// ── Hermes WebUI ──────────────────────────────────────────────────────

fn default_hermes_webui_port() -> u16 {
    8787
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HermesWebuiConfig {
    #[schemars(description = "Whether the Hermes WebUI is installed and started")]
    #[serde(default)]
    pub enabled: bool,
    #[schemars(description = "WebUI listen port")]
    #[serde(default = "default_hermes_webui_port")]
    pub port: u16,
    #[schemars(description = "WebUI listen address")]
    #[serde(default = "default_gateway_host")]
    pub host: String,
    #[schemars(
        description = "Optional password for the WebUI. When set, written to \
                       $HERMES_HOME/webui/.env as HERMES_WEBUI_PASSWORD so the \
                       login flow gates browser access."
    )]
    #[serde(default)]
    pub password: Option<Secret>,
    #[schemars(
        description = "Default workspace directory shown in the UI on first launch \
                       (HERMES_WEBUI_DEFAULT_WORKSPACE)."
    )]
    #[serde(default)]
    pub default_workspace: Option<String>,
}

impl Default for HermesWebuiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_hermes_webui_port(),
            host: default_gateway_host(),
            password: None,
            default_workspace: None,
        }
    }
}

// ── Metrics ─────────────────────────────────────────────────────────────

fn default_metrics_port() -> u16 {
    9396
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MetricsConfig {
    #[schemars(description = "Prometheus metrics endpoint port")]
    #[serde(default = "default_metrics_port")]
    pub port: u16,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            port: default_metrics_port(),
        }
    }
}

// ── Server (daemon → server connection) ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DaemonServerConfig {
    pub url: Option<String>,
    pub token: Option<Secret>,
}

// ── Global ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GlobalConfig {
    #[schemars(description = "Default LLM backend (ollama, lms, cloud, or none)")]
    #[serde(default, alias = "llm_provider")]
    pub default_llm: LlmProvider,
    #[schemars(description = "Default agent provider (openclaw or none)")]
    #[serde(default, alias = "agent_provider")]
    pub default_agent: AgentProvider,
    #[schemars(description = "Display name for this agent")]
    #[serde(default)]
    pub agent_name: Option<String>,
    #[schemars(description = "Display name for the user")]
    #[serde(default)]
    pub user_name: Option<String>,
}

impl GlobalConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        Ok(())
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Deserializes either a single `T` or a `Vec<T>`, enabling backwards-compat
/// for fields that changed from a single object to a list (e.g. `[cloud]` → `[[cloud]]`).
fn deserialize_one_or_many<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    use serde::de;

    struct OneOrManyVisitor<T>(std::marker::PhantomData<T>);

    impl<'de, T: serde::Deserialize<'de>> de::Visitor<'de> for OneOrManyVisitor<T> {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a single object or an array of objects")
        }

        fn visit_seq<A>(self, seq: A) -> Result<Vec<T>, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            Vec::deserialize(de::value::SeqAccessDeserializer::new(seq))
        }

        fn visit_map<M>(self, map: M) -> Result<Vec<T>, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            T::deserialize(de::value::MapAccessDeserializer::new(map)).map(|v| vec![v])
        }
    }

    deserializer.deserialize_any(OneOrManyVisitor(std::marker::PhantomData))
}

/// Concrete wrapper for `CloudConfig` lists (serde `deserialize_with` can't use turbofish).
fn deserialize_cloud_list<'de, D>(deserializer: D) -> Result<Vec<CloudConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_one_or_many(deserializer)
}

// ── AI API Proxy ──────────────────────────────────────────────────────

fn default_ai_proxy_port() -> u16 {
    18900
}

fn default_budget_window() -> String {
    "24h".to_string()
}

/// Concrete wrapper for `AiProxyKeyConfig` lists.
fn deserialize_ai_proxy_key_list<'de, D>(
    deserializer: D,
) -> Result<Vec<AiProxyKeyConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_one_or_many(deserializer)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AiProxyConfig {
    #[schemars(description = "Whether the AI API proxy is enabled")]
    #[serde(default)]
    pub enabled: bool,
    #[schemars(description = "Proxy listen port")]
    #[serde(default = "default_ai_proxy_port")]
    pub port: u16,
    #[schemars(description = "Proxy listen address")]
    #[serde(default = "default_host")]
    pub host: String,
    #[schemars(description = "API keys with per-key token budgets")]
    #[serde(default, deserialize_with = "deserialize_ai_proxy_key_list")]
    pub keys: Vec<AiProxyKeyConfig>,
    /// Plaintext probe token, generated at runtime. Never serialized.
    #[serde(skip)]
    #[schemars(skip)]
    pub probe_token: Option<String>,
}

impl Default for AiProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_ai_proxy_port(),
            host: default_host(),
            keys: Vec::new(),
            probe_token: None,
        }
    }
}

impl AiProxyConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.port == 0 {
            return Err(ValidationError("ai_proxy.port must be > 0".into()));
        }
        for (i, key) in self.keys.iter().enumerate() {
            key.validate()
                .map_err(|e| ValidationError(format!("ai_proxy.keys[{i}]: {e}")))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AiProxyKeyConfig {
    #[schemars(description = "Human-readable label for this key")]
    #[serde(default)]
    pub name: String,
    #[schemars(
        description = "Hex-encoded multihash of the API key (the raw key is only shown once on generation)"
    )]
    pub key_hash: String,
    #[schemars(description = "Maximum total tokens (input+output) within the budget window. 0 = unlimited")]
    #[serde(default)]
    pub token_budget: i64,
    #[schemars(
        description = "Sliding window duration for the token budget (e.g. \"24h\", \"7d\", \"1h\")"
    )]
    #[serde(default = "default_budget_window")]
    pub budget_window: String,
    #[schemars(description = "Whether this key is active")]
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for AiProxyKeyConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            key_hash: String::new(),
            token_budget: 0,
            budget_window: default_budget_window(),
            enabled: true,
        }
    }
}

impl AiProxyKeyConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.key_hash.is_empty() {
            return Err(ValidationError("key_hash must not be empty".into()));
        }
        if self.token_budget < 0 {
            return Err(ValidationError("token_budget must be >= 0".into()));
        }
        if !self.budget_window.is_empty() {
            humantime::parse_duration(&self.budget_window).map_err(|e| {
                ValidationError(format!("invalid budget_window '{}': {e}", self.budget_window))
            })?;
        }
        Ok(())
    }
}

// ── Healer per-cluster settings ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HealerClusterConfig {
    #[schemars(description = "Enable auto-triggered healer sessions for this cluster")]
    #[serde(default)]
    pub auto_trigger: Option<bool>,
    #[schemars(description = "Provider for auto-triggered sessions")]
    #[serde(default)]
    pub auto_trigger_provider: Option<String>,
    #[schemars(description = "Model for auto-triggered sessions")]
    #[serde(default)]
    pub auto_trigger_model: Option<String>,
    #[schemars(description = "Auto-approve remediation (skip approval gate)")]
    #[serde(default)]
    pub auto_approve: Option<bool>,
    #[schemars(description = "Provider for the fix-model (remediation phase)")]
    #[serde(default)]
    pub fix_provider: Option<String>,
    #[schemars(description = "Model for the fix-model (remediation phase)")]
    #[serde(default)]
    pub fix_model: Option<String>,
    #[schemars(description = "Fine-tuned Ollama model name (e.g. 'mac-mgmt-healer'). Preferred over ollama_model when present in Ollama.")]
    #[serde(default)]
    pub fine_tuned_model: Option<String>,
}

// ── Backup (restic) ───────────────────────────────────────────────────

fn default_backup_interval() -> String {
    "6h".to_string()
}

fn default_backup_keep() -> String {
    "7d".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BackupConfig {
    #[schemars(description = "Whether restic backups are enabled")]
    #[serde(default)]
    pub enabled: bool,
    #[schemars(
        description = "Restic repository path or URL (e.g. /backup/restic, s3:bucket/prefix, sftp:host:/path)"
    )]
    #[serde(default)]
    pub repository: String,
    #[schemars(description = "Path to the restic password/key file for repository encryption. Auto-generated if omitted.", extend("x-advanced" = true))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_file: Option<String>,
    #[schemars(description = "How often to run backups (e.g. \"6h\", \"1d\")")]
    #[serde(default = "default_backup_interval")]
    pub interval: String,
    #[schemars(description = "Retention policy: keep snapshots from the last N duration (e.g. \"7d\", \"30d\")")]
    #[serde(default = "default_backup_keep")]
    pub keep_within: String,
    #[schemars(description = "Extra paths to include in backups (beyond auto-detected service paths)", extend("x-advanced" = true))]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_paths: Vec<String>,
    #[schemars(description = "Glob patterns to exclude from backups", extend("x-advanced" = true))]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    #[schemars(description = "Whether to include large model caches (ollama models, lm-studio cache). Default false.", extend("x-advanced" = true))]
    #[serde(default)]
    pub include_models: bool,
    #[schemars(description = "Environment variables for restic (e.g. AWS_ACCESS_KEY_ID for S3 backends)", extend("x-advanced" = true))]
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub env: std::collections::HashMap<String, String>,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            repository: String::new(),
            password_file: None,
            interval: default_backup_interval(),
            keep_within: default_backup_keep(),
            extra_paths: Vec::new(),
            exclude: Vec::new(),
            include_models: false,
            env: std::collections::HashMap::new(),
        }
    }
}

impl BackupConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.enabled && self.repository.is_empty() {
            return Err(ValidationError(
                "backup.repository must be set when backup is enabled".into(),
            ));
        }
        if self.enabled {
            humantime::parse_duration(&self.interval).map_err(|e| {
                ValidationError(format!("invalid backup.interval '{}': {e}", self.interval))
            })?;
            humantime::parse_duration(&self.keep_within).map_err(|e| {
                ValidationError(format!(
                    "invalid backup.keep_within '{}': {e}",
                    self.keep_within
                ))
            })?;
        }
        Ok(())
    }
}

// ── Memvault Config ────────────────────────────────────────────────────

fn default_memvault_port() -> u16 {
    8401
}

/// Configuration for the memvault subsystem.
#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct MemvaultConfig {
    #[schemars(description = "Whether memvault is enabled")]
    #[serde(default)]
    pub enabled: bool,
    #[schemars(description = "Data directory for memvault storage (redb, identity, etc.)", extend("x-advanced" = true))]
    #[serde(default)]
    pub data_dir: String,
    #[schemars(
        description = "Cluster ID (base58-encoded 32 bytes, or \"auto\" to generate on first run)",
        extend("x-advanced" = true),
    )]
    #[serde(default)]
    pub cluster_id: String,
    #[schemars(description = "Bootstrap peers for Kademlia and initial connections")]
    #[serde(default)]
    pub bootstrap_peers: Vec<String>,
    #[schemars(description = "Port for the memvault API server (default 8401)")]
    #[serde(default = "default_memvault_port")]
    pub port: u16,
}

// ── Cluster Config (what the server manages per-cluster) ──────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClusterConfig {
    #[serde(default)]
    #[schemars(extend("x-category" = "infra"))]
    pub daemon: DaemonSettings,
    #[serde(default)]
    #[schemars(extend("x-category" = "ops"))]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "identity", "x-always-on" = true))]
    pub global: GlobalConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "agents"))]
    pub openclaw: OpenClawConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "agents"))]
    pub opencode: OpencodeConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "agents"))]
    pub hermes: HermesConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "llm-providers"))]
    pub ollama: OllamaConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "llm-providers"))]
    pub lms: LmsConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "llm-providers"))]
    pub unsloth: UnslothConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "llm-providers"))]
    pub litellm: LitellmConfig,
    #[schemars(
        description = "Cloud LLM provider entries (list of providers)",
        extend("x-category" = "llm-providers", "x-array-entry-label" = "provider"),
    )]
    #[serde(default, deserialize_with = "deserialize_cloud_list")]
    pub cloud: Vec<CloudConfig>,
    #[serde(default)]
    #[schemars(extend("x-category" = "ops"))]
    pub metrics: MetricsConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "infra"))]
    pub relay: RelayConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "infra"))]
    pub ai_proxy: AiProxyConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "ops"))]
    pub healer: HealerClusterConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "ops"))]
    pub backup: BackupConfig,
    #[serde(default)]
    #[schemars(extend("x-category" = "infra"))]
    pub memvault: MemvaultConfig,
    #[schemars(extend("x-category" = "custom", "x-array-entry-label" = "name"))]
    #[serde(default, rename = "custom-service")]
    pub custom_services: Vec<custom_service::CustomServiceConfig>,
}

impl OllamaConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !VALID_FLAVOURS.contains(&self.flavour.as_str()) {
            return Err(ValidationError(format!(
                "invalid ollama flavour '{}', must be one of: {}",
                self.flavour,
                VALID_FLAVOURS.join(", ")
            )));
        }
        if self.models.is_empty() {
            return Err(ValidationError("ollama models list cannot be empty".into()));
        }
        Ok(())
    }
}

impl ClusterConfig {
    /// Parse and validate a TOML string as a cluster config.
    pub fn from_toml(toml_str: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(toml_str).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }

    /// Parse and validate a JSON value as a cluster config.
    pub fn from_json(json: &serde_json::Value) -> Result<Self, String> {
        let config: Self = serde_json::from_value(json.clone()).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.daemon.validate().map_err(|e| e.to_string())?;
        self.global.validate().map_err(|e| e.to_string())?;
        if self.ollama.enabled {
            self.ollama.validate().map_err(|e| e.to_string())?;
        }
        if self.lms.enabled {
            self.lms.validate().map_err(|e| e.to_string())?;
        }
        if self.unsloth.enabled {
            self.unsloth.validate().map_err(|e| e.to_string())?;
        }
        if self.litellm.enabled {
            self.litellm.validate().map_err(|e| e.to_string())?;
        }
        for (i, c) in self.cloud.iter().enumerate() {
            if c.enabled {
                c.validate().map_err(|e| format!("cloud[{i}]: {e}"))?;
            }
        }
        if self.ai_proxy.enabled {
            self.ai_proxy.validate().map_err(|e| e.to_string())?;
        }
        if self.backup.enabled {
            self.backup.validate().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Return the first enabled cloud entry, if any.
    pub fn default_cloud(&self) -> Option<&CloudConfig> {
        self.cloud.iter().find(|c| c.enabled)
    }

    /// Resolve `env:` and `secret:` references in all Secret fields.
    pub fn resolve_secrets(
        &mut self,
        env_vars: &std::collections::HashMap<String, String>,
        vault: &std::collections::HashMap<String, String>,
    ) -> Result<(), Vec<String>> {
        let mut errors = vec![];
        for (i, cloud) in self.cloud.iter_mut().enumerate() {
            if let Some(ref mut key) = cloud.api_key {
                if let Err(e) = key.resolve(env_vars, vault) {
                    errors.push(format!("cloud[{i}].api_key: {e}"));
                }
            }
        }
        if let Some(ref mut tg) = self.openclaw.telegram {
            if let Err(e) = tg.bot_token.resolve(env_vars, vault) {
                errors.push(format!("openclaw.telegram.bot_token: {e}"));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ── Relay ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct RelayConfig {
    #[schemars(description = "Whether remote SSH access is enabled on startup")]
    #[serde(default)]
    pub remote_ssh_enabled: bool,
    #[schemars(
        description = "Whether to expose service tunnels (TCP, file, shell) via the relay"
    )]
    #[serde(default = "default_true")]
    pub tunnels_enabled: bool,
    #[schemars(
        description = "Rewrite Host, Referer, Origin and strip forwarding headers when proxying TCP tunnels. Prevents services (e.g. Ollama) from rejecting requests with non-local origins. Default true.",
        extend("x-advanced" = true),
    )]
    #[serde(default = "default_true")]
    pub fake_origin_local: bool,
    #[schemars(
        description = "Cluster pre-shared key for p2p peer authentication (hex-encoded, 32 bytes). Generated by the server.",
        extend("x-advanced" = true),
    )]
    #[serde(default)]
    pub cluster_psk: Option<Secret>,
    #[schemars(
        description = "Relay node libp2p multiaddress for circuit relay (e.g. \"/dns4/relay.example.com/tcp/4001/wss\")"
    )]
    #[serde(default, alias = "url")]
    pub relay_multiaddr: Option<String>,
    #[schemars(description = "Enable mDNS local peer discovery (default true)", extend("x-advanced" = true))]
    #[serde(default = "default_true")]
    pub mdns_enabled: bool,
    #[schemars(description = "QUIC listen port for p2p connections (default 1122)", extend("x-advanced" = true))]
    #[serde(default = "default_p2p_port")]
    pub p2p_port: u16,
    #[schemars(
        description = "Enable AI proxy request distribution across cluster peers (default true)",
        extend("x-advanced" = true),
    )]
    #[serde(default = "default_true")]
    pub ai_proxy_distribution: bool,
}

fn default_p2p_port() -> u16 {
    1122
}

impl RelayConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(ref psk) = self.cluster_psk {
            let psk_str = psk.expose();
            if !psk_str.is_empty() && psk_str.len() != 64 {
                return Err(ValidationError(format!(
                    "relay.cluster_psk must be exactly 64 hex characters (32 bytes), got {} chars",
                    psk_str.len()
                )));
            }
            if !psk_str.is_empty() && !psk_str.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ValidationError(
                    "relay.cluster_psk must be valid hex".into(),
                ));
            }
        }
        Ok(())
    }
}

// ── Daemon Config (full config including server section) ────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DaemonConfig {
    #[serde(default)]
    pub daemon: DaemonSettings,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default)]
    pub openclaw: OpenClawConfig,
    #[serde(default)]
    pub opencode: OpencodeConfig,
    #[serde(default)]
    pub hermes: HermesConfig,
    #[serde(default)]
    pub ollama: OllamaConfig,
    #[serde(default)]
    pub lms: LmsConfig,
    #[serde(default)]
    pub unsloth: UnslothConfig,
    #[serde(default)]
    pub litellm: LitellmConfig,
    #[serde(default, deserialize_with = "deserialize_cloud_list")]
    pub cloud: Vec<CloudConfig>,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub server: DaemonServerConfig,
    #[serde(default)]
    pub relay: RelayConfig,
    #[serde(default)]
    pub ai_proxy: AiProxyConfig,
    #[serde(default)]
    pub healer: HealerClusterConfig,
    #[serde(default)]
    pub backup: BackupConfig,
    #[serde(default)]
    pub memvault: MemvaultConfig,
    #[serde(default, rename = "custom-service")]
    pub custom_services: Vec<custom_service::CustomServiceConfig>,
}

impl DaemonSettings {
    pub fn validate(&self) -> Result<(), ValidationError> {
        humantime::parse_duration(&self.update_interval).map_err(|e| {
            ValidationError(format!(
                "invalid update_interval '{}': {e}",
                self.update_interval
            ))
        })?;
        humantime::parse_duration(&self.health_interval).map_err(|e| {
            ValidationError(format!(
                "invalid health_interval '{}': {e}",
                self.health_interval
            ))
        })?;
        if !VALID_LOG_LEVELS.contains(&self.log_level.as_str()) {
            return Err(ValidationError(format!(
                "invalid log_level '{}', must be one of: {}",
                self.log_level,
                VALID_LOG_LEVELS.join(", ")
            )));
        }
        if let Some(ref window) = self.upgrade_window {
            parse_time_window(window).map_err(|e| {
                ValidationError(format!("invalid upgrade_window '{}': {e}", window))
            })?;
        }
        Ok(())
    }
}

impl DaemonConfig {
    /// Parse and validate a TOML string as a daemon config.
    pub fn from_toml(toml_str: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(toml_str).map_err(|e| e.to_string())?;
        config.daemon.validate().map_err(|e| e.to_string())?;
        config.global.validate().map_err(|e| e.to_string())?;
        config.ollama.validate().map_err(|e| e.to_string())?;
        config.litellm.validate().map_err(|e| e.to_string())?;
        config.relay.validate().map_err(|e| e.to_string())?;
        // Validate custom services and check for duplicate names.
        let mut seen_names = std::collections::HashSet::new();
        for cs in &config.custom_services {
            cs.validate().map_err(|e| e.to_string())?;
            if !seen_names.insert(&cs.name) {
                return Err(format!(
                    "custom-service: duplicate name '{}'",
                    cs.name
                ));
            }
        }
        Ok(config)
    }

    /// Resolve `env:` and `secret:` references in all Secret fields,
    /// including the server token and all ClusterConfig fields.
    pub fn resolve_secrets(
        &mut self,
        env_vars: &std::collections::HashMap<String, String>,
        vault: &std::collections::HashMap<String, String>,
    ) -> Result<(), Vec<String>> {
        let mut errors = vec![];
        if let Some(ref mut token) = self.server.token {
            if let Err(e) = token.resolve(env_vars, vault) {
                errors.push(format!("server.token: {e}"));
            }
        }
        // Resolve ClusterConfig fields (cloud keys, bot token, etc.)
        for (i, cloud) in self.cloud.iter_mut().enumerate() {
            if let Some(ref mut key) = cloud.api_key {
                if let Err(e) = key.resolve(env_vars, vault) {
                    errors.push(format!("cloud[{i}].api_key: {e}"));
                }
            }
        }
        if let Some(ref mut tg) = self.openclaw.telegram {
            if let Err(e) = tg.bot_token.resolve(env_vars, vault) {
                errors.push(format!("openclaw.telegram.bot_token: {e}"));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

use chrono::NaiveTime;

pub fn parse_time_window(s: &str) -> Result<(NaiveTime, NaiveTime), String> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        return Err("expected format HH:MM-HH:MM".to_string());
    }
    let start = NaiveTime::parse_from_str(parts[0].trim(), "%H:%M")
        .map_err(|e| format!("invalid start time: {e}"))?;
    let end = NaiveTime::parse_from_str(parts[1].trim(), "%H:%M")
        .map_err(|e| format!("invalid end time: {e}"))?;
    Ok((start, end))
}

pub fn is_within_window_at(start: NaiveTime, end: NaiveTime, now: NaiveTime) -> bool {
    if start <= end {
        now >= start && now < end
    } else {
        now >= start || now < end
    }
}

pub fn is_within_window(start: NaiveTime, end: NaiveTime) -> bool {
    is_within_window_at(start, end, chrono::Local::now().time())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_minimal_config() {
        let config = ClusterConfig::from_toml("").unwrap();
        assert_eq!(config.ollama.flavour, "cpu");
        assert_eq!(config.global.default_llm, LlmProvider::None);
        assert_eq!(config.global.default_agent, AgentProvider::None);
        assert!(!config.ollama.enabled);
        assert!(!config.lms.enabled);
        assert!(!config.openclaw.enabled);
        assert!(config.cloud.is_empty());
    }

    #[test]
    fn valid_full_config() {
        let toml = r#"
[global]
default_llm = "ollama"

[ollama]
host = "10.0.0.1"
port = 11435
models = ["qwen3.5"]
default_model = "qwen3.5"
flavour = "rocm"

[metrics]
port = 9000
"#;
        let config = ClusterConfig::from_toml(toml).unwrap();
        assert_eq!(config.ollama.host, "10.0.0.1");
        assert_eq!(config.ollama.port, 11435);
        assert_eq!(config.ollama.flavour, "rocm");
        assert_eq!(config.metrics.port, 9000);
    }

    #[test]
    fn rejects_unknown_field() {
        let toml = r#"
[ollama]
bogus = true
"#;
        let err = ClusterConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("unknown field"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_section() {
        let toml = r#"
[nosuch]
key = "value"
"#;
        let err = ClusterConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("unknown field"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_flavour() {
        let toml = r#"
[ollama]
enabled = true
flavour = "metal"
"#;
        let err = ClusterConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("invalid ollama flavour"), "got: {err}");
    }

    #[test]
    fn rejects_empty_models() {
        let toml = r#"
[ollama]
enabled = true
models = []
"#;
        let err = ClusterConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("models list cannot be empty"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_llm_provider() {
        let toml = r#"
[global]
default_llm = "chatgpt"
"#;
        let err = ClusterConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("unknown variant"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_agent_provider() {
        let toml = r#"
[global]
default_agent = "chatgpt"
"#;
        let err = ClusterConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("unknown variant"), "got: {err}");
    }

    #[test]
    fn accepts_none_providers() {
        let toml = r#"
[global]
default_llm = "none"
default_agent = "none"
"#;
        ClusterConfig::from_toml(toml).unwrap();
    }

    #[test]
    fn accepts_legacy_provider_aliases() {
        let toml = r#"
[global]
llm_provider = "ollama"
agent_provider = "openclaw"
"#;
        let config = ClusterConfig::from_toml(toml).unwrap();
        assert_eq!(config.global.default_llm, LlmProvider::Ollama);
        assert_eq!(config.global.default_agent, AgentProvider::Openclaw);
    }

    #[test]
    fn rejects_wrong_type() {
        let toml = r#"
[ollama]
port = "not_a_number"
"#;
        let err = ClusterConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("invalid type"), "got: {err}");
    }

    #[test]
    fn accepts_all_valid_flavours() {
        for flavour in VALID_FLAVOURS {
            let toml = format!("[ollama]\nflavour = \"{flavour}\"");
            ClusterConfig::from_toml(&toml).unwrap();
        }
    }

    // ── Daemon settings tests ──────────────────────────────────────────

    #[test]
    fn daemon_config_defaults() {
        let config: DaemonConfig = toml::from_str("").unwrap();
        assert_eq!(config.daemon.update_interval, "1h");
        assert_eq!(config.daemon.health_interval, "1m");
        assert_eq!(config.daemon.log_level, "info");
    }

    #[test]
    fn daemon_config_custom_intervals() {
        let toml = r#"
[daemon]
update_interval = "30s"
health_interval = "5m"
log_level = "debug"
"#;
        let config = DaemonConfig::from_toml(toml).unwrap();
        assert_eq!(config.daemon.update_interval, "30s");
        assert_eq!(config.daemon.health_interval, "5m");
        assert_eq!(config.daemon.log_level, "debug");
    }

    #[test]
    fn daemon_config_invalid_interval() {
        let toml = r#"
[daemon]
update_interval = "bogus"
"#;
        let err = DaemonConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("invalid update_interval"), "got: {err}");
    }

    #[test]
    fn daemon_config_invalid_log_level() {
        let toml = r#"
[daemon]
log_level = "verbose"
"#;
        let err = DaemonConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("invalid log_level"), "got: {err}");
    }

    // ── Provider toggle tests ──────────────────────────────────────────

    #[test]
    fn providers_default_to_none() {
        let config = ClusterConfig::from_toml("").unwrap();
        assert_eq!(config.global.default_llm, LlmProvider::None);
        assert_eq!(config.global.default_agent, AgentProvider::None);
    }

    #[test]
    fn providers_can_be_set_to_none() {
        let toml = r#"
[global]
default_llm = "none"
default_agent = "none"
"#;
        let config = ClusterConfig::from_toml(toml).unwrap();
        assert_eq!(config.global.default_llm, LlmProvider::None);
        assert_eq!(config.global.default_agent, AgentProvider::None);
    }

    #[test]
    fn enabled_flags_default_to_false() {
        let config = ClusterConfig::from_toml("").unwrap();
        assert!(!config.ollama.enabled);
        assert!(!config.lms.enabled);
        assert!(!config.openclaw.enabled);
    }

    #[test]
    fn enabled_flags_can_be_enabled() {
        let toml = r#"
[ollama]
enabled = true

[lms]
enabled = true

[openclaw]
enabled = true
"#;
        let config = ClusterConfig::from_toml(toml).unwrap();
        assert!(config.ollama.enabled);
        assert!(config.lms.enabled);
        assert!(config.openclaw.enabled);
    }

    #[test]
    fn cloud_as_list() {
        let toml = r#"
[[cloud]]
provider = "anthropic"
api_key = "sk-ant-test"

[[cloud]]
provider = "openai"
api_key = "sk-test"
enabled = false
"#;
        let config = ClusterConfig::from_toml(toml).unwrap();
        assert_eq!(config.cloud.len(), 2);
        assert!(config.cloud[0].enabled);
        assert!(!config.cloud[1].enabled);
        assert_eq!(config.cloud[0].provider, CloudProvider::Anthropic);
        assert_eq!(config.cloud[1].provider, CloudProvider::Openai);
        // default_cloud returns first enabled
        let dc = config.default_cloud().unwrap();
        assert_eq!(dc.provider, CloudProvider::Anthropic);
    }

    #[test]
    fn cloud_single_object_compat() {
        let toml = r#"
[cloud]
provider = "anthropic"
api_key = "sk-ant-test"
"#;
        let config = ClusterConfig::from_toml(toml).unwrap();
        assert_eq!(config.cloud.len(), 1);
        assert_eq!(config.cloud[0].provider, CloudProvider::Anthropic);
    }

    // ── Notifications config tests ─────────────────────────────────────

    #[test]
    fn notifications_empty_by_default() {
        let config: DaemonConfig = toml::from_str("").unwrap();
        assert!(config.notifications.urls.is_empty());
        assert!(config.notifications.events.is_none());
    }

    #[test]
    fn notifications_with_urls_and_events() {
        let toml = r#"
[notifications]
urls = ["tgram://bot/chat", "slack://token/#channel"]
events = ["service_crashed", "upgrade_installed"]
"#;
        let config: DaemonConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.notifications.urls.len(), 2);
        assert_eq!(
            config.notifications.events.as_ref().unwrap(),
            &["service_crashed", "upgrade_installed"]
        );
    }

    #[test]
    fn notifications_urls_without_filter() {
        let toml = r#"
[notifications]
urls = ["ntfy://ntfy.sh/topic"]
"#;
        let config: DaemonConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.notifications.urls.len(), 1);
        assert!(config.notifications.events.is_none());
    }

    // ── Structured OpenClaw config tests ───────────────────────────────

    #[test]
    fn openclaw_gateway_config() {
        let toml = r#"
[openclaw.gateway]
port = 9090
host = "0.0.0.0"
"#;
        let config = ClusterConfig::from_toml(toml).unwrap();
        let gw = config.openclaw.gateway.unwrap();
        assert_eq!(gw.port, 9090);
        assert_eq!(gw.host, "0.0.0.0");
    }

    #[test]
    fn openclaw_skills_config() {
        let toml = r#"
[openclaw.skills]
auto_update = false
"#;
        let config = ClusterConfig::from_toml(toml).unwrap();
        let skills = config.openclaw.skills.unwrap();
        assert!(!skills.auto_update);
    }

    #[test]
    fn openclaw_telegram_config() {
        let toml = r#"
[openclaw.telegram]
bot_token = "123456:ABC-DEF"
allowed_chat_ids = [111, 222]
"#;
        let config = ClusterConfig::from_toml(toml).unwrap();
        let tg = config.openclaw.telegram.unwrap();
        assert_eq!(tg.bot_token.expose(), "123456:ABC-DEF");
        assert_eq!(tg.allowed_chat_ids, vec![111, 222]);
        assert!(tg.enabled); // default true
    }

    #[test]
    fn openclaw_telegram_minimal() {
        let toml = r#"
[openclaw.telegram]
bot_token = "tok"
"#;
        let config = ClusterConfig::from_toml(toml).unwrap();
        let tg = config.openclaw.telegram.unwrap();
        assert_eq!(tg.bot_token.expose(), "tok");
        assert!(tg.allowed_chat_ids.is_empty());
        assert!(tg.enabled);
    }

    #[test]
    fn openclaw_gateway_and_extra_config() {
        let toml = r#"
[openclaw.gateway]
port = 8080

[openclaw.extra_config]
some_key = "some_value"
"#;
        let config = ClusterConfig::from_toml(toml).unwrap();
        assert!(config.openclaw.gateway.is_some());
        assert!(config.openclaw.extra_config.is_some());
    }

    // ── Upgrade window tests ──────────────────────────────────────────

    #[test]
    fn parse_valid_window() {
        let (start, end) = parse_time_window("02:00-05:00").unwrap();
        assert_eq!(start, NaiveTime::from_hms_opt(2, 0, 0).unwrap());
        assert_eq!(end, NaiveTime::from_hms_opt(5, 0, 0).unwrap());
    }

    #[test]
    fn parse_midnight_crossing_window() {
        let (start, end) = parse_time_window("23:00-05:00").unwrap();
        assert_eq!(start, NaiveTime::from_hms_opt(23, 0, 0).unwrap());
        assert_eq!(end, NaiveTime::from_hms_opt(5, 0, 0).unwrap());
    }

    #[test]
    fn parse_invalid_window_format() {
        assert!(parse_time_window("invalid").is_err());
        assert!(parse_time_window("").is_err());
        assert!(parse_time_window("02:00").is_err());
        assert!(parse_time_window("25:00-05:00").is_err());
    }

    #[test]
    fn daemon_config_with_upgrade_window() {
        let toml = r#"
[daemon]
upgrade_window = "02:00-05:00"
"#;
        let config = DaemonConfig::from_toml(toml).unwrap();
        assert_eq!(config.daemon.upgrade_window.as_deref(), Some("02:00-05:00"));
    }

    #[test]
    fn daemon_config_invalid_upgrade_window() {
        let toml = r#"
[daemon]
upgrade_window = "bogus"
"#;
        let err = DaemonConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("invalid upgrade_window"), "got: {err}");
    }

    #[test]
    fn daemon_config_no_upgrade_window() {
        let config: DaemonConfig = toml::from_str("").unwrap();
        assert!(config.daemon.upgrade_window.is_none());
    }

    #[test]
    fn within_normal_window() {
        let start = NaiveTime::from_hms_opt(2, 0, 0).unwrap();
        let end = NaiveTime::from_hms_opt(5, 0, 0).unwrap();
        let at_3am = NaiveTime::from_hms_opt(3, 0, 0).unwrap();
        let at_6am = NaiveTime::from_hms_opt(6, 0, 0).unwrap();
        let at_1am = NaiveTime::from_hms_opt(1, 0, 0).unwrap();

        assert!(is_within_window_at(start, end, at_3am));
        assert!(!is_within_window_at(start, end, at_6am));
        assert!(!is_within_window_at(start, end, at_1am));
    }

    #[test]
    fn within_midnight_crossing_window() {
        let start = NaiveTime::from_hms_opt(23, 0, 0).unwrap();
        let end = NaiveTime::from_hms_opt(5, 0, 0).unwrap();
        let at_1am = NaiveTime::from_hms_opt(1, 0, 0).unwrap();
        let at_midnight = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
        let at_noon = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
        let at_23_30 = NaiveTime::from_hms_opt(23, 30, 0).unwrap();

        assert!(is_within_window_at(start, end, at_1am));
        assert!(is_within_window_at(start, end, at_midnight));
        assert!(is_within_window_at(start, end, at_23_30));
        assert!(!is_within_window_at(start, end, at_noon));
    }

    // ── Secret type tests ──────────────────────────────────────────────

    #[test]
    fn secret_debug_redacts() {
        let s = Secret::new("hunter2");
        assert_eq!(format!("{:?}", s), "[REDACTED]");
        assert_eq!(format!("{}", s), "[REDACTED]");
    }

    #[test]
    fn secret_serde_roundtrip() {
        let s = Secret::new("hunter2");
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"hunter2\"");
        let back: Secret = serde_json::from_str(&json).unwrap();
        assert_eq!(back.expose(), "hunter2");
    }

    #[test]
    fn secret_schema_has_extension() {
        let schema = schemars::schema_for!(Secret);
        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["x-secret"], serde_json::json!(true));
        assert_eq!(json["type"], serde_json::json!("string"));
    }

    #[test]
    fn secret_default_is_empty() {
        let s = Secret::default();
        assert!(s.is_empty());
        assert_eq!(s.expose(), "");
    }

    #[test]
    fn secret_option_serde() {
        #[derive(Serialize, Deserialize)]
        struct T {
            #[serde(default)]
            key: Option<Secret>,
        }
        let t: T = serde_json::from_str(r#"{"key":"abc"}"#).unwrap();
        assert_eq!(t.key.as_ref().unwrap().expose(), "abc");
        let t: T = serde_json::from_str(r#"{}"#).unwrap();
        assert!(t.key.is_none());
    }

    // ── Secret resolution tests ─────────────────────────────────────────

    #[test]
    fn secret_resolve_env() {
        let mut s = Secret::new("env:MY_KEY");
        let env = std::collections::HashMap::from([("MY_KEY".into(), "resolved-value".into())]);
        let vault = std::collections::HashMap::new();
        s.resolve(&env, &vault).unwrap();
        assert_eq!(s.expose(), "resolved-value");
    }

    #[test]
    fn secret_resolve_vault() {
        let mut s = Secret::new("secret:my-secret");
        let env = std::collections::HashMap::new();
        let vault =
            std::collections::HashMap::from([("my-secret".into(), "vault-value".into())]);
        s.resolve(&env, &vault).unwrap();
        assert_eq!(s.expose(), "vault-value");
    }

    #[test]
    fn secret_resolve_literal_unchanged() {
        let mut s = Secret::new("sk-ant-literal");
        let env = std::collections::HashMap::new();
        let vault = std::collections::HashMap::new();
        s.resolve(&env, &vault).unwrap();
        assert_eq!(s.expose(), "sk-ant-literal");
    }

    #[test]
    fn secret_resolve_missing_env_errors() {
        let mut s = Secret::new("env:MISSING");
        let env = std::collections::HashMap::new();
        let vault = std::collections::HashMap::new();
        let err = s.resolve(&env, &vault).unwrap_err();
        assert!(err.contains("MISSING"), "error should name the var: {err}");
    }

    #[test]
    fn secret_resolve_missing_vault_errors() {
        let mut s = Secret::new("secret:nope");
        let env = std::collections::HashMap::new();
        let vault = std::collections::HashMap::new();
        let err = s.resolve(&env, &vault).unwrap_err();
        assert!(err.contains("nope"), "error should name the secret: {err}");
    }

    #[test]
    fn cluster_config_resolve_secrets() {
        let toml = r#"
[[cloud]]
provider = "anthropic"
api_key = "env:ANT_KEY"

[openclaw.telegram]
bot_token = "secret:tg-token"
"#;
        let mut config = ClusterConfig::from_toml(toml).unwrap();
        let env = std::collections::HashMap::from([("ANT_KEY".into(), "sk-ant-xxx".into())]);
        let vault = std::collections::HashMap::from([("tg-token".into(), "123:ABC".into())]);
        config.resolve_secrets(&env, &vault).unwrap();
        assert_eq!(config.cloud[0].api_key.as_ref().unwrap().expose(), "sk-ant-xxx");
        assert_eq!(
            config.openclaw.telegram.as_ref().unwrap().bot_token.expose(),
            "123:ABC"
        );
    }

    // ── Schema description tests ───────────────────────────────────────

    #[test]
    fn schema_has_descriptions() {
        let schema = schemars::schema_for!(ClusterConfig);
        let json = serde_json::to_string(&schema).unwrap();
        // Spot-check that descriptions made it into the schema
        assert!(
            json.contains("Package flavour"),
            "schema missing flavour description"
        );
        assert!(
            json.contains("Models to pull"),
            "schema missing models description"
        );
        assert!(
            json.contains("Default LLM backend"),
            "schema missing default_llm description"
        );
    }
}

// ── Healer stream events (shared between server and web client) ────────

/// Event streamed from server to client during a healer session.
/// This is the canonical wire type — used directly by both the server
/// streaming functions and the WASM client renderer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HealerStreamEvent {
    /// Event kind: "session_created", "message", "running_tools", "pins",
    /// "staff_pings", "state", "done", "error"
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_tools: Option<Vec<HealerRunningTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pins: Option<Vec<HealerPin>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staff_pings: Option<Vec<HealerStaffPing>>,
    /// Ephemeral status message (e.g. "Waiting for daemon reconnect...")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    /// Human-readable reason for the current state (e.g. "manual_pause",
    /// "token_budget_exceeded"). Sent alongside "state" and "done" events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HealerRunningTool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    pub started_at: String,
    /// Validation status for this tool call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<HealerToolValidation>,
}

/// Validation verdict for a healer tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerToolValidation {
    /// "approved", "rejected", "skipped", "validating", "error"
    pub status: String,
    /// Validator reasoning (both for approvals and rejections).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    /// Risk level: "read_only", "session_local", "mutating", "destructive"
    pub risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerPin {
    pub slot: String,
    pub summary: String,
    #[serde(default)]
    pub affected_services: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerStaffPing {
    pub id: String,
    pub category: String,
    pub message: String,
    pub resolved: bool,
    pub created_at: String,
}
