use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt;

const VALID_FLAVOURS: &[&str] = &["cpu", "rocm", "cuda", "vulkan"];
const VALID_LLM_PROVIDERS: &[&str] = &["ollama", "nexa", "none"];
const VALID_AGENT_PROVIDERS: &[&str] = &["openclaw", "none"];
const VALID_LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

#[derive(Debug)]
pub struct ValidationError(String);

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ValidationError {}

// ── Daemon settings (daemon-only, not in CustomerConfig) ────────────────

fn default_update_interval() -> String {
    "1h".to_string()
}

fn default_health_interval() -> String {
    "1m".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonSettings {
    #[serde(default = "default_update_interval")]
    pub update_interval: String,
    #[serde(default = "default_health_interval")]
    pub health_interval: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub log_dir: Option<String>,
    #[serde(default)]
    pub upgrade_window: Option<String>,
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            update_interval: default_update_interval(),
            health_interval: default_health_interval(),
            log_level: default_log_level(),
            log_dir: None,
            upgrade_window: None,
        }
    }
}

// ── Notifications (daemon-only) ──────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub urls: Vec<String>,
    #[serde(default)]
    pub events: Option<Vec<String>>,
}

// ── Ollama ──────────────────────────────────────────────────────────────

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    11434
}

fn default_models() -> Vec<String> {
    vec![
        "phi4-mini".to_string(),
        "qwen3.5".to_string(),
        "Flux_AI/Flux_AI".to_string(),
    ]
}

fn default_model() -> String {
    "phi4-mini".to_string()
}

fn default_flavour() -> String {
    "cpu".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OllamaConfig {
    #[schemars(description = "Ollama listen address")]
    #[serde(default = "default_host")]
    pub host: String,
    #[schemars(description = "Ollama listen port")]
    #[serde(default = "default_port")]
    pub port: u16,
    #[schemars(description = "Models to pull on startup; at least one required")]
    #[serde(default = "default_models")]
    pub models: Vec<String>,
    #[schemars(description = "Default model for OpenClaw to use")]
    #[serde(default = "default_model")]
    pub default_model: String,
    #[schemars(description = "Package flavour: cpu, rocm (AMD), cuda (NVIDIA), or vulkan")]
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

// ── Nexa ───────────────────────────────────────────────────────────────

fn default_nexa_port() -> u16 {
    18181
}

fn default_nexa_models() -> Vec<String> {
    vec!["ggml-org/Qwen3-1.7B-GGUF".to_string()]
}

fn default_nexa_model() -> String {
    "ggml-org/Qwen3-1.7B-GGUF".to_string()
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NexaConfig {
    #[schemars(description = "Nexa listen address")]
    #[serde(default = "default_host")]
    pub host: String,
    #[schemars(description = "Nexa listen port")]
    #[serde(default = "default_nexa_port")]
    pub port: u16,
    #[schemars(description = "Models to pull on startup; at least one required")]
    #[serde(default = "default_nexa_models")]
    pub models: Vec<String>,
    #[schemars(description = "Default model for OpenClaw to use")]
    #[serde(default = "default_nexa_model")]
    pub default_model: String,
}

impl Default for NexaConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_nexa_port(),
            models: default_nexa_models(),
            default_model: default_nexa_model(),
        }
    }
}

impl NexaConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.models.is_empty() {
            return Err(ValidationError("nexa models list cannot be empty".into()));
        }
        Ok(())
    }
}

// ── OpenClaw ────────────────────────────────────────────────────────────

fn default_gateway_port() -> u16 {
    8080
}

fn default_gateway_host() -> String {
    "127.0.0.1".to_string()
}

#[derive(Debug, Clone, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenClawGatewayConfig {
    #[schemars(description = "OpenClaw gateway listen port")]
    #[serde(default = "default_gateway_port")]
    pub port: u16,
    #[schemars(description = "OpenClaw gateway listen address")]
    #[serde(default = "default_gateway_host")]
    pub host: String,
}

