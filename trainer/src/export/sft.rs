//! Convert ExportedSession → ChatML with tool-use format for SFT fine-tuning.

use serde::{Deserialize, Serialize};

use super::ExportedSession;

/// A single turn in an SFT conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftTurn {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<SftToolCall>>,
    /// For role="tool": the ID of the tool call this responds to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// For role="tool": the tool function name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub function: SftFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftFunction {
    pub name: String,
    pub arguments: String,
}

/// A complete SFT conversation with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftConversation {
    pub messages: Vec<SftTurn>,
    pub metadata: SftMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftMetadata {
    pub session_id: String,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curriculum_rank: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_weight: Option<f64>,
}

const DEFAULT_SYSTEM_TEMPLATE: &str = r#"You are a server diagnosis and repair agent for the mac-mgmt fleet management system.

## Target
- Cluster: {cluster}
- Instance: {instance}

## Detected Issues
{issues}

## Workflow
1. Gather information using read-only tools (get_probe_status, fetch_logs, read_file, get_system_sample, get_metrics)
2. Diagnose the root cause — pin your findings with the `pin` tool (slot: "diagnosis")
3. Remediate with minimal, targeted changes
4. Verify fixes with `get_probe_status` or `request_assessment`
5. Transition to done with `set_phase("done")`

If you cannot resolve the issue, use `staff_ping` to escalate to an admin."#;

/// Convert an ExportedSession into an SFT conversation.
///
/// Reconstructs assistant tool_calls from the message sequence:
/// an assistant message followed by tool_result messages becomes
/// an assistant turn with tool_calls + corresponding tool responses.
pub fn convert_session(
    session: &ExportedSession,
    system_template: Option<&str>,
) -> SftConversation {
    let template = system_template.unwrap_or(DEFAULT_SYSTEM_TEMPLATE);

    // Build system prompt from template
    let issues_text = if session.initial_issues.is_array() {
        session
            .initial_issues
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| {
                let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                let healthy = v.get("healthy").and_then(|h| h.as_bool()).unwrap_or(true);
                if !healthy {
                    let error = v
                        .get("last_error")
                        .and_then(|e| e.as_str())
                        .unwrap_or("unhealthy");
                    Some(format!("- {name}: {error}"))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        session.initial_issues.to_string()
    };

    let system_prompt = template
        .replace("{cluster}", &session.cluster_id.to_string())
        .replace("{instance}", &session.instance_id)
        .replace("{issues}", &issues_text);

    let mut turns = Vec::new();
    turns.push(SftTurn {
        role: "system".to_string(),
        content: Some(system_prompt),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });

    // Walk messages and reconstruct tool-call pairs.
    // Strategy: collect assistant text, then when we see tool_result(s),
    // attach them as tool_calls on the preceding assistant turn.
    let mut tool_call_counter = 0u32;
    let mut pending_assistant_text: Option<String> = None;
    let mut pending_tool_calls: Vec<SftToolCall> = Vec::new();
    let mut pending_tool_responses: Vec<SftTurn> = Vec::new();

    for msg in &session.messages {
        match msg.role.as_str() {
            "assistant" => {
                // Flush any pending assistant + tool calls
                flush_pending(
                    &mut turns,
                    &mut pending_assistant_text,
                    &mut pending_tool_calls,
                    &mut pending_tool_responses,
                );
                pending_assistant_text = Some(msg.content.clone());
            }
            "tool_result" => {
                let tool_name = msg
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("tool_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                let tool_args = msg
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("tool_args"))
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "{}".to_string());

                let call_id = format!("call_{:03}", tool_call_counter);
                tool_call_counter += 1;

                pending_tool_calls.push(SftToolCall {
                    id: call_id.clone(),
                    type_: "function".to_string(),
                    function: SftFunction {
                        name: tool_name.to_string(),
                        arguments: tool_args,
                    },
                });

                // Truncate very long tool outputs for training
                let content = truncate_tool_output(&msg.content, 4096);

                pending_tool_responses.push(SftTurn {
                    role: "tool".to_string(),
                    content: Some(content),
                    tool_calls: None,
                    tool_call_id: Some(call_id),
                    name: Some(tool_name.to_string()),
                });
            }
            "state_change" => {
                // Fold state changes into the assistant's reasoning
                let state = msg
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("state"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(&msg.content);

                if let Some(ref mut text) = pending_assistant_text {
                    text.push_str(&format!("\n[STATE: {state}]"));
                }
                // If no pending assistant, create a minimal one
                else {
                    pending_assistant_text = Some(format!("[STATE: {state}]"));
                }
            }
            "user" => {
                flush_pending(
                    &mut turns,
                    &mut pending_assistant_text,
                    &mut pending_tool_calls,
                    &mut pending_tool_responses,
                );
                turns.push(SftTurn {
                    role: "user".to_string(),
                    content: Some(msg.content.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
            }
            // Skip summary messages (synthetic)
            "summary" => {}
            // Fold pins into assistant text
            _ => {
                let pin_slot = msg
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("tool_name"))
                    .and_then(|v| v.as_str());

                if (pin_slot == Some("pin") || msg.role == "pin")
                    && let Some(ref mut text) = pending_assistant_text
                {
                    text.push_str(&format!("\n[PIN] {}", msg.content));
                }
            }
        }
    }

    // Flush final pending
    flush_pending(
        &mut turns,
        &mut pending_assistant_text,
        &mut pending_tool_calls,
        &mut pending_tool_responses,
    );

    SftConversation {
        messages: turns,
        metadata: SftMetadata {
            session_id: session.id.to_string(),
            outcome: session.state.clone(),
            curriculum_rank: None,
            sample_weight: None,
        },
    }
}

fn flush_pending(
    turns: &mut Vec<SftTurn>,
    pending_text: &mut Option<String>,
    pending_calls: &mut Vec<SftToolCall>,
    pending_responses: &mut Vec<SftTurn>,
) {
    if pending_text.is_none() && pending_calls.is_empty() {
        return;
    }

    let text = pending_text.take();
    let calls = if pending_calls.is_empty() {
        None
    } else {
        Some(std::mem::take(pending_calls))
    };

    turns.push(SftTurn {
        role: "assistant".to_string(),
        content: text,
        tool_calls: calls,
        tool_call_id: None,
        name: None,
    });

    // Append tool responses after the assistant turn
    turns.append(pending_responses);
}

/// Truncate tool output to a maximum character length, adding a marker if truncated.
fn truncate_tool_output(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        return content.to_string();
    }
    let mut truncated = content[..max_chars].to_string();
    truncated.push_str("\n... [truncated]");
    truncated
}

/// Convert multiple sessions into SFT conversations.
pub fn convert_sessions(
    sessions: &[ExportedSession],
    system_template: Option<&str>,
) -> Vec<SftConversation> {
    sessions
        .iter()
        .map(|s| convert_session(s, system_template))
        .collect()
}
