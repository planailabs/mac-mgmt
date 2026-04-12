use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RelayConfig {
    /// Address to listen on for WebSocket + REST API (e.g., "0.0.0.0:8080")
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    /// Start of SSH port range for dynamic allocation
    #[serde(default = "default_ssh_port_min")]
    pub ssh_port_min: u16,

    /// End of SSH port range for dynamic allocation
    #[serde(default = "default_ssh_port_max")]
    pub ssh_port_max: u16,

    /// Server API URL for token validation (e.g., "http://localhost:7378")
    pub server_api_url: String,

    /// Maximum number of concurrent daemon connections (default: 1000)
    #[serde(default = "default_max_daemons")]
    pub max_daemons: usize,

    /// Wildcard proxy hostname (e.g. "relay.plan.ai" enables *.relay.plan.ai).
    /// When set, the relay serves browser proxy endpoints for TCP tunnels.
    pub proxy_hostname: Option<String>,

    /// Directory for persistent data (port reservations, etc.).
    /// Defaults to the current working directory.
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
}

fn default_data_dir() -> String {
    ".".to_string()
}

fn default_listen_addr() -> String {
    "0.0.0.0:8080".to_string()
}

fn default_ssh_port_min() -> u16 {
    30000
}

fn default_ssh_port_max() -> u16 {
    40000
}

fn default_max_daemons() -> usize {
    1000
}

pub fn load(path: &str) -> Result<RelayConfig> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config from {path}"))?;
    let config: RelayConfig = toml::from_str(&contents)
        .with_context(|| format!("failed to parse config from {path}"))?;
    Ok(config)
}
