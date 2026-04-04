use serde::Deserialize;
use std::sync::OnceLock;

static CONFIG: OnceLock<ServerConfig> = OnceLock::new();

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub database: DatabaseConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub web: WebConfig,
    pub oidc: Option<OidcConfig>,
    pub xzar: Option<XzarConfig>,
    pub anthropic: Option<AnthropicConfig>,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct XzarConfig {
    pub url: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    #[serde(default = "default_api_port")]
    pub port: u16,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self { port: default_api_port() }
    }
}

fn default_api_port() -> u16 {
    7378
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebConfig {
    #[serde(default = "default_web_port")]
    pub port: u16,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self { port: default_web_port() }
    }
}

fn default_web_port() -> u16 {
    7377
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub allowed_emails: Vec<String>,
    pub cookie_secret: String,
    /// Optional Redis URL for session cache. If absent, PostgreSQL is used.
    pub redis_url: Option<String>,
}

pub fn load() -> &'static ServerConfig {
    CONFIG.get_or_init(|| {
        let path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "./config.toml".to_string());
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read config from {path}: {e}"));
        let config: ServerConfig = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("failed to parse config from {path}: {e}"));

        #[cfg(feature = "webui")]
        if cfg!(not(debug_assertions)) && config.oidc.is_none() {
            panic!("[oidc] section is required in release builds");
        }

        config
    })
}

pub fn config() -> &'static ServerConfig {
    CONFIG.get().expect("config not loaded — call config::load() first")
}
