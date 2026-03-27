use serde::Deserialize;

// ── Ollama ──────────────────────────────────────────────────────────────

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    11434
}

fn default_models() -> Vec<String> {
    vec![
        "qwen3.5".to_string(),
        "qwen3-coder-next".to_string(),
        "glm-5".to_string(),
        "kimi-k2.5".to_string(),
        "minimax-m2.7".to_string(),
    ]
}

fn default_model() -> String {
    "qwen3.5".to_string()
}

fn default_flavour() -> String {
    "cpu".to_string()
}

#[derive(Debug, Deserialize)]
pub struct OllamaConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_models")]
    pub models: Vec<String>,
    #[serde(default = "default_model")]
    pub default_model: String,
    #[serde(default = "default_flavour")]
    pub flavour: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            models: default_models(),
            default_model: default_model(),
            flavour: default_flavour(),
        }
    }
}

// ── OpenClaw ────────────────────────────────────────────────────────────

fn default_provider() -> String {
    "ollama".to_string()
}

#[derive(Debug, Deserialize)]
pub struct OpenClawConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub extra_config: Option<serde_json::Value>,
}

impl Default for OpenClawConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            extra_config: None,
        }
    }
}

// ── Metrics ─────────────────────────────────────────────────────────────

fn default_metrics_port() -> u16 {
    9396
}

#[derive(Debug, Deserialize, Default)]
pub struct MetricsConfig {
    #[serde(default = "default_metrics_port")]
    pub port: u16,
}

// ── Server (daemon → server connection) ─────────────────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct DaemonServerConfig {
    pub url: Option<String>,
    pub token: Option<String>,
}

// ── Customer Config (what the server manages per-customer) ──────────────

#[derive(Debug, Deserialize, Default)]
pub struct CustomerConfig {
    #[serde(default)]
    pub openclaw: OpenClawConfig,
    #[serde(default)]
    pub ollama: OllamaConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
}

impl CustomerConfig {
    /// Parse and validate a TOML string as a customer config.
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }
}

// ── Daemon Config (full config including server section) ────────────────

#[derive(Debug, Deserialize, Default)]
pub struct DaemonConfig {
    #[serde(default)]
    pub openclaw: OpenClawConfig,
    #[serde(default)]
    pub ollama: OllamaConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub server: DaemonServerConfig,
}
