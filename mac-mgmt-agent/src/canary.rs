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
        Openrouter | Together | Bedrock | Custom(_) => None,
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

/// Resolve the canary model for the Hermes functional probe based on the
/// active LLM provider. Hermes uses bare model names (no provider/ prefix).
pub fn hermes_canary(cfg: &DaemonConfig) -> String {
    match cfg.global.default_llm {
        LlmProvider::Ollama => OLLAMA_CANARY.to_string(),
        LlmProvider::Lms => LMS_CANARY.to_string(),
        LlmProvider::Cloud => cfg
            .cloud
            .iter()
            .find(|c| c.enabled)
            .map(|c| {
                if c.default_model.is_empty() {
                    c.provider.default_model().to_string()
                } else {
                    c.default_model.clone()
                }
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mac_mgmt_common::CloudConfig;

    fn cfg_with(llm: LlmProvider, cloud: Vec<CloudConfig>) -> DaemonConfig {
        let mut c = DaemonConfig::default();
        c.global.default_llm = llm;
        c.cloud = cloud;
        c
    }

    fn cloud(provider: &str, enabled: bool) -> CloudConfig {
        CloudConfig {
            provider: CloudProvider::from_name(provider),
            enabled,
            ..Default::default()
        }
    }

    #[test]
    fn cloud_canary_known_and_unknown() {
        assert_eq!(
            cloud_canary(&CloudProvider::from_name("anthropic")),
            Some("anthropic/claude-haiku-4-5")
        );
        assert_eq!(cloud_canary(&CloudProvider::from_name("openrouter")), None);
        assert_eq!(cloud_canary(&CloudProvider::from_name("moonshot")), None);
    }

    #[test]
    fn openclaw_canary_per_provider() {
        assert_eq!(
            openclaw_canary(&cfg_with(LlmProvider::Ollama, vec![])),
            "ollama/qwen3:0.6b"
        );
        assert_eq!(
            openclaw_canary(&cfg_with(LlmProvider::Lms, vec![])),
            format!("lms/{LMS_CANARY}")
        );
        assert_eq!(
            openclaw_canary(&cfg_with(
                LlmProvider::Cloud,
                vec![cloud("anthropic", true)]
            )),
            "anthropic/claude-haiku-4-5"
        );
        // Provider without a cheap tier → fallback.
        assert_eq!(
            openclaw_canary(&cfg_with(LlmProvider::Cloud, vec![cloud("moonshot", true)])),
            "openclaw/default"
        );
    }

    #[test]
    fn hermes_canary_uses_bare_model_names() {
        assert_eq!(
            hermes_canary(&cfg_with(LlmProvider::Ollama, vec![])),
            "qwen3:0.6b"
        );
        // Explicit default_model wins.
        let with_default = CloudConfig {
            default_model: "moonshot/kimi-k2.6".into(),
            ..cloud("moonshot", true)
        };
        assert_eq!(
            hermes_canary(&cfg_with(LlmProvider::Cloud, vec![with_default])),
            "moonshot/kimi-k2.6"
        );
        // Else the provider's default model.
        assert_eq!(
            hermes_canary(&cfg_with(
                LlmProvider::Cloud,
                vec![cloud("anthropic", true)]
            )),
            "anthropic/claude-sonnet-4-6"
        );
    }
}
