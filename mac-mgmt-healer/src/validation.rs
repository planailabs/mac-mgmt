//! Tool-call validation layer for the healer agent.
//!
//! Wraps every `Box<dyn Tool>` in a [`ValidatedTool`] that intercepts `invoke()`
//! to run tiered checks before forwarding to the inner tool:
//!
//! - **Layer 0 — Static checks** (no LLM cost): duplicate detection, write-before-read,
//!   rapid-retry loop detection. Runs for all risk levels.
//! - **Layer 1 — LLM pre-flight** (opt-in): a second, cheap model validates whether the
//!   tool call makes sense given the agent's stated `_reason` and recent history.
//!   Runs only for `Mutating` and `Destructive` tools.

use std::borrow::Cow;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use swiftide::chat_completion::{Tool, ToolCall, ToolOutput, ToolSpec, errors::ToolError};
use swiftide::traits::AgentContext;

use crate::tools::ToolRisk;

// ── Configuration ────────────────────────────────────────────────────

/// Configuration for the validation layer.
#[derive(Clone)]
pub struct ValidationConfig {
    /// LLM backend for pre-flight validation. `None` = Layer 0 only.
    pub validator_llm: Option<Arc<dyn ValidatorLlm>>,
    /// Master switch. When false, `ValidatedTool` delegates directly.
    pub enabled: bool,
}

// ── Validator LLM trait ──────────────────────────────────────────────

/// Trait abstracting the validator LLM call.
#[async_trait]
pub trait ValidatorLlm: Send + Sync {
    /// Validate a tool call. Returns the verdict with reasoning.
    async fn validate_tool_call(
        &self,
        ctx: &ValidationContext<'_>,
    ) -> anyhow::Result<ValidationVerdict>;
}

/// Context passed to the validator LLM.
pub struct ValidationContext<'a> {
    pub tool_name: &'a str,
    pub tool_description: &'a str,
    pub args: &'a str,
    pub reason: &'a str,
    pub risk: ToolRisk,
    pub history: &'a [ToolCallRecord],
    /// Last assistant message (truncated to 500 chars).
    pub last_assistant_message: &'a str,
    /// Current session phase: "diagnosing", "remediating", "verifying".
    pub session_phase: &'a str,
}

/// Verdict from the validator LLM.
#[derive(Debug, Clone)]
pub enum ValidationVerdict {
    Approved { reasoning: String },
    Rejected { explanation: String },
}

// ── Tool call history ────────────────────────────────────────────────

/// A record of a single tool call, kept in session-scoped history.
#[derive(Debug, Clone)]
pub struct ToolCallRecord {
    pub tool_name: String,
    /// Full args JSON (not truncated).
    pub args: String,
    /// Full result for results <= MAX_RESULT_SIZE; capped with marker for larger.
    pub result: String,
    /// "ok" or "error"
    pub status: String,
    pub timestamp: DateTime<Utc>,
    /// Validation verdict, if LLM validation ran.
    pub validation: Option<ValidationResult>,
}

/// Stored validation result for a tool call.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationResult {
    pub approved: bool,
    pub reasoning: String,
}

/// Maximum result size stored verbatim in history. Larger results are truncated
/// with an explicit marker so the validator knows the content is incomplete.
const MAX_RESULT_SIZE: usize = 2048;

/// Maximum number of history entries to keep per session.
const MAX_HISTORY: usize = 20;

/// Shared, append-only history of tool calls within a session.
#[derive(Clone, Default)]
pub struct ToolCallHistory {
    inner: Arc<std::sync::Mutex<Vec<ToolCallRecord>>>,
}

impl ToolCallHistory {
    pub fn push(&self, record: ToolCallRecord) {
        let mut hist = self.inner.lock().unwrap();
        hist.push(record);
        let len = hist.len();
        if len > MAX_HISTORY {
            hist.drain(..len - MAX_HISTORY);
        }
    }

    pub fn recent(&self, n: usize) -> Vec<ToolCallRecord> {
        let hist = self.inner.lock().unwrap();
        hist.iter().rev().take(n).cloned().collect::<Vec<_>>().into_iter().rev().collect()
    }
}

