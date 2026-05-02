use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub mgmt: MgmtConfig,
    pub incus: IncusConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub fleet: FleetConfig,
    #[serde(default)]
    pub matrix: MatrixConfig,
    #[serde(default)]
    pub sentry: SentryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MgmtConfig {
    /// Admin-side URL used by the runner to hit the mgmt admin API.
    pub url: String,
    /// Admin bearer token.
    pub admin_token: String,
    /// Organization under which all fleet clusters are created.
    pub organization_id: Uuid,
    /// Public URL the spawned daemons inside Incus use to dial the server.
    /// If unset, the server's configured `api.external_url` is used
    /// (passed through by the cloud-init endpoint).
    #[serde(default)]
    pub public_url: Option<String>,
    /// Nix system identifier baked into the cloud-init download URL.
    #[serde(default = "default_system")]
    pub system: String,
    /// Optional explicit daemon version; when unset, server resolves from rollout/pinned.
    #[serde(default)]
    pub daemon_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncusConfig {
    /// Incus HTTPS endpoint, e.g. "https://incus.example:8443".
    pub url: String,
    /// Path to PEM-encoded client cert used to authenticate to Incus.
    pub client_cert: PathBuf,
    /// Path to PEM-encoded client key.
    pub client_key: PathBuf,
    /// Optional PEM CA bundle used to validate the Incus server.
    /// If absent, TLS verification is skipped (use only for self-signed test Incus hosts).
    #[serde(default)]
    pub server_ca: Option<PathBuf>,
    /// Target Incus project (default: "default").
    #[serde(default = "default_project")]
    pub project: String,
    /// Image alias or fingerprint to launch (e.g. "images:ubuntu/26.04/cloud").
    #[serde(default = "default_image")]
    pub image_alias: String,
    /// Image server for remote image lookups (e.g. "https://images.linuxcontainers.org").
    #[serde(default = "default_image_server")]
    pub image_server: String,
    /// Instance type: "container" or "virtual-machine".
    #[serde(default = "default_instance_type")]
    pub instance_type: String,
    /// Profiles to attach to instances.
    #[serde(default = "default_profiles")]
    pub profiles: Vec<String>,
    /// Prefix prepended to every instance name.
    #[serde(default = "default_name_prefix")]
    pub name_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Bind address for the runner's local HTTP API.
    #[serde(default = "default_api_bind")]
    pub bind: String,
    /// Bind port.
    #[serde(default = "default_api_port")]
    pub port: u16,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind: default_api_bind(),
            port: default_api_port(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetConfig {
    /// Reconcile loop interval — how often we re-check the fleet.
    #[serde(default = "default_reconcile")]
    pub reconcile_interval: String,
    /// Interval between random VM-level chaos operations (start / stop /
    /// reprovision) on a random running cell. Aliased by the older name
    /// `random_reprovision_interval` for backwards compat.
    #[serde(default = "default_vm_chaos", alias = "random_reprovision_interval")]
    pub vm_chaos_interval: String,
    /// Path to the runner's state file.
    #[serde(default = "default_state_path")]
    pub state_path: PathBuf,
    /// Health grace period — instances younger than this are not considered unhealthy.
    #[serde(default = "default_grace")]
    pub startup_grace: String,
    /// Heartbeat staleness threshold.
    #[serde(default = "default_heartbeat_stale")]
    pub heartbeat_stale_after: String,
    /// Maximum time a newly-launched instance has to produce its first
    /// heartbeat before the runner tears it down and retries.
    #[serde(default = "default_deploy_timeout")]
    pub deploy_timeout: String,
    /// Maximum number of cells that may be in the Launching stage
    /// concurrently. Cells in ConfigPushed are held back until the launch
    /// slot frees up (another cell reaches Running or times out).
    #[serde(default = "default_max_concurrent_launches")]
    pub max_concurrent_launches: usize,
    /// Base interval between random install/uninstall actions across
    /// running clusters (skills, bundles, mcp servers, mcp bundles).
    /// Actual delay is jittered ±50% around this value. Set to "0" or "off"
    /// to disable the churn entirely.
    #[serde(default = "default_chaos_interval")]
    pub chaos_interval: String,
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            reconcile_interval: default_reconcile(),
            vm_chaos_interval: default_vm_chaos(),
            state_path: default_state_path(),
            startup_grace: default_grace(),
            heartbeat_stale_after: default_heartbeat_stale(),
            deploy_timeout: default_deploy_timeout(),
            max_concurrent_launches: default_max_concurrent_launches(),
            chaos_interval: default_chaos_interval(),
        }
    }
}

pub use mac_mgmt_common::sentry_ext::SentryConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixConfig {
    /// Cloud provider API keys. Providers without a key here are skipped when generating the matrix.
    #[serde(default)]
    pub cloud_api_keys: std::collections::HashMap<String, String>,
    /// Agent providers to include. Default: ["openclaw", "none"].
    #[serde(default)]
    pub agents: Option<Vec<String>>,
    /// LLM providers to include. Default: ["ollama", "lms", "cloud"].
    #[serde(default)]
    pub llms: Option<Vec<String>>,
    /// Cloud sub-providers to include. Default: all with configured API keys.
    #[serde(default)]
    pub cloud_providers: Option<Vec<String>>,
    /// Ollama model to configure on every ollama-llm cell.
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    /// LM Studio model to configure on every lms-llm cell.
    #[serde(default = "default_lms_model")]
    pub lms_model: String,
    /// Cluster sizes to exercise. For each size, every (agent × llm) combo
    /// is emitted as an independent cell; size 1 uses the bare key while
    /// larger sizes append `-n{size}`. Default: [1, 2].
    #[serde(default = "default_cluster_sizes")]
    pub cluster_sizes: Vec<u32>,
}

/// Hand-rolled Default — the derived one would make `cluster_sizes`
/// an empty `Vec` (and `ollama_model`/`lms_model` empty strings) which
/// kicks in when the whole `[matrix]` section is absent from
/// config.toml: `RunnerConfig`'s `#[serde(default)] matrix: MatrixConfig`
/// calls `Default::default()` on the struct, bypassing every
/// per-field `#[serde(default = …)]`. Mirror the serde defaults here.
impl Default for MatrixConfig {
    fn default() -> Self {
        Self {
            cloud_api_keys: Default::default(),
            agents: None,
            llms: None,
            cloud_providers: None,
            ollama_model: default_ollama_model(),
            lms_model: default_lms_model(),
            cluster_sizes: default_cluster_sizes(),
        }
    }
}

fn default_system() -> String {
    "x86_64-linux".into()
}
fn default_project() -> String {
    "default".into()
}
fn default_image() -> String {
    "ubuntu/26.04/cloud".into()
}
fn default_image_server() -> String {
    "https://images.linuxcontainers.org".into()
}
fn default_instance_type() -> String {
    "container".into()
}
fn default_profiles() -> Vec<String> {
    vec!["default".into()]
}
fn default_name_prefix() -> String {
    "mmr-".into()
}
fn default_api_bind() -> String {
    "127.0.0.1".into()
}
fn default_api_port() -> u16 {
    9400
}
fn default_reconcile() -> String {
    "1m".into()
}
fn default_vm_chaos() -> String {
    "30m".into()
}
fn default_state_path() -> PathBuf {
    PathBuf::from("/var/lib/mac-mgmt-runner/state.json")
}
fn default_grace() -> String {
    "3m".into()
}
fn default_heartbeat_stale() -> String {
    "5m".into()
}
fn default_deploy_timeout() -> String {
    "15m".into()
}
fn default_max_concurrent_launches() -> usize {
    3
}
fn default_chaos_interval() -> String {
    "5m".into()
}
fn default_ollama_model() -> String {
    "smollm2:1.7b".into()
}
fn default_lms_model() -> String {
    "smollm2-1.7b-instruct".into()
}
fn default_cluster_sizes() -> Vec<u32> {
    vec![1, 2]
}

impl RunnerConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading runner config at {}", path.display()))?;
        let cfg: RunnerConfig = toml::from_str(&content)
            .with_context(|| format!("parsing runner config at {}", path.display()))?;
        Ok(cfg)
    }
}