#[derive(Debug, Clone, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenClawSkillsConfig {
    #[schemars(description = "Automatically update skills on the update interval")]
    #[serde(default = "default_true")]
    pub auto_update: bool,
}

#[derive(Debug, Clone, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenClawTelegramConfig {
    #[schemars(description = "Telegram bot token from @BotFather")]
    pub bot_token: String,
    #[schemars(description = "Allowed Telegram chat IDs. If empty, all chats are allowed.")]
    #[serde(default)]
    pub allowed_chat_ids: Vec<i64>,
    #[schemars(description = "Enable the Telegram integration")]
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenClawConfig {
    #[schemars(description = "Gateway settings merged into ~/.openclaw/openclaw.json")]
    #[serde(default)]
    pub gateway: Option<OpenClawGatewayConfig>,
    #[schemars(description = "Skills auto-update behavior")]
    #[serde(default)]
    pub skills: Option<OpenClawSkillsConfig>,
    #[schemars(description = "Telegram bot integration")]
    #[serde(default)]
    pub telegram: Option<OpenClawTelegramConfig>,
    #[schemars(description = "Arbitrary key-value pairs merged into openclaw.json after typed fields")]
    #[serde(default)]
    pub extra_config: Option<serde_json::Value>,
}

impl Default for OpenClawConfig {
    fn default() -> Self {
        Self {
            gateway: None,
            skills: None,
            telegram: None,
            extra_config: None,
        }
    }
}

// ── Metrics ─────────────────────────────────────────────────────────────

fn default_metrics_port() -> u16 {
    9396
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MetricsConfig {
    #[schemars(description = "Prometheus metrics endpoint port")]
    #[serde(default = "default_metrics_port")]
    pub port: u16,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            port: default_metrics_port(),
        }
    }
}

// ── Server (daemon → server connection) ─────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DaemonServerConfig {
    pub url: Option<String>,
    pub token: Option<String>,
}

// ── Global ─────────────────────────────────────────────────────────────

fn default_llm_provider() -> String {
    "ollama".to_string()
}

fn default_agent_provider() -> String {
    "openclaw".to_string()
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GlobalConfig {
    #[schemars(description = "LLM backend to use: ollama, nexa, or none")]
    #[serde(default = "default_llm_provider")]
    pub llm_provider: String,
    #[schemars(description = "Agent provider to use: openclaw or none")]
    #[serde(default = "default_agent_provider")]
    pub agent_provider: String,
    #[schemars(description = "Display name for this agent")]
    #[serde(default)]
    pub agent_name: Option<String>,
    #[schemars(description = "Display name for the user")]
    #[serde(default)]
    pub user_name: Option<String>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            llm_provider: default_llm_provider(),
            agent_provider: default_agent_provider(),
            agent_name: None,
            user_name: None,
        }
    }
}

impl GlobalConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !VALID_LLM_PROVIDERS.contains(&self.llm_provider.as_str()) {
            return Err(ValidationError(format!(
                "invalid llm_provider '{}', must be one of: {}",
                self.llm_provider,
                VALID_LLM_PROVIDERS.join(", ")
            )));
        }
        if !VALID_AGENT_PROVIDERS.contains(&self.agent_provider.as_str()) {
            return Err(ValidationError(format!(
                "invalid agent_provider '{}', must be one of: {}",
                self.agent_provider,
                VALID_AGENT_PROVIDERS.join(", ")
            )));
        }
        Ok(())
    }
}

// ── Customer Config (what the server manages per-customer) ──────────────

#[derive(Debug, Clone, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CustomerConfig {
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default)]
    pub openclaw: OpenClawConfig,
    #[serde(default)]
    pub ollama: OllamaConfig,
    #[serde(default)]
    pub nexa: NexaConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
}

