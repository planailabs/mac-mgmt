use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RelayConfig {
    /// Address to listen on for WebSocket + REST API (e.g., "0.0.0.0:8080")
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    /// Start of SSH port range for per-daemon TCP listeners.
    #[serde(default = "default_ssh_port_min")]
    pub ssh_port_min: u16,

    /// End of SSH port range for per-daemon TCP listeners.
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

    /// Full external URL of the relay API (e.g. "https://relay.plan.ai" or
    /// "http://localhost:8080"). Sent to daemons and included in heartbeats
    /// so the server can call the relay's file-tunnel endpoints without extra
    /// config. Required for file tunnel support.
    pub proxy_url: Option<String>,

    /// Origins allowed to make cross-origin requests to the proxy (e.g. the
    /// management server's web UI). Each entry is a full origin URL
    /// (e.g. `"http://localhost:8080"`) matched exactly against the
    /// request's Origin header.
    #[serde(default = "default_cors_origins")]
    pub cors_origins: Vec<String>,

    /// Directory for persistent data (port reservations, etc.).
    /// Defaults to the current working directory.
    #[serde(default = "default_data_dir")]
    pub data_dir: String,

    /// QUIC listen port for libp2p p2p connections (default: 4001)
    #[serde(default = "default_p2p_port")]
    pub p2p_port: u16,

    /// Path to Ed25519 private key file for libp2p identity (PEM format).
    /// If not set, a new key is generated and stored at `{data_dir}/relay_ed25519_key`.
    pub p2p_key_file: Option<String>,

    /// Path to the relay's TLS server certificate (PEM).
    /// Required in release builds; in debug builds a self-signed cert is
    /// generated automatically when omitted.
    pub tls_cert_path: Option<String>,

    /// Path to the relay's TLS server private key (PEM).
    /// Required in release builds; in debug builds a self-signed key is
    /// generated automatically when omitted.
    pub tls_key_path: Option<String>,
}

fn default_cors_origins() -> Vec<String> {
    vec!["http://localhost:8080".to_string()]
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

fn default_p2p_port() -> u16 {
    4001
}

pub fn load(path: &str) -> Result<RelayConfig> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config from {path}"))?;
    let config: RelayConfig =
        toml::from_str(&contents).with_context(|| format!("failed to parse config from {path}"))?;
    Ok(config)
}