// ── Static checks (Layer 0) ─────────────────────────────────────────

/// Fast, no-LLM validation. Returns `Some(reason)` to reject, `None` to pass.
fn static_check(
    tool_name: &str,
    args: &str,
    risk: ToolRisk,
    history: &[ToolCallRecord],
) -> Option<String> {
    // Duplicate: same tool + identical args within last 3 calls
    let recent_dupes = history
        .iter()
        .rev()
        .take(3)
        .filter(|e| e.tool_name == tool_name && e.args == args)
        .count();
    if recent_dupes >= 1 {
        return Some(format!(
            "Identical call to `{tool_name}` with the same arguments was made recently. \
             Check if you already have the result."
        ));
    }

    // Write-before-read: destructive tool without any prior read/get/list
    if risk == ToolRisk::Destructive && !history.is_empty() {
        let has_prior_read = history.iter().any(|r| {
            r.tool_name.starts_with("read_")
                || r.tool_name.starts_with("get_")
                || r.tool_name.starts_with("list_")
                || r.tool_name == "fetch_logs"
                || r.tool_name == "fetch_cluster_logs"
                || r.tool_name == "check_node_online"
        });
        if !has_prior_read {
            return Some(format!(
                "Destructive tool `{tool_name}` called without any prior read/get operation. \
                 Read the current state first."
            ));
        }
    }

    // Rapid retry: same tool >3 times in last 10 calls (looping)
    let rapid_count = history
        .iter()
        .rev()
        .take(10)
        .filter(|e| e.tool_name == tool_name)
        .count();
    if rapid_count > 3 {
        return Some(format!(
            "Tool `{tool_name}` has been called {rapid_count} times in the last 10 calls. \
             This looks like a loop — try a different approach."
        ));
    }

    None
}

// ── Schema injection ─────────────────────────────────────────────────

/// Inject a `_reason` string property into an existing schemars Schema.
fn inject_reason_field(schema: &mut schemars::Schema) {
    let value = serde_json::to_value(&*schema).unwrap();
    let serde_json::Value::Object(mut obj) = value else {
        return;
    };

    // Add to properties
    if let Some(props_val) = obj.get_mut("properties") {
        if let Some(props) = props_val.as_object_mut() {
            props.insert(
                "_reason".to_string(),
                serde_json::json!({
                    "type": "string",
                    "description": "Brief explanation of WHY you are making this tool call and what you expect to learn or change."
                }),
            );
        }
    }

    // Add to required array
    if let Some(req_val) = obj.get_mut("required") {
        if let Some(req) = req_val.as_array_mut() {
            req.push(serde_json::json!("_reason"));
        }
    }

    *schema = serde_json::from_value(serde_json::Value::Object(obj)).unwrap();
}

/// Create a schema containing only the `_reason` field (for no-params tools).
fn reason_only_schema() -> schemars::Schema {
    serde_json::from_value(serde_json::json!({
        "type": "object",
        "properties": {
            "_reason": {
                "type": "string",
                "description": "Brief explanation of WHY you are making this tool call and what you expect to learn or change."
            }
        },
        "required": ["_reason"]
    }))
    .unwrap()
}

/// Extract `_reason` from JSON args, returning (reason, cleaned_args).
/// The cleaned args have `_reason` stripped so the inner tool doesn't see it.
fn extract_reason(args: Option<&str>) -> (String, Option<String>) {
    let Some(args_str) = args else {
        return (String::new(), None);
    };
    let Ok(mut obj) = serde_json::from_str::<serde_json::Value>(args_str) else {
        return (String::new(), Some(args_str.to_string()));
    };
    let reason = obj
        .as_object_mut()
        .and_then(|m| m.remove("_reason"))
        .and_then(|v| match v {
            serde_json::Value::String(s) => Some(s),
            _ => None,
        })
        .unwrap_or_default();
    let clean = serde_json::to_string(&obj).ok();
    (reason, clean)
}

