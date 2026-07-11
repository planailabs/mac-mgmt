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
    #[serde(default = "default_http_addr")]
    pub http_addr: String,
    #[serde(default = "default_https_addr")]
    pub https_addr: String,
    #[serde(default = "default_internal_token_path")]
    pub internal_token_path: String,
    #[serde(default = "default_server_url")]
    pub server_url: String,
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
    /// Read the internal API token. The token file is provisioned by the
    /// web-agency server and may not exist yet when the proxy starts, so we
    /// wait for it to appear rather than crashing.
    pub fn internal_token(&self) -> String {
        let mut logged = false;
        loop {
            match std::fs::read_to_string(&self.internal_token_path) {
                Ok(s) if !s.trim().is_empty() => return s.trim().to_string(),
                Ok(_) => {
                    if !logged {
                        tracing::warn!(
                            path = %self.internal_token_path,
                            "internal token file is empty; waiting for it to be written"
                        );
                        logged = true;
                    }
                }
                Err(e) => {
                    if !logged {
                        tracing::warn!(
                            path = %self.internal_token_path,
                            "waiting for internal token file to appear: {e}"
                        );
                        logged = true;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
}

pub fn load() -> &'static ProxyConfig {
    CONFIG.get_or_init(|| {
        let path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "./config.toml".to_string());
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read config from {path}: {e}"));
        let file: ConfigFile = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("failed to parse config from {path}: {e}"));
        file.proxy
    })
}
