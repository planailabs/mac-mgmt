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
    pub relay: Option<RelayProxyConfig>,
    #[serde(default)]
    pub sentry: SentryConfig,
}

#[derive(Debug, Deserialize)]
pub struct RelayProxyConfig {
    /// Relay HTTP API URL (e.g. "https://relay.plan.ai")
    pub url: String,
    /// Token with admin or setting kind for relay API calls.
    pub token: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SentryConfig {
    /// Sentry DSN. When unset, Sentry is disabled.
    #[serde(default)]
    pub dsn: Option<String>,
    /// Environment tag (e.g. "staging", "production").
    #[serde(default)]
    pub environment: Option<String>,
    /// Sample rate for traces [0.0, 1.0]. Default 0 (off).
    #[serde(default)]
    pub traces_sample_rate: f32,
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
    /// External base URL for the API (e.g. "https://mgmt.example.com:7378").
    /// Used by the web UI to build links to API endpoints such as binary
    /// downloads and swagger-ui.
    #[serde(default = "default_api_external_url")]
    pub external_url: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self { port: default_api_port(), external_url: default_api_external_url() }
    }
}

fn default_api_external_url() -> String {
    format!("http://localhost:{}", default_api_port())
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
    /// Emails that are automatically granted admin on first login.
    #[serde(default)]
    pub admin_emails: Vec<String>,
}

pub fn load() -> &'static ServerConfig {
    CONFIG.get_or_init(|| {
        let path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "./config.toml".to_string());
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read config from {path}: {e}"));
        let config: ServerConfig = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("failed to parse config from {path}: {e}"));

        #[cfg(feature = "webui")]
        if cfg!(not(debug_assertions))
            && config.oidc.is_none()
            && std::env::var("DEV_ONLY_NO_AUTH").as_deref() != Ok("1")
        {
            panic!("[oidc] section is required in release builds (set DEV_ONLY_NO_AUTH=1 to bypass)");
        }

        config
    })
}

pub fn config() -> &'static ServerConfig {
    CONFIG.get().expect("config not loaded — call config::load() first")
}