/// Cap a result string to MAX_RESULT_SIZE with an explicit truncation marker.
fn cap_result(result: &str) -> String {
    if result.len() <= MAX_RESULT_SIZE {
        result.to_string()
    } else {
        let first_kb = &result[..1024.min(result.len())];
        format!(
            "[truncated: {} bytes total] {}... [remaining {} bytes omitted]",
            result.len(),
            first_kb,
            result.len() - 1024
        )
    }
}

// ── ValidatedTool wrapper ────────────────────────────────────────────

/// A tool wrapper that validates calls before forwarding to the inner tool.
#[derive(Clone)]
pub struct ValidatedTool {
    inner: Box<dyn Tool>,
    risk: ToolRisk,
    config: ValidationConfig,
    history: ToolCallHistory,
    /// Cached ToolSpec with `_reason` injected.
    spec: ToolSpec,
}

impl ValidatedTool {
    /// Wrap a single tool with validation.
    pub fn wrap(
        tool: Box<dyn Tool>,
        risk: ToolRisk,
        config: ValidationConfig,
        history: ToolCallHistory,
    ) -> Box<dyn Tool> {
        let mut spec = tool.tool_spec();
        if spec.parameters_schema.is_some() {
            inject_reason_field(spec.parameters_schema.as_mut().unwrap());
        } else {
            spec.parameters_schema = Some(reason_only_schema());
        }
        Box::new(Self {
            inner: tool,
            risk,
            config,
            history,
            spec,
        })
    }

    /// Wrap a batch of tools with validation.
    pub fn wrap_all(
        tools: Vec<(Box<dyn Tool>, ToolRisk)>,
        config: ValidationConfig,
        history: ToolCallHistory,
    ) -> Vec<Box<dyn Tool>> {
        tools
            .into_iter()
            .map(|(tool, risk)| Self::wrap(tool, risk, config.clone(), history.clone()))
            .collect()
    }
}

#[async_trait]
impl Tool for ValidatedTool {
    fn name(&self) -> Cow<'_, str> {
        self.inner.name()
    }

    fn tool_spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    async fn invoke(
        &self,
        agent_context: &dyn AgentContext,
        tool_call: &ToolCall,
    ) -> Result<ToolOutput, ToolError> {
        if !self.config.enabled {
            return self.inner.invoke(agent_context, tool_call).await;
        }

        // 1. Extract _reason, build clean args for inner tool
        let (reason, clean_args) = extract_reason(tool_call.args());
        // Own the args string for use throughout the method
        let clean_args_owned = clean_args.unwrap_or_else(|| "{}".to_string());

        // 2. Layer 0: static checks
        let history_snapshot = self.history.recent(10);
        if let Some(rejection) = static_check(
            &self.inner.name(),
            &clean_args_owned,
            self.risk,
            &history_snapshot,
        ) {
            return Ok(ToolOutput::Fail(format!("[Validation rejected] {rejection}")));
        }

        // 3. Layer 1: LLM pre-flight (Mutating/Destructive only)
        let mut validation_result: Option<ValidationResult> = None;
        if self.risk >= ToolRisk::Mutating {
            if let Some(validator) = &self.config.validator_llm {
                let reason_str = if reason.is_empty() {
                    "(no reason given)"
                } else {
                    &reason
                };
                let tool_spec = self.inner.tool_spec();
                let tool_name = self.inner.name();
                let ctx = ValidationContext {
                    tool_name: &tool_name,
                    tool_description: &tool_spec.description,
                    args: &clean_args_owned,
                    reason: reason_str,
                    risk: self.risk,
                    history: &history_snapshot,
                    // TODO: wire last_assistant_message and session_phase from agent context
                    last_assistant_message: "",
                    session_phase: "",
                };

                match validator.validate_tool_call(&ctx).await {
                    Ok(ValidationVerdict::Rejected { explanation }) => {
                        validation_result = Some(ValidationResult {
                            approved: false,
                            reasoning: explanation.clone(),
                        });
                        // Record rejected call in history
                        self.history.push(ToolCallRecord {
                            tool_name: self.inner.name().to_string(),
                            args: clean_args_owned,
                            result: format!("[rejected] {explanation}"),
                            status: "rejected".to_string(),
                            timestamp: Utc::now(),
                            validation: validation_result.clone(),
                        });
                        return Ok(ToolOutput::Fail(format!(
                            "[Validator rejected] {explanation}"
                        )));
                    }
                    Ok(ValidationVerdict::Approved { reasoning }) => {
                        validation_result = Some(ValidationResult {
                            approved: true,
                            reasoning,
                        });
                    }
                    Err(e) => {
                        // Fail-open for Mutating, fail-closed for Destructive
                        if self.risk == ToolRisk::Destructive {
                            return Ok(ToolOutput::Fail(format!(
                                "[Validator error, blocking destructive call] {e}"
                            )));
                        }
                        tracing::warn!(
                            tool = %self.inner.name(),
                            err = %e,
                            "validator LLM error, proceeding (fail-open for mutating)"
                        );
                    }
                }
            }
        }

        // 4. Forward to inner tool with _reason stripped
        let mut modified_call = tool_call.clone();
        modified_call.with_args(Some(clean_args_owned.clone()));
        let result = self.inner.invoke(agent_context, &modified_call).await;

        // 5. Record in history
        let (status, result_text) = match &result {
            Ok(out) => ("ok".to_string(), out.content().unwrap_or("").to_string()),
            Err(e) => ("error".to_string(), e.to_string()),
        };
        self.history.push(ToolCallRecord {
            tool_name: self.inner.name().to_string(),
            args: clean_args_owned,
            result: cap_result(&result_text),
            status,
            timestamp: Utc::now(),
            validation: validation_result,
        });

        result
    }
}

