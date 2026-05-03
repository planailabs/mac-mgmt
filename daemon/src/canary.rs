//! Per-backend canary model identifiers.
//!
//! These are small, fast models used for functional probes and always
//! pulled/loaded during `post_start` so probes never fail due to a missing model.

use mac_mgmt_common::{CloudProvider, DaemonConfig, LlmProvider};

pub const OLLAMA_CANARY: &str = "qwen3:0.6b";
pub const LMS_CANARY: &str = "lmstudio-community/Qwen3-0.6B-GGUF";

/// Smallest/cheapest model per cloud provider for functional probes.
/// Returns `None` for providers without a known cheap tier (falls back to
/// `openclaw/default` at the call site).
pub fn cloud_canary(provider: &CloudProvider) -> Option<&'static str> {
    use CloudProvider::*;
    match provider {
        Anthropic => Some("anthropic/claude-haiku-4-5"),
        Openai => Some("openai/gpt-4.1-nano"),
        Google => Some("google/gemini-3-flash-preview"),
        Groq => Some("groq/llama-4-scout-17b-16e-instruct"),
        Xai => Some("xai/grok-3-mini"),
        Deepseek => Some("deepseek/deepseek-chat"),
        Mistral => Some("mistral/mistral-small-latest"),
        // No obvious cheapest tier — fall back to openclaw/default.
        Openrouter | Together | Bedrock => None,
    }
}

/// Resolve the canary model for the OpenClaw functional probe based on the
/// active LLM provider.
pub fn openclaw_canary(cfg: &DaemonConfig) -> String {
    match cfg.global.default_llm {
        LlmProvider::Ollama => format!("ollama/{OLLAMA_CANARY}"),
        LlmProvider::Lms => format!("lms/{LMS_CANARY}"),
        LlmProvider::Cloud => cfg
            .cloud
            .iter()
            .find(|c| c.enabled)
            .and_then(|c| cloud_canary(&c.provider))
            .unwrap_or("openclaw/default")
            .to_string(),
        _ => "openclaw/default".to_string(),
    }
}
