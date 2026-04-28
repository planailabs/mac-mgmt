use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub provider: String,
    pub model: String,
    pub prompt_sha256: String,
    pub prompt_length_chars: usize,
    pub response_length_chars: usize,
    pub cleaner_session_id: Option<String>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
}

// ── MCP tool parameter structs ──────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct CloudSendParams {
    /// The prompt text to send to the LLM.
    pub prompt: String,
    /// Optional system prompt.
    #[serde(default)]
    pub system: Option<String>,
    /// Model identifier (e.g. "anthropic/claude-sonnet-4-6", "openai/gpt-5.4").
    /// Defaults to the LiteLLM proxy's default.
    #[serde(default)]
    pub model: Option<String>,
    /// If set, automatically uses the redacted text from this cleaner session
    /// as the prompt, and rehydrates the response before returning.
    #[serde(default)]
    pub cleaner_session_id: Option<String>,
    /// Maximum tokens in the response (default: 4096).
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Temperature (default: 0.7).
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_max_tokens() -> u32 {
    4096
}

fn default_temperature() -> f32 {
    0.7
}

#[derive(Deserialize, JsonSchema)]
pub struct AuditLogParams {
    /// Maximum number of entries to return (default: 20).
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    20
}