// ── HttpValidatorLlm ─────────────────────────────────────────────────

/// OpenAI-compatible HTTP backend for the validator LLM.
#[derive(Clone)]
pub struct HttpValidatorLlm {
    client: reqwest::Client,
    url: String,
    model: String,
    api_key: Option<String>,
}

impl HttpValidatorLlm {
    pub fn new(url: String, model: String, api_key: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self {
            client,
            url,
            model,
            api_key,
        }
    }
}

#[async_trait]
impl ValidatorLlm for HttpValidatorLlm {
    async fn validate_tool_call(
        &self,
        ctx: &ValidationContext<'_>,
    ) -> anyhow::Result<ValidationVerdict> {
        // Build history summary
        let mut history_text = String::new();
        for (i, entry) in ctx.history.iter().enumerate() {
            history_text.push_str(&format!(
                "  {}. {}({}) -> {} ({})\n",
                i + 1,
                entry.tool_name,
                truncate_for_prompt(&entry.args, 200),
                entry.status,
                truncate_for_prompt(&entry.result, 300),
            ));
        }
        if history_text.is_empty() {
            history_text = "  (no prior tool calls)\n".to_string();
        }

        let risk_str = match ctx.risk {
            ToolRisk::ReadOnly => "read_only",
            ToolRisk::SessionLocal => "session_local",
            ToolRisk::Mutating => "mutating",
            ToolRisk::Destructive => "destructive",
        };

        let user_prompt = format!(
            r#"Tool: `{name}` ({risk})
Description: {desc}
Args: {args}
Agent's reason: "{reason}"
{phase_line}{assistant_line}
Recent tool history:
{history}
Does this tool call make sense given the agent's stated reason and history?
Reply with EXACTLY one line: "APPROVED: <brief reasoning>" or "REJECTED: <brief reasoning>"."#,
            name = ctx.tool_name,
            risk = risk_str,
            desc = ctx.tool_description,
            args = ctx.args,
            reason = ctx.reason,
            phase_line = if ctx.session_phase.is_empty() {
                String::new()
            } else {
                format!("Session phase: {}\n", ctx.session_phase)
            },
            assistant_line = if ctx.last_assistant_message.is_empty() {
                String::new()
            } else {
                format!("Agent's last message: \"{}\"\n", ctx.last_assistant_message)
            },
            history = history_text,
        );

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are a tool-call validator for an IT operations agent. \
                        Evaluate whether the proposed tool call is appropriate given the agent's \
                        stated intent and recent actions. Reply with exactly one line: \
                        \"APPROVED: <reasoning>\" or \"REJECTED: <reasoning>\". Be concise."
                },
                {
                    "role": "user",
                    "content": user_prompt
                }
            ],
            "max_tokens": 150,
            "temperature": 0.0,
        });

        let mut req = self.client.post(&self.url).json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }

        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("validator LLM returned {status}: {body}");
        }

        let json: serde_json::Value = resp.json().await?;
        let text = json
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        // Parse verdict
        if let Some(reasoning) = text.strip_prefix("APPROVED:") {
            Ok(ValidationVerdict::Approved {
                reasoning: reasoning.trim().to_string(),
            })
        } else if let Some(reasoning) = text.strip_prefix("REJECTED:") {
            Ok(ValidationVerdict::Rejected {
                explanation: reasoning.trim().to_string(),
            })
        } else {
            // Ambiguous → fail-open
            tracing::warn!("ambiguous validator response: {text}");
            Ok(ValidationVerdict::Approved {
                reasoning: format!("(ambiguous validator response, defaulting to approved) {text}"),
            })
        }
    }
}