pub fn parse_duration(s: &str) -> Result<std::time::Duration> {
    humantime_parse(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG_MIN: &str = r#"
[mgmt]
url = "http://example"
admin_token = "tkn"
organization_id = "00000000-0000-0000-0000-000000000000"

[incus]
url = "http://example:8443"
client_cert = "/tmp/c"
client_key = "/tmp/k"
"#;

    #[test]
    fn cluster_sizes_default_when_no_matrix_section() {
        let cfg: RunnerConfig = toml::from_str(CONFIG_MIN).expect("parse");
        assert_eq!(cfg.matrix.cluster_sizes, vec![1, 2]);
        assert_eq!(cfg.matrix.ollama_model, "smollm2:1.7b");
    }

    #[test]
    fn cluster_sizes_default_when_matrix_section_is_partial() {
        let mut t = CONFIG_MIN.to_string();
        t.push_str("\n[matrix]\nollama_model = \"smollm2:1.7b\"\n");
        let cfg: RunnerConfig = toml::from_str(&t).expect("parse");
        assert_eq!(cfg.matrix.cluster_sizes, vec![1, 2]);
    }

    #[test]
    fn cluster_sizes_respects_explicit_value() {
        let mut t = CONFIG_MIN.to_string();
        t.push_str("\n[matrix]\ncluster_sizes = [1]\n");
        let cfg: RunnerConfig = toml::from_str(&t).expect("parse");
        assert_eq!(cfg.matrix.cluster_sizes, vec![1]);
    }
}

fn humantime_parse(s: &str) -> Result<std::time::Duration> {
    let s = s.trim();
    let (num, unit) = s.split_at(
        s.find(|c: char| !c.is_ascii_digit())
            .with_context(|| format!("invalid duration: {s}"))?,
    );
    let n: u64 = num
        .parse()
        .with_context(|| format!("invalid duration number: {num}"))?;
    let d = match unit {
        "s" => std::time::Duration::from_secs(n),
        "m" => std::time::Duration::from_secs(n * 60),
        "h" => std::time::Duration::from_secs(n * 3600),
        "d" => std::time::Duration::from_secs(n * 86400),
        other => anyhow::bail!("unknown duration unit: {other}"),
    };
    Ok(d)
}