impl OllamaConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !VALID_FLAVOURS.contains(&self.flavour.as_str()) {
            return Err(ValidationError(format!(
                "invalid ollama flavour '{}', must be one of: {}",
                self.flavour,
                VALID_FLAVOURS.join(", ")
            )));
        }
        if self.models.is_empty() {
            return Err(ValidationError("ollama models list cannot be empty".into()));
        }
        Ok(())
    }
}

impl CustomerConfig {
    /// Parse and validate a TOML string as a customer config.
    pub fn from_toml(toml_str: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(toml_str).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.global.validate().map_err(|e| e.to_string())?;
        self.ollama.validate().map_err(|e| e.to_string())?;
        self.nexa.validate().map_err(|e| e.to_string())?;
        Ok(())
    }
}

// ── Relay ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RelayConfig {
    pub url: Option<String>,
}

// ── Daemon Config (full config including server section) ────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DaemonConfig {
    #[serde(default)]
    pub daemon: DaemonSettings,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default)]
    pub openclaw: OpenClawConfig,
    #[serde(default)]
    pub ollama: OllamaConfig,
    #[serde(default)]
    pub nexa: NexaConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub server: DaemonServerConfig,
    #[serde(default)]
    pub relay: RelayConfig,
}

impl DaemonSettings {
    pub fn validate(&self) -> Result<(), ValidationError> {
        humantime::parse_duration(&self.update_interval).map_err(|e| {
            ValidationError(format!(
                "invalid update_interval '{}': {e}",
                self.update_interval
            ))
        })?;
        humantime::parse_duration(&self.health_interval).map_err(|e| {
            ValidationError(format!(
                "invalid health_interval '{}': {e}",
                self.health_interval
            ))
        })?;
        if !VALID_LOG_LEVELS.contains(&self.log_level.as_str()) {
            return Err(ValidationError(format!(
                "invalid log_level '{}', must be one of: {}",
                self.log_level,
                VALID_LOG_LEVELS.join(", ")
            )));
        }
        if let Some(ref window) = self.upgrade_window {
            parse_time_window(window).map_err(|e| {
                ValidationError(format!("invalid upgrade_window '{}': {e}", window))
            })?;
        }
        Ok(())
    }
}

impl DaemonConfig {
    /// Parse and validate a TOML string as a daemon config.
    pub fn from_toml(toml_str: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(toml_str).map_err(|e| e.to_string())?;
        config.daemon.validate().map_err(|e| e.to_string())?;
        config.global.validate().map_err(|e| e.to_string())?;
        config.ollama.validate().map_err(|e| e.to_string())?;
        config.nexa.validate().map_err(|e| e.to_string())?;
        Ok(config)
    }
}

use chrono::NaiveTime;

pub fn parse_time_window(s: &str) -> Result<(NaiveTime, NaiveTime), String> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        return Err("expected format HH:MM-HH:MM".to_string());
    }
    let start = NaiveTime::parse_from_str(parts[0].trim(), "%H:%M")
        .map_err(|e| format!("invalid start time: {e}"))?;
    let end = NaiveTime::parse_from_str(parts[1].trim(), "%H:%M")
        .map_err(|e| format!("invalid end time: {e}"))?;
    Ok((start, end))
}

pub fn is_within_window_at(start: NaiveTime, end: NaiveTime, now: NaiveTime) -> bool {
    if start <= end {
        now >= start && now < end
    } else {
        now >= start || now < end
    }
}