fn truncate_for_prompt(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

// ── Builder helper ───────────────────────────────────────────────────

/// Build a validator LLM from connector config, if configured.
pub fn build_validator_llm(
    connector_config: &crate::connector::ConnectorConfig,
) -> Option<Arc<dyn ValidatorLlm>> {
    let provider = connector_config.validator_provider.as_deref()?;
    let model = connector_config.validator_model.as_deref()?;

    let (url, api_key) = match provider {
        "ollama" => {
            let base = connector_config
                .ollama_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            (format!("{base}/v1/chat/completions"), None)
        }
        "anthropic" => {
            // Anthropic doesn't use OpenAI-compatible endpoint natively,
            // but many proxies (litellm, etc.) expose one. For direct Anthropic
            // usage, we'd need a different client. For now, skip if no proxy.
            tracing::warn!("anthropic validator requires an OpenAI-compatible proxy; skipping");
            return None;
        }
        "openrouter" => {
            let key = connector_config.openrouter_api_key.clone();
            ("https://openrouter.ai/api/v1/chat/completions".to_string(), key)
        }
        "openai_compat" => {
            let base = connector_config.openai_compat_url.as_deref()?;
            let url = if base.ends_with("/chat/completions") {
                base.to_string()
            } else {
                format!("{}/chat/completions", base.trim_end_matches('/'))
            };
            (url, connector_config.openai_compat_api_key.clone())
        }
        _ => {
            tracing::warn!("unknown validator provider: {provider}");
            return None;
        }
    };

    Some(Arc::new(HttpValidatorLlm::new(
        url,
        model.to_string(),
        api_key,
    )))
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_reason_strips_field() {
        let args = r#"{"path": "/etc/config.toml", "_reason": "checking port setting"}"#;
        let (reason, clean) = extract_reason(Some(args));
        assert_eq!(reason, "checking port setting");
        let clean = clean.unwrap();
        assert!(!clean.contains("_reason"));
        assert!(clean.contains("path"));
    }

    #[test]
    fn test_extract_reason_missing() {
        let args = r#"{"path": "/etc/config.toml"}"#;
        let (reason, clean) = extract_reason(Some(args));
        assert_eq!(reason, "");
        // serde round-trip may normalize whitespace
        let clean = clean.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&clean).unwrap();
        assert_eq!(parsed, serde_json::json!({"path": "/etc/config.toml"}));
    }

    #[test]
    fn test_extract_reason_no_args() {
        let (reason, clean) = extract_reason(None);
        assert_eq!(reason, "");
        assert!(clean.is_none());
    }

    #[test]
    fn test_static_check_duplicate() {
        let history = vec![ToolCallRecord {
            tool_name: "read_file".to_string(),
            args: r#"{"path": "foo"}"#.to_string(),
            result: "ok".to_string(),
            status: "ok".to_string(),
            timestamp: Utc::now(),
            validation: None,
        }];
        let result = static_check("read_file", r#"{"path": "foo"}"#, ToolRisk::ReadOnly, &history);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Identical call"));
    }

    #[test]
    fn test_static_check_no_duplicate_different_args() {
        let history = vec![ToolCallRecord {
            tool_name: "read_file".to_string(),
            args: r#"{"path": "foo"}"#.to_string(),
            result: "ok".to_string(),
            status: "ok".to_string(),
            timestamp: Utc::now(),
            validation: None,
        }];
        let result = static_check("read_file", r#"{"path": "bar"}"#, ToolRisk::ReadOnly, &history);
        assert!(result.is_none());
    }

    #[test]
    fn test_static_check_write_before_read() {
        // History has only a pin (session-local), no read
        let history = vec![ToolCallRecord {
            tool_name: "pin".to_string(),
            args: "{}".to_string(),
            result: "ok".to_string(),
            status: "ok".to_string(),
            timestamp: Utc::now(),
            validation: None,
        }];
        let result = static_check("write_file", "{}", ToolRisk::Destructive, &history);
        assert!(result.is_some());
        assert!(result.unwrap().contains("without any prior read"));
    }

    #[test]
    fn test_static_check_write_after_read_ok() {
        let history = vec![ToolCallRecord {
            tool_name: "read_file".to_string(),
            args: "{}".to_string(),
            result: "content".to_string(),
            status: "ok".to_string(),
            timestamp: Utc::now(),
            validation: None,
        }];
        let result = static_check("write_file", "{}", ToolRisk::Destructive, &history);
        assert!(result.is_none());
    }

    #[test]
    fn test_static_check_rapid_retry() {
        let history: Vec<ToolCallRecord> = (0..4)
            .map(|i| ToolCallRecord {
                tool_name: "staff_ping".to_string(),
                args: format!(r#"{{"msg": "attempt_{i}"}}"#),
                result: "ok".to_string(),
                status: "ok".to_string(),
                timestamp: Utc::now(),
                validation: None,
            })
            .collect();
        let result = static_check(
            "staff_ping",
            r#"{"msg": "attempt_5"}"#,
            ToolRisk::Mutating,
            &history,
        );
        assert!(result.is_some());
        assert!(result.unwrap().contains("loop"));
    }

    #[test]
    fn test_cap_result_small() {
        let small = "hello world";
        assert_eq!(cap_result(small), small);
    }

    #[test]
    fn test_cap_result_large() {
        let large = "x".repeat(5000);
        let capped = cap_result(&large);
        assert!(capped.contains("[truncated: 5000 bytes total]"));
        assert!(capped.len() < large.len());
    }

    #[test]
    fn test_inject_reason_field() {
        use schemars::JsonSchema;

        #[derive(serde::Deserialize, JsonSchema)]
        #[allow(dead_code)]
        struct TestParams {
            path: String,
        }

        let schema = schemars::schema_for!(TestParams);
        let mut schema: schemars::Schema =
            serde_json::from_value(serde_json::to_value(&schema).unwrap()).unwrap();

        inject_reason_field(&mut schema);

        let val = serde_json::to_value(&schema).unwrap();
        let props = val.get("properties").unwrap();
        assert!(props.get("_reason").is_some());
        assert!(props.get("path").is_some());

        let required = val.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&serde_json::json!("_reason")));
        assert!(required.contains(&serde_json::json!("path")));
    }

    #[test]
    fn test_history_bounds() {
        let hist = ToolCallHistory::default();
        for i in 0..30 {
            hist.push(ToolCallRecord {
                tool_name: format!("tool_{i}"),
                args: "{}".to_string(),
                result: "ok".to_string(),
                status: "ok".to_string(),
                timestamp: Utc::now(),
                validation: None,
            });
        }
        let recent = hist.recent(100);
        assert_eq!(recent.len(), MAX_HISTORY);
        assert_eq!(recent.first().unwrap().tool_name, "tool_10");
        assert_eq!(recent.last().unwrap().tool_name, "tool_29");
    }
}
