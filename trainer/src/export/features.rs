use serde::{Deserialize, Serialize};

use super::ExportedSession;
use super::tokenizer::{self, Vocabulary};

/// Maximum sequence length for model input.
pub const MAX_SEQ_LEN: usize = 512;
/// Number of recent tool calls to include as history features.
pub const TOOL_HISTORY_LEN: usize = 5;

/// A single training sample for the tool selector model.
/// Each sample represents one tool call decision point in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSelectorSample {
    /// Token IDs of the context (messages up to this point).
    pub context_tokens: Vec<u32>,
    /// Role IDs for each context token position.
    pub context_roles: Vec<u32>,
    /// Recent tool call history (tool class indices, padded with NUM_TOOLS).
    pub tool_history: Vec<usize>,
    /// Current session phase as a token ID.
    pub phase: u32,
    /// Target: index into TOOL_NAMES for the next tool called.
    pub target_tool: usize,
    /// Session outcome (for optional weighting: successful sessions matter more).
    pub session_outcome: String,
}

/// A single training sample for the outcome predictor model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeSample {
    /// Token IDs of the first N messages.
    pub tokens: Vec<u32>,
    /// Role IDs for each token position.
    pub roles: Vec<u32>,
    /// Target: 0 = done/completed, 1 = failed, 2 = needs_human_attention/cancelled/paused.
    pub target: usize,
}

/// Extract tool selector training samples from a session.
///
/// Produces one sample per tool_result message: the context is everything
/// before that tool call, and the label is which tool was called.
pub fn extract_tool_selector_samples(
    session: &ExportedSession,
    vocab: &Vocabulary,
) -> Vec<ToolSelectorSample> {
    let mut samples = Vec::new();
    let mut context_tokens = Vec::new();
    let mut context_roles = Vec::new();
    let mut tool_history: Vec<usize> = Vec::new();
    let mut current_phase = tokenizer::PHASE_CREATED;

    for msg in &session.messages {
        // Check if this is a tool_result — that means the previous assistant call
        // selected this tool, so we create a sample for predicting it.
        if msg.role == "tool_result" {
            if let Some(tool_name) = msg
                .metadata
                .as_ref()
                .and_then(|m| m.get("tool_name"))
                .and_then(|v| v.as_str())
            {
                if let Some(tool_class) = Vocabulary::tool_class(tool_name) {
                    // Build the sample from context accumulated so far
                    let ctx_len = context_tokens.len().min(MAX_SEQ_LEN);
                    let ctx_start = context_tokens.len().saturating_sub(MAX_SEQ_LEN);

                    let mut history = vec![tokenizer::NUM_TOOLS; TOOL_HISTORY_LEN];
                    let hist_start = tool_history.len().saturating_sub(TOOL_HISTORY_LEN);
                    for (i, &h) in tool_history[hist_start..].iter().enumerate() {
                        history[i] = h;
                    }

                    samples.push(ToolSelectorSample {
                        context_tokens: context_tokens[ctx_start..].to_vec(),
                        context_roles: context_roles[ctx_start..ctx_start + ctx_len].to_vec(),
                        tool_history: history,
                        phase: current_phase,
                        target_tool: tool_class,
                        session_outcome: session.state.clone(),
                    });

                    tool_history.push(tool_class);
                }
            }
        }

        // Track phase changes from set_phase tool results
        if msg.role == "tool_result" {
            if let Some(tool_name) = msg
                .metadata
                .as_ref()
                .and_then(|m| m.get("tool_name"))
                .and_then(|v| v.as_str())
            {
                if tool_name == "set_phase" {
                    // Try to extract the phase from the content
                    let content_lower = msg.content.to_lowercase();
                    for phase in &[
                        "diagnosing",
                        "remediating",
                        "verifying",
                        "done",
                        "needs_human_attention",
                    ] {
                        if content_lower.contains(phase) {
                            current_phase = Vocabulary::phase_token(phase);
                            break;
                        }
                    }
                }
            }
        }

        // Accumulate context tokens
        let role_token = Vocabulary::role_token(&msg.role);
        let msg_tokens = vocab.encode_text(&msg.content);
        for &t in &msg_tokens {
            context_tokens.push(t);
            context_roles.push(role_token);
        }
        // Add separator
        context_tokens.push(tokenizer::SEP);
        context_roles.push(role_token);
    }

    samples
}

/// Extract an outcome prediction sample from a session.
///
/// Uses the first ~10 messages as input, with the terminal state as the label.
pub fn extract_outcome_sample(
    session: &ExportedSession,
    vocab: &Vocabulary,
) -> Option<OutcomeSample> {
    let target = match session.state.as_str() {
        "done" | "completed" => 0,
        "failed" => 1,
        "needs_human_attention" | "cancelled" | "paused" => 2,
        _ => return None,
    };

    let max_messages = 10;
    let mut tokens = Vec::new();
    let mut roles = Vec::new();

    // CLS token at start
    tokens.push(tokenizer::CLS);
    roles.push(tokenizer::CLS);

    for msg in session.messages.iter().take(max_messages) {
        let role_token = Vocabulary::role_token(&msg.role);
        let msg_tokens = vocab.encode_text(&msg.content);
        for &t in &msg_tokens {
            if tokens.len() >= MAX_SEQ_LEN - 1 {
                break;
            }
            tokens.push(t);
            roles.push(role_token);
        }
        tokens.push(tokenizer::SEP);
        roles.push(role_token);
    }

    // Truncate to max length
    tokens.truncate(MAX_SEQ_LEN);
    roles.truncate(MAX_SEQ_LEN);

    Some(OutcomeSample {
        tokens,
        roles,
        target,
    })
}
