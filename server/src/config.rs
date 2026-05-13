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
    #[serde(default)]
    pub secrets: Option<SecretsConfig>,
    #[serde(default = "default_importer")]
    pub importer: Option<ImporterConfig>,
    /// Runtime server mode: controls which API route groups are mounted.
    /// Set via `MAC_MGMT_SERVER_MODE` env var or config file.
    /// Values: "mgmt", "skill-center", "skill-importer", or empty/absent for monolith.
    #[serde(default)]
    pub mode: ServerMode,
}

/// Runtime server mode — controls which API route modules are active.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServerMode {
    /// All modules enabled (default).
    #[default]
    Monolith,
    /// Management server only (fleet, rollouts, healer).
    Mgmt,
    /// Skill center only (catalog, federation, packages).
    SkillCenter,
    /// Skill center + importer.
    SkillImporter,
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

fn default_gitlab_url() -> String {
    "https://git.plan.ai".to_string()
}
fn default_nixpkgs_project() -> String {
    "plan-ai/nixpkgs".to_string()
}
fn default_nixpkgs_branch() -> String {
    "plan-ai".to_string()
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
    /// GitLab instance URL for API calls (e.g. resolving latest CI pipeline).
    #[serde(default = "default_gitlab_url")]
    pub gitlab_url: String,
    /// GitLab project path for nixpkgs (used for CI pipeline lookups).
    #[serde(default = "default_nixpkgs_project")]
    pub nixpkgs_project: String,
    /// Branch to resolve when no explicit nixpkgs pin is set.
    #[serde(default = "default_nixpkgs_branch")]
    pub nixpkgs_branch: String,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            state_dir: default_git_state_dir(),
            mac_mgmt_url: default_mac_mgmt_git_url(),
            nixpkgs_url: default_nixpkgs_git_url(),
            fetch_interval_secs: default_git_fetch_interval(),
            gitlab_url: default_gitlab_url(),
            nixpkgs_project: default_nixpkgs_project(),
            nixpkgs_branch: default_nixpkgs_branch(),
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
    /// Nix public signing key for this cache (e.g. "xzar.plan.ai:BASE64KEY=").
    /// Exposed to daemons via GET /api/nix-caches so they can trust store paths.
    pub public_key: String,
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

pub use plan_ai_auth::AuthConfig;

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

    /// Models available in the validator model picker.
    /// If empty, built-in defaults (cheap/fast models) are used.
    #[serde(default)]
    pub validator_models: Vec<HealerModelEntry>,

    /// Fine-tuned Ollama model name (e.g. "mac-mgmt-healer").
    /// When set and present in Ollama, preferred over `ollama_model`.
    #[serde(default)]
    pub fine_tuned_model: Option<String>,
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

/// Built-in default validator model list. Cheap/fast models suitable for
/// single-shot tool-call validation. Used when `[healer] validator_models` is empty.
pub fn default_validator_models() -> Vec<HealerModelEntry> {
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
            name: "Claude Haiku 4.5".into(),
            model: "claude-haiku-4-5-20251001".into(),
            provider: "anthropic".into(),
            token_budget: None,
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

#[derive(Debug, Deserialize)]
pub struct SecretsConfig {
    /// Base64-encoded 32-byte AES-256 key for encrypting secrets at rest.
    /// Generate with: `openssl rand -base64 32`
    pub encryption_key: String,
}

#[derive(Debug, Deserialize)]
pub struct ImporterConfig {
    /// Directory for temporary git clones and builds.
    #[serde(default = "default_importer_work_dir")]
    pub work_dir: String,
    /// ClawHub registry URL.
    #[serde(default = "default_clawhub_url")]
    pub clawhub_url: String,
    /// Interval in seconds for auto-sync of import sources.
    #[serde(default = "default_importer_sync_interval")]
    pub sync_interval_secs: u64,
}

fn default_importer() -> Option<ImporterConfig> {
    Some(ImporterConfig {
        work_dir: default_importer_work_dir(),
        clawhub_url: default_clawhub_url(),
        sync_interval_secs: default_importer_sync_interval(),
    })
}

fn default_clawhub_url() -> String {
    "https://wry-manatee-359.convex.site".to_string()
}

fn default_importer_work_dir() -> String {
    "/tmp/mac-mgmt-import".to_string()
}

fn default_importer_sync_interval() -> u64 {
    3600
}

pub fn load() -> &'static ServerConfig {
    CONFIG.get_or_init(|| {
        let path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "./config.toml".to_string());
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read config from {path}: {e}"));
        let mut config: ServerConfig = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("failed to parse config from {path}: {e}"));

        // Env var override for server mode (used by nix wrapper scripts).
        if let Ok(mode) = std::env::var("MAC_MGMT_SERVER_MODE") {
            config.mode = match mode.as_str() {
                "mgmt" => ServerMode::Mgmt,
                "skill-center" => ServerMode::SkillCenter,
                "skill-importer" => ServerMode::SkillImporter,
                "" | "monolith" => ServerMode::Monolith,
                other => panic!("unknown MAC_MGMT_SERVER_MODE: {other}"),
            };
        }

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
