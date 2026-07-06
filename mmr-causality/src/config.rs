//! mmrc CLI config (`~/.config/mmrc/config.toml`). Environment selected by
//! `MMRC_ENV` (falling back to `default_env`), overridable per-invocation.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_env")]
    pub default_env: String,
    #[serde(default)]
    pub env: EnvsConfig,
    #[serde(default)]
    pub emulator: EmulatorConfig,
}

fn default_env() -> String {
    "antithesis".into()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EnvsConfig {
    pub prod: Option<ProdConfig>,
    pub antithesis: Option<AntithesisConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProdConfig {
    pub server_url: String,
    pub admin_token: String,
    pub relay_url: String,
    pub organization_id: Uuid,
    #[serde(default = "default_cluster_name")]
    pub cluster_name: String,
    #[serde(default = "default_ec_secs")]
    pub ec_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AntithesisConfig {
    pub mmrcd_url: String,
    pub mmrcd_token: String,
    #[serde(default = "default_admin_token")]
    pub admin_token: String,
    #[serde(default = "default_proxy_hostname")]
    pub proxy_hostname: String,
    #[serde(default = "default_cluster_name")]
    pub cluster_name: String,
    #[serde(default = "default_org")]
    pub organization_id: Uuid,
    #[serde(default = "default_ec_secs_short")]
    pub ec_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmulatorConfig {
    #[serde(default = "default_rounds")]
    pub rounds: usize,
    #[serde(default = "default_wpr")]
    pub workloads_per_round: usize,
    #[serde(default = "default_nodes")]
    pub nodes: usize,
    #[serde(default = "default_run_dir")]
    pub run_dir: PathBuf,
}

impl Default for EmulatorConfig {
    fn default() -> Self {
        Self {
            rounds: default_rounds(),
            workloads_per_round: default_wpr(),
            nodes: default_nodes(),
            run_dir: default_run_dir(),
        }
    }
}

fn default_cluster_name() -> String {
    "mmrc".into()
}
fn default_admin_token() -> String {
    "admin".into()
}
fn default_proxy_hostname() -> String {
    "test-mac-mgmt-relay".into()
}
fn default_org() -> Uuid {
    Uuid::nil()
}
fn default_ec_secs() -> u64 {
    600
}
fn default_ec_secs_short() -> u64 {
    120
}
fn default_rounds() -> usize {
    1
}
fn default_wpr() -> usize {
    6
}
fn default_nodes() -> usize {
    2
}
fn default_run_dir() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("mmrc/runs")
}

/// Default config path: `~/.config/mmrc/config.toml`.
pub fn default_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("mmrc/config.toml")
}

impl Config {
    pub fn load(path: Option<&std::path::Path>) -> Result<Self> {
        let path = path.map(PathBuf::from).unwrap_or_else(default_path);
        if !path.exists() {
            // No config file: usable defaults for the antithesis env against a
            // local mmrcd, still overridable by env vars in the callers.
            return Ok(Config {
                default_env: default_env(),
                env: EnvsConfig::default(),
                emulator: EmulatorConfig::default(),
            });
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    /// Resolve the active environment name: `--env` > `MMRC_ENV` > default.
    pub fn resolve_env_name(&self, flag: Option<&str>) -> String {
        flag.map(String::from)
            .or_else(|| std::env::var("MMRC_ENV").ok())
            .unwrap_or_else(|| self.default_env.clone())
    }
}
