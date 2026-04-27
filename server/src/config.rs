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
    pub auth: Option<AuthConfig>,
    pub xzar: Option<XzarConfig>,
    pub anthropic: Option<AnthropicConfig>,
    #[serde(default)]
    pub healer: HealerConfig,
    #[serde(default)]
    pub sentry: SentryConfig,
    #[serde(default)]
    pub git: GitConfig,
    #[serde(default)]
    pub skill_centers: SkillCentersConfig,
}

#[derive(Debug, Deserialize)]
pub struct SkillCentersConfig {
    #[serde(default = "default_skill_center_refresh")]
    pub refresh_interval_secs: u64,
}

impl Default for SkillCentersConfig {
    fn default() -> Self {
        Self {
            refresh_interval_secs: default_skill_center_refresh(),
        }
    }
}

fn default_skill_center_refresh() -> u64 {
    60
}

pub use mac_mgmt_common::sentry_ext::SentryConfig;

fn default_git_state_dir() -> String {
    "./state".to_string()
}
fn default_mac_mgmt_git_url() -> String {
    "https://git.plan.ai/plan-ai/mac-mgmt.git".to_string()
}
fn default_nixpkgs_git_url() -> String {
    "https://git.plan.ai/plan-ai/nixpkgs.git".to_string()
}
fn default_git_fetch_interval() -> u64 {
    300
}

#[derive(Debug, Deserialize)]
pub struct GitConfig {
    #[serde(default = "default_git_state_dir")]
    pub state_dir: String,
    #[serde(default = "default_mac_mgmt_git_url")]
    pub mac_mgmt_url: String,
    #[serde(default = "default_nixpkgs_git_url")]
    pub nixpkgs_url: String,
    #[serde(default = "default_git_fetch_interval")]
    pub fetch_interval_secs: u64,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            state_dir: default_git_state_dir(),
            mac_mgmt_url: default_mac_mgmt_git_url(),
            nixpkgs_url: default_nixpkgs_git_url(),
            fetch_interval_secs: default_git_fetch_interval(),
        }
    }
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
        Self {
            port: default_api_port(),
            external_url: default_api_external_url(),
        }
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
        Self {
            port: default_web_port(),
        }
    }
}

