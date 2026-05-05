use serde::Deserialize;
use std::sync::OnceLock;

static CONFIG: OnceLock<ServerConfig> = OnceLock::new();

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub database: DatabaseConfig,
    #[serde(default)]
    pub web: WebConfig,
    pub auth: Option<AuthConfig>,
    pub secrets: Option<SecretsConfig>,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthConfig {
    pub cookie_secret: String,
    #[serde(default = "default_auth_external_url")]
    pub external_url: String,
    pub redis_url: Option<String>,
    #[serde(default)]
    pub admin_emails: Vec<String>,
    pub providers: Vec<OidcProviderConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcProviderConfig {
    pub slug: String,
    pub name: String,
    pub issuer: Option<String>,
    pub client_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub allow_all: bool,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub allowed_emails: Vec<String>,
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
    #[serde(default)]
    pub auto_join_orgs: Vec<String>,
}

fn default_auth_external_url() -> String {
    "http://localhost:7380".to_string()
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
