use serde::Deserialize;
use std::sync::OnceLock;

pub use plan_ai_auth::AuthConfig;

static CONFIG: OnceLock<ServerConfig> = OnceLock::new();

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub database: DatabaseConfig,
    #[serde(default)]
    pub web: WebConfig,
    pub local_hosting: LocalHostingConfig,
    pub auth: Option<AuthConfig>,
    pub secrets: Option<SecretsConfig>,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Deserialize)]
pub struct LocalHostingConfig {
    /// Root directory for locally-hosted static webspace content. Each webspace
    /// is served from `{dir}/{webspace_id}/`. Required.
    pub dir: String,
}

/// The `[proxy]` section is shared with the `web-agency-proxy` binary, which
/// parses it with its own struct; fields only the proxy reads (bind
/// addresses, server_url) live in `web-agency/proxy/src/config.rs`.
#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub agency_domain: String,
    #[serde(default = "default_agency_upstream")]
    pub agency_upstream: String,
    pub acme_email: Option<String>,
    #[serde(default = "default_internal_token_path")]
    pub internal_token_path: String,
}

fn default_agency_upstream() -> String {
    "127.0.0.1:7380".to_string()
}
fn default_internal_token_path() -> String {
    "/var/lib/web-agency/internal.token".to_string()
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_web_port")]
    pub port: u16,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            port: default_web_port(),
        }
    }
}

fn default_web_port() -> u16 {
    7380
}

#[derive(Debug, Deserialize)]
pub struct SecretsConfig {
    pub encryption_key: String,
}

pub fn load() -> &'static ServerConfig {
    CONFIG.get_or_init(|| {
        let path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "./config.toml".to_string());
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read config from {path}: {e}"));
        toml::from_str(&content)
            .unwrap_or_else(|e| panic!("failed to parse config from {path}: {e}"))
    })
}

pub fn config() -> &'static ServerConfig {
    CONFIG.get().expect("config not loaded")
}
