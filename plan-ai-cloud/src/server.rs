use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use sha2::Digest;

use crate::state::SharedState;
use crate::types::*;

#[derive(Clone)]
pub struct CloudServer {
    state: SharedState,
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
}

impl CloudServer {
    pub fn new(state: SharedState) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler]
impl ServerHandler for CloudServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Multi-AI cloud LLM server. Routes prompts through LiteLLM to any cloud \
                 provider (Anthropic, OpenAI, Google, etc.) with optional PII redaction \
                 integration via plan-ai-cleaner sessions. Use cloud_send to query a model."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tool_router]
impl CloudServer {
    #[tool(
        name = "cloud_send",
        description = "Send a prompt to a cloud LLM via LiteLLM proxy. Specify a model like 'anthropic/claude-sonnet-4-6' or 'openai/gpt-5.4'. If a cleaner_session_id is provided, the prompt is taken from the approved redacted document and the response is automatically rehydrated with original values. In strict mode, a cleaner_session_id is required."
    )]
    async fn send(&self, Parameters(params): Parameters<CloudSendParams>) -> String {
        // Strict mode check.
        if self.state.strict() && params.cleaner_session_id.is_none() {
            return "Error: strict mode is enabled — a cleaner_session_id is required. \
                    Use plan-ai-cleaner to scan and approve the document first."
                .to_string();
        }

        // Determine the actual prompt text.
        let prompt = if let Some(ref session_id) = params.cleaner_session_id {
            match crate::rehydrate::load_redacted_text(self.state.cleaner_dir(), session_id) {
                Ok(text) => text,
                Err(e) => return format!("Error loading cleaner session: {e}"),
            }
        } else {
            params.prompt.clone()
        };

        // Send to LiteLLM (or Ollama for local models).
        let (base_url, api_key) = if is_local_model(params.model.as_deref()) {
            (self.state.ollama_url(), None)
        } else {
            (
                self.state.litellm_url().to_string(),
                self.state.litellm_key().map(|s| s.to_string()),
            )
        };

        let result = crate::backend::chat_completion(
            &base_url,
            api_key.as_deref(),
            params.model.as_deref(),
            &prompt,
            params.system.as_deref(),
            params.max_tokens,
            params.temperature,
        )
        .await;

        let response = match result {
            Ok(r) => r,
            Err(e) => return format!("Error from LLM: {e}"),
        };

        // Log audit entry.
        let mut hasher = sha2::Sha256::new();
        hasher.update(prompt.as_bytes());
        let prompt_hash = format!("{:x}", hasher.finalize());

        let entry = AuditEntry {
            timestamp: chrono::Utc::now(),
            provider: base_url.clone(),
            model: params.model.clone().unwrap_or_default(),
            prompt_sha256: prompt_hash,
            prompt_length_chars: prompt.len(),
            response_length_chars: response.text.len(),
            cleaner_session_id: params.cleaner_session_id.clone(),
            tokens_in: response.tokens_in,
            tokens_out: response.tokens_out,
        };
        if let Err(e) = crate::audit::log_entry(self.state.audit_dir(), &entry) {
            tracing::warn!("failed to write audit log: {e}");
        }

        // Rehydrate if a cleaner session was used.
        if let Some(ref session_id) = params.cleaner_session_id {
            match crate::rehydrate::rehydrate(self.state.cleaner_dir(), session_id, &response.text)
            {
                Ok(rehydrated) => rehydrated,
                Err(e) => {
                    format!("{}\n\n[Warning: rehydration failed: {e}]", response.text)
                }
            }
        } else {
            response.text
        }
    }

    #[tool(
        name = "cloud_list_providers",
        description = "List available cloud LLM providers. Shows the LiteLLM proxy URL, Ollama fallback URL, and whether API keys are configured."
    )]
    async fn list_providers(&self) -> String {
        let mut out = String::from("Configured backends:\n\n");
        out.push_str(&format!("LiteLLM proxy: {}\n", self.state.litellm_url()));
        out.push_str(&format!(
            "  API key: {}\n",
            if self.state.litellm_key().is_some() {
                "configured"
            } else {
                "not set"
            }
        ));
        out.push_str(&format!("Ollama fallback: {}\n", self.state.ollama_url()));
        out.push_str(&format!("Strict mode: {}\n", self.state.strict()));
        out.push_str(
            "\nUse model prefixes like 'anthropic/...', 'openai/...', 'google/...' \
                      to route through LiteLLM. Use 'ollama/...' or local model names for \
                      direct Ollama access.",
        );
        out
    }

    #[tool(
        name = "cloud_audit_log",
        description = "View recent cloud LLM audit log entries. Shows what was sent to which provider."
    )]
    async fn audit_log(&self, Parameters(params): Parameters<AuditLogParams>) -> String {
        let entries = match crate::audit::read_recent(self.state.audit_dir(), params.limit) {
            Ok(e) => e,
            Err(e) => return format!("Error reading audit log: {e}"),
        };

        if entries.is_empty() {
            return "No audit log entries found.".to_string();
        }

        let mut out = format!("{} entries:\n\n", entries.len());
        out.push_str("| Time | Model | Prompt chars | Response chars | Session |\n");
        out.push_str("|------|-------|-------------|----------------|----------|\n");
        for e in &entries {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                e.timestamp.format("%H:%M:%S"),
                if e.model.is_empty() {
                    "(default)"
                } else {
                    &e.model
                },
                e.prompt_length_chars,
                e.response_length_chars,
                e.cleaner_session_id.as_deref().unwrap_or("-"),
            ));
        }
        out
    }
}

/// Check if a model name refers to a local Ollama model.
fn is_local_model(model: Option<&str>) -> bool {
    match model {
        Some(m) => m.starts_with("ollama/") || m.starts_with("local/"),
        None => false,
    }
}