fn default_web_port() -> u16 {
    7377
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthConfig {
    pub cookie_secret: String,
    /// External base URL of the web UI (e.g. "https://mgmt.example.com").
    /// Used to derive OIDC callback URLs (`{external_url}/auth/{slug}/callback`).
    #[serde(default = "default_auth_external_url")]
    pub external_url: String,
    /// Optional Redis URL for session cache. If absent, PostgreSQL is used.
    pub redis_url: Option<String>,
    /// Emails that are automatically granted admin on first login.
    #[serde(default)]
    pub admin_emails: Vec<String>,
    /// OIDC providers. Each gets its own auth routes and access control.
    pub providers: Vec<OidcProviderConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcProviderConfig {
    /// URL slug used in auth routes: /auth/{slug}, /auth/{slug}/callback
    pub slug: String,
    /// Human-readable name shown on the login page.
    pub name: String,
    /// OIDC issuer URL for auto-discovery (e.g. "https://accounts.google.com").
    /// If omitted, authorization_endpoint and token_endpoint must be set manually.
    pub issuer: Option<String>,
    pub client_id: String,
    pub client_secret: String,
    /// Allow any authenticated user from this provider. Mutually exclusive
    /// with `allowed_domains` / `allowed_emails`.
    #[serde(default)]
    pub allow_all: bool,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub allowed_emails: Vec<String>,
    /// OAuth scopes to request. Defaults to ["openid", "email", "profile"].
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
    /// Organization names to auto-add users to on login (with "read" role).
    #[serde(default)]
    pub auto_join_orgs: Vec<String>,
}

fn default_auth_external_url() -> String {
    "http://localhost:8080".to_string()
}

fn default_token_budget() -> u64 {
    200_000
}

#[derive(Debug, Deserialize, Default)]
pub struct HealerConfig {
    /// Ollama base URL. Defaults to http://localhost:11434.
    #[serde(default)]
    pub ollama_url: Option<String>,
    /// Ollama model for the healer agent. Defaults to a tool-use-capable model.
    #[serde(default)]
    pub ollama_model: Option<String>,
    /// Anthropic model override (default: claude-sonnet-4-6).
    /// The API key comes from the [anthropic] section.
    #[serde(default)]
    pub anthropic_model: Option<String>,
    /// OpenRouter API key.
    #[serde(default)]
    pub openrouter_api_key: Option<String>,
    /// OpenRouter model override (default: anthropic/claude-sonnet-4).
    #[serde(default)]
    pub openrouter_model: Option<String>,
    /// Generic OpenAI-compatible API key.
    #[serde(default)]
    pub openai_compat_api_key: Option<String>,
    /// Generic OpenAI-compatible base URL (e.g. "http://my-vllm:8000/v1").
    #[serde(default)]
    pub openai_compat_url: Option<String>,
    /// Default model for the OpenAI-compatible provider.
    #[serde(default)]
    pub openai_compat_model: Option<String>,
    /// Max input+output tokens per cloud session before auto-pause. 0 = unlimited.
    #[serde(default = "default_token_budget")]
    pub token_budget: u64,
    /// Context7 API key for documentation lookup MCP server.
    #[serde(default)]
    pub context7_api_key: Option<String>,
    /// Models available in the UI model picker. Each entry specifies a display
    /// name, provider ("ollama" or "anthropic"), and the model identifier.
    /// If empty, built-in defaults are used.
    #[serde(default)]
    pub models: Vec<HealerModelEntry>,

    /// Automatically trigger healer sessions when instances are unhealthy.
    #[serde(default)]
    pub auto_trigger: bool,
    /// Number of consecutive unhealthy heartbeats before auto-triggering.
    #[serde(default = "default_auto_trigger_threshold")]
    pub auto_trigger_threshold: u32,
    /// Provider for auto-triggered sessions. Defaults to "ollama".
    #[serde(default = "default_auto_trigger_provider")]
    pub auto_trigger_provider: String,
    /// Model for auto-triggered sessions. If empty, uses the provider's default.
    #[serde(default)]
    pub auto_trigger_model: Option<String>,

    /// Default fix-model provider for the remediation phase.
    /// Used when not overridden by per-cluster config or request body.
    #[serde(default)]
    pub fix_provider: Option<String>,
    /// Default fix-model for the remediation phase.
    #[serde(default)]
    pub fix_model: Option<String>,
}

fn default_auto_trigger_threshold() -> u32 {
    10
}

fn default_auto_trigger_provider() -> String {
    "ollama".to_string()
}

/// A model entry for the healer UI model picker.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct HealerModelEntry {
    /// Human-readable display name shown in the dropdown.
    pub name: String,
    /// Model identifier passed to the provider (e.g. "gemma4", "claude-sonnet-4-6").
    pub model: String,
    /// Provider: "ollama", "anthropic", or "openrouter".
    pub provider: String,
    /// Per-model token budget override. If set, overrides the global `token_budget`
    /// when this model is selected. 0 = unlimited.
    #[serde(default)]
    pub token_budget: Option<u64>,
}

/// Built-in default model list used when `[healer] models` is empty.
pub fn default_healer_models() -> Vec<HealerModelEntry> {
    vec![
        HealerModelEntry {
            name: "Gemma 4".into(),
            model: "gemma4".into(),
            provider: "ollama".into(),
            token_budget: None,
        },
        HealerModelEntry {
            name: "Qwen 3".into(),
            model: "qwen3".into(),
            provider: "ollama".into(),
            token_budget: None,
        },
        HealerModelEntry {
            name: "Llama 3.3".into(),
            model: "llama3.3".into(),
            provider: "ollama".into(),
            token_budget: None,
        },
        HealerModelEntry {
            name: "Devstral".into(),
            model: "devstral".into(),
            provider: "ollama".into(),
            token_budget: None,
        },
        HealerModelEntry {
            name: "Claude Sonnet 4.6".into(),
            model: "claude-sonnet-4-6".into(),
            provider: "anthropic".into(),
            token_budget: Some(200_000),
        },
        HealerModelEntry {
            name: "Claude Haiku 4.5".into(),
            model: "claude-haiku-4-5-20251001".into(),
            provider: "anthropic".into(),
            token_budget: Some(400_000),
        },
        HealerModelEntry {
            name: "Claude Sonnet 4".into(),
            model: "anthropic/claude-sonnet-4".into(),
            provider: "openrouter".into(),
            token_budget: Some(200_000),
        },
        HealerModelEntry {
            name: "GPT-4.1".into(),
            model: "openai/gpt-4.1".into(),
            provider: "openrouter".into(),
            token_budget: Some(200_000),
        },
        HealerModelEntry {
            name: "Gemini 2.5 Pro".into(),
            model: "google/gemini-2.5-pro-preview".into(),
            provider: "openrouter".into(),
            token_budget: Some(200_000),
        },
        HealerModelEntry {
            name: "Kimi K2.6".into(),
            model: "moonshotai/kimi-k2.6".into(),
            provider: "openrouter".into(),
            token_budget: Some(200_000),
        },
    ]
}

impl HealerModelEntry {
    /// Return the display name with an auto-appended provider suffix
    /// (e.g. "Gemma 4" becomes "Gemma 4 (Ollama)") unless it already
    /// contains the provider name (case-insensitive).
    pub fn display_name(&self) -> String {
        let lower = self.name.to_lowercase();
        let provider_lower = self.provider.to_lowercase();
        if lower.contains(&provider_lower) {
            self.name.clone()
        } else {
            let suffix = match self.provider.as_str() {
                "ollama" => "Ollama",
                "anthropic" => "Anthropic",
                "openrouter" => "OpenRouter",
                "openai_compat" => "plan.ai Hosted",
                other => other,
            };
            format!("{} ({})", self.name, suffix)
        }
    }
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
            && config.auth.is_none()
            && std::env::var("DEV_ONLY_NO_AUTH").as_deref() != Ok("1")
        {
            panic!(
                "[auth] section is required in release builds (set DEV_ONLY_NO_AUTH=1 to bypass)"
            );
        }

        config
    })
}

pub fn config() -> &'static ServerConfig {
    CONFIG
        .get()
        .expect("config not loaded — call config::load() first")
}
