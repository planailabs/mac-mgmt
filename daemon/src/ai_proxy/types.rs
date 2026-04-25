use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Chat Completions Request ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub frequency_penalty: Option<f64>,
    #[serde(default)]
    pub presence_penalty: Option<f64>,
    #[serde(default)]
    pub stop: Option<StopCondition>,
    /// Pass through any extra fields the client sends.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StopCondition {
    Single(String),
    Multiple(Vec<String>),
}

// ── Chat Completions Response ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    #[serde(default)]
    pub message: Option<ChatMessage>,
    #[serde(default)]
    pub delta: Option<ChatMessage>,
    pub finish_reason: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── Streaming chunk ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── Models endpoint ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<Model>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

// ── Usage endpoint ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct UsageResponse {
    pub key_name: String,
    pub window: String,
    pub tokens_used: i64,
    pub tokens_remaining: i64,
    pub token_budget: i64,
    pub recent_events: Vec<UsageEventResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageEventResponse {
    pub ts: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub latency_ms: u64,
    pub backend: String,
}

// ── Error response ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorDetail {
    pub message: String,
    pub r#type: String,
    pub code: Option<String>,
}

impl ErrorResponse {
    pub fn new(message: impl Into<String>, error_type: &str, code: Option<&str>) -> Self {
        Self {
            error: ErrorDetail {
                message: message.into(),
                r#type: error_type.to_string(),
                code: code.map(String::from),
            },
        }
    }
}
