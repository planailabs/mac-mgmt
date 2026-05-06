use serde::Deserialize;
use std::sync::OnceLock;

static CONFIG: OnceLock<ProxyConfig> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
struct ConfigFile {
    proxy: ProxyConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub agency_domain: String,
    #[serde(default = "default_agency_upstream")]
    pub agency_upstream: String,
    #[serde(default = "default_http_addr")]
    pub http_addr: String,
    #[serde(default = "default_https_addr")]
    pub https_addr: String,
    #[serde(default = "default_internal_token_path")]
    pub internal_token_path: String,
    #[serde(default = "default_server_url")]
    pub server_url: String,
}

fn default_agency_upstream() -> String {
    "127.0.0.1:7380".to_string()
}
fn default_http_addr() -> String {
    "[::]:80".to_string()
}
fn default_https_addr() -> String {
    "[::]:443".to_string()
}
fn default_internal_token_path() -> String {
    "/var/lib/web-agency/internal.token".to_string()
}
fn default_server_url() -> String {
    "http://127.0.0.1:7380".to_string()
}

impl ProxyConfig {
    pub fn internal_token(&self) -> String {
        std::fs::read_to_string(&self.internal_token_path)
            .unwrap_or_else(|e| {
                panic!(
                    "failed to read internal token from {}: {e}",
                    self.internal_token_path
                )
            })
            .trim()
            .to_string()
    }
}

pub fn load() -> &'static ProxyConfig {
    CONFIG.get_or_init(|| {
        let path =
            std::env::var("CONFIG_PATH").unwrap_or_else(|_| "./config.toml".to_string());
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read config from {path}: {e}"));
        let file: ConfigFile = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("failed to parse config from {path}: {e}"));
        file.proxy
    })
}