pub fn is_within_window(start: NaiveTime, end: NaiveTime) -> bool {
    is_within_window_at(start, end, chrono::Local::now().time())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_minimal_config() {
        let config = CustomerConfig::from_toml("").unwrap();
        assert_eq!(config.ollama.flavour, "cpu");
        assert_eq!(config.global.llm_provider, "ollama");
        assert_eq!(config.global.agent_provider, "openclaw");
    }

    #[test]
    fn valid_full_config() {
        let toml = r#"
[global]
llm_provider = "ollama"

[ollama]
host = "10.0.0.1"
port = 11435
models = ["qwen3.5"]
default_model = "qwen3.5"
flavour = "rocm"

[metrics]
port = 9000
"#;
        let config = CustomerConfig::from_toml(toml).unwrap();
        assert_eq!(config.ollama.host, "10.0.0.1");
        assert_eq!(config.ollama.port, 11435);
        assert_eq!(config.ollama.flavour, "rocm");
        assert_eq!(config.metrics.port, 9000);
    }

    #[test]
    fn rejects_unknown_field() {
        let toml = r#"
[ollama]
bogus = true
"#;
        let err = CustomerConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("unknown field"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_section() {
        let toml = r#"
[nosuch]
key = "value"
"#;
        let err = CustomerConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("unknown field"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_flavour() {
        let toml = r#"
[ollama]
flavour = "metal"
"#;
        let err = CustomerConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("invalid ollama flavour"), "got: {err}");
    }

    #[test]
    fn rejects_empty_models() {
        let toml = r#"
[ollama]
models = []
"#;
        let err = CustomerConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("models list cannot be empty"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_llm_provider() {
        let toml = r#"
[global]
llm_provider = "chatgpt"
"#;
        let err = CustomerConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("invalid llm_provider"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_agent_provider() {
        let toml = r#"
[global]
agent_provider = "chatgpt"
"#;
        let err = CustomerConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("invalid agent_provider"), "got: {err}");
    }

    #[test]
    fn valid_nexa_config() {
        let toml = r#"
[nexa]
host = "10.0.0.1"
port = 18181
models = ["ggml-org/Qwen3-1.7B-GGUF"]
default_model = "ggml-org/Qwen3-1.7B-GGUF"
"#;
        let config = CustomerConfig::from_toml(toml).unwrap();
        assert_eq!(config.nexa.host, "10.0.0.1");
        assert_eq!(config.nexa.port, 18181);
    }

    #[test]
    fn rejects_empty_nexa_models() {
        let toml = r#"
[nexa]
models = []
"#;
        let err = CustomerConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("nexa models list cannot be empty"), "got: {err}");
    }

    #[test]
    fn accepts_nexa_llm_provider() {
        let toml = r#"
[global]
llm_provider = "nexa"
"#;
        CustomerConfig::from_toml(toml).unwrap();
    }

    #[test]
    fn accepts_none_providers() {
        let toml = r#"
[global]
llm_provider = "none"
agent_provider = "none"
"#;
        CustomerConfig::from_toml(toml).unwrap();
    }

    #[test]
    fn rejects_wrong_type() {
        let toml = r#"
[ollama]
port = "not_a_number"
"#;
        let err = CustomerConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("invalid type"), "got: {err}");
    }

    #[test]
    fn accepts_all_valid_flavours() {
        for flavour in VALID_FLAVOURS {
            let toml = format!("[ollama]\nflavour = \"{flavour}\"");
            CustomerConfig::from_toml(&toml).unwrap();
        }
    }

    // ── Daemon settings tests ──────────────────────────────────────────

    #[test]
    fn daemon_config_defaults() {
        let config: DaemonConfig = toml::from_str("").unwrap();
        assert_eq!(config.daemon.update_interval, "1h");
        assert_eq!(config.daemon.health_interval, "1m");
        assert_eq!(config.daemon.log_level, "info");
    }

    #[test]
    fn daemon_config_custom_intervals() {
        let toml = r#"
[daemon]
update_interval = "30s"
health_interval = "5m"
log_level = "debug"
"#;
        let config = DaemonConfig::from_toml(toml).unwrap();
        assert_eq!(config.daemon.update_interval, "30s");
        assert_eq!(config.daemon.health_interval, "5m");
        assert_eq!(config.daemon.log_level, "debug");
    }

    #[test]
    fn daemon_config_invalid_interval() {
        let toml = r#"
[daemon]
update_interval = "bogus"
"#;
        let err = DaemonConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("invalid update_interval"), "got: {err}");
    }

    #[test]
    fn daemon_config_invalid_log_level() {
        let toml = r#"
[daemon]
log_level = "verbose"
"#;
        let err = DaemonConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("invalid log_level"), "got: {err}");
    }

    // ── Provider toggle tests ──────────────────────────────────────────

    #[test]
    fn providers_default_to_ollama_and_openclaw() {
        let config = CustomerConfig::from_toml("").unwrap();
        assert_eq!(config.global.llm_provider, "ollama");
        assert_eq!(config.global.agent_provider, "openclaw");
    }

    #[test]
    fn providers_can_be_set_to_none() {
        let toml = r#"
[global]
llm_provider = "none"
agent_provider = "none"
"#;
        let config = CustomerConfig::from_toml(toml).unwrap();
        assert_eq!(config.global.llm_provider, "none");
        assert_eq!(config.global.agent_provider, "none");
    }

    // ── Notifications config tests ─────────────────────────────────────

    #[test]
    fn notifications_empty_by_default() {
        let config: DaemonConfig = toml::from_str("").unwrap();
        assert!(config.notifications.urls.is_empty());
        assert!(config.notifications.events.is_none());
    }

    #[test]
    fn notifications_with_urls_and_events() {
        let toml = r#"
[notifications]
urls = ["tgram://bot/chat", "slack://token/#channel"]
events = ["service_crashed", "upgrade_installed"]
"#;
        let config: DaemonConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.notifications.urls.len(), 2);
        assert_eq!(
            config.notifications.events.as_ref().unwrap(),
            &["service_crashed", "upgrade_installed"]
        );
    }

    #[test]
    fn notifications_urls_without_filter() {
        let toml = r#"
[notifications]
urls = ["ntfy://ntfy.sh/topic"]
"#;
        let config: DaemonConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.notifications.urls.len(), 1);
        assert!(config.notifications.events.is_none());
    }

    // ── Structured OpenClaw config tests ───────────────────────────────

    #[test]
    fn openclaw_gateway_config() {
        let toml = r#"
[openclaw.gateway]
port = 9090
host = "0.0.0.0"
"#;
        let config = CustomerConfig::from_toml(toml).unwrap();
        let gw = config.openclaw.gateway.unwrap();
        assert_eq!(gw.port, 9090);
        assert_eq!(gw.host, "0.0.0.0");
    }

    #[test]
    fn openclaw_skills_config() {
        let toml = r#"
[openclaw.skills]
auto_update = false
"#;
        let config = CustomerConfig::from_toml(toml).unwrap();
        let skills = config.openclaw.skills.unwrap();
        assert!(!skills.auto_update);
    }

    #[test]
    fn openclaw_telegram_config() {
        let toml = r#"
[openclaw.telegram]
bot_token = "123456:ABC-DEF"
allowed_chat_ids = [111, 222]
"#;
        let config = CustomerConfig::from_toml(toml).unwrap();
        let tg = config.openclaw.telegram.unwrap();
        assert_eq!(tg.bot_token, "123456:ABC-DEF");
        assert_eq!(tg.allowed_chat_ids, vec![111, 222]);
        assert!(tg.enabled); // default true
    }

    #[test]
    fn openclaw_telegram_minimal() {
        let toml = r#"
[openclaw.telegram]
bot_token = "tok"
"#;
        let config = CustomerConfig::from_toml(toml).unwrap();
        let tg = config.openclaw.telegram.unwrap();
        assert_eq!(tg.bot_token, "tok");
        assert!(tg.allowed_chat_ids.is_empty());
        assert!(tg.enabled);
    }

    #[test]
    fn openclaw_gateway_and_extra_config() {
        let toml = r#"
[openclaw.gateway]
port = 8080

[openclaw.extra_config]
some_key = "some_value"
"#;
        let config = CustomerConfig::from_toml(toml).unwrap();
        assert!(config.openclaw.gateway.is_some());
        assert!(config.openclaw.extra_config.is_some());
    }

    // ── Upgrade window tests ──────────────────────────────────────────

    #[test]
    fn parse_valid_window() {
        let (start, end) = parse_time_window("02:00-05:00").unwrap();
        assert_eq!(start, NaiveTime::from_hms_opt(2, 0, 0).unwrap());
        assert_eq!(end, NaiveTime::from_hms_opt(5, 0, 0).unwrap());
    }

    #[test]
    fn parse_midnight_crossing_window() {
        let (start, end) = parse_time_window("23:00-05:00").unwrap();
        assert_eq!(start, NaiveTime::from_hms_opt(23, 0, 0).unwrap());
        assert_eq!(end, NaiveTime::from_hms_opt(5, 0, 0).unwrap());
    }

    #[test]
    fn parse_invalid_window_format() {
        assert!(parse_time_window("invalid").is_err());
        assert!(parse_time_window("").is_err());
        assert!(parse_time_window("02:00").is_err());
        assert!(parse_time_window("25:00-05:00").is_err());
    }

    #[test]
    fn daemon_config_with_upgrade_window() {
        let toml = r#"
[daemon]
upgrade_window = "02:00-05:00"
"#;
        let config = DaemonConfig::from_toml(toml).unwrap();
        assert_eq!(config.daemon.upgrade_window.as_deref(), Some("02:00-05:00"));
    }

    #[test]
    fn daemon_config_invalid_upgrade_window() {
        let toml = r#"
[daemon]
upgrade_window = "bogus"
"#;
        let err = DaemonConfig::from_toml(toml).unwrap_err();
        assert!(err.contains("invalid upgrade_window"), "got: {err}");
    }

    #[test]
    fn daemon_config_no_upgrade_window() {
        let config: DaemonConfig = toml::from_str("").unwrap();
        assert!(config.daemon.upgrade_window.is_none());
    }

    #[test]
    fn within_normal_window() {
        let start = NaiveTime::from_hms_opt(2, 0, 0).unwrap();
        let end = NaiveTime::from_hms_opt(5, 0, 0).unwrap();
        let at_3am = NaiveTime::from_hms_opt(3, 0, 0).unwrap();
        let at_6am = NaiveTime::from_hms_opt(6, 0, 0).unwrap();
        let at_1am = NaiveTime::from_hms_opt(1, 0, 0).unwrap();

        assert!(is_within_window_at(start, end, at_3am));
        assert!(!is_within_window_at(start, end, at_6am));
        assert!(!is_within_window_at(start, end, at_1am));
    }

    #[test]
    fn within_midnight_crossing_window() {
        let start = NaiveTime::from_hms_opt(23, 0, 0).unwrap();
        let end = NaiveTime::from_hms_opt(5, 0, 0).unwrap();
        let at_1am = NaiveTime::from_hms_opt(1, 0, 0).unwrap();
        let at_midnight = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
        let at_noon = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
        let at_23_30 = NaiveTime::from_hms_opt(23, 30, 0).unwrap();

        assert!(is_within_window_at(start, end, at_1am));
        assert!(is_within_window_at(start, end, at_midnight));
        assert!(is_within_window_at(start, end, at_23_30));
        assert!(!is_within_window_at(start, end, at_noon));
    }

    // ── Schema description tests ───────────────────────────────────────

    #[test]
    fn schema_has_descriptions() {
        let schema = schemars::schema_for!(CustomerConfig);
        let json = serde_json::to_string(&schema).unwrap();
        // Spot-check that descriptions made it into the schema
        assert!(json.contains("Package flavour"), "schema missing flavour description");
        assert!(json.contains("Models to pull"), "schema missing models description");
        assert!(json.contains("LLM backend"), "schema missing llm_provider description");
    }
}
