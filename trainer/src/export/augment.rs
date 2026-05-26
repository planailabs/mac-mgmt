//! Data augmentation strategies for SFT training data.
//!
//! All strategies are deterministic (seeded RNG) and don't require an LLM.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::sft::SftConversation;

/// Augmentation configuration.
pub struct AugmentConfig {
    /// Random seed for reproducibility.
    pub seed: u64,
    /// Number of augmented variants per original conversation.
    pub variants_per_session: usize,
    /// Whether to apply tool output perturbation.
    pub perturb_tool_output: bool,
    /// Whether to apply phase dropout (drop intermediate tool calls).
    pub phase_dropout: bool,
    /// Phase dropout probability (0.0–1.0).
    pub dropout_prob: f32,
}

impl Default for AugmentConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            variants_per_session: 3,
            perturb_tool_output: true,
            phase_dropout: true,
            dropout_prob: 0.15,
        }
    }
}

/// Generate augmented variants of SFT conversations.
///
/// Returns the originals + augmented copies.
pub fn augment(conversations: &[SftConversation], config: &AugmentConfig) -> Vec<SftConversation> {
    let mut rng = StdRng::seed_from_u64(config.seed);
    let mut result = Vec::with_capacity(conversations.len() * (1 + config.variants_per_session));

    // Always include originals
    result.extend_from_slice(conversations);

    for conv in conversations {
        for _ in 0..config.variants_per_session {
            let mut variant = conv.clone();

            if config.perturb_tool_output {
                perturb_tool_outputs(&mut variant, &mut rng);
            }

            if config.phase_dropout {
                phase_dropout(&mut variant, config.dropout_prob, &mut rng);
            }

            // Mark augmented in metadata
            variant.metadata.sample_weight = variant
                .metadata
                .sample_weight
                .map(|w| w * 0.9) // slightly downweight augmented samples
                .or(Some(0.9));

            result.push(variant);
        }
    }

    result
}

/// Perturb tool outputs: truncate long outputs at different points,
/// add/remove trailing whitespace, shuffle JSON key order.
fn perturb_tool_outputs(conv: &mut SftConversation, rng: &mut StdRng) {
    for turn in &mut conv.messages {
        if turn.role != "tool" {
            continue;
        }
        let Some(ref mut content) = turn.content else {
            continue;
        };

        // Strategy 1: Truncate at a random point (if long enough)
        if content.len() > 500 && rng.random_bool(0.3) {
            let cut = rng.random_range(content.len() / 2..content.len());
            content.truncate(cut);
            content.push_str("\n... [truncated]");
            continue;
        }

        // Strategy 2: If content looks like JSON, try to reorder keys
        if content.starts_with('{') && rng.random_bool(0.4) {
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(content) {
                shuffle_json_keys(&mut val, rng);
                if let Ok(rewritten) = serde_json::to_string_pretty(&val) {
                    *content = rewritten;
                }
            }
            continue;
        }

        // Strategy 3: Add/remove trailing whitespace/newlines
        if rng.random_bool(0.3) {
            *content = content.trim().to_string();
            if rng.random_bool(0.5) {
                content.push('\n');
            }
        }
    }
}

/// Randomly shuffle JSON object keys (shallow, one level).
fn shuffle_json_keys(val: &mut serde_json::Value, rng: &mut StdRng) {
    if let serde_json::Value::Object(map) = val {
        let entries: Vec<(String, serde_json::Value)> = map.clone().into_iter().collect();
        if entries.len() <= 1 {
            return;
        }
        // Fisher-Yates on a vec, then rebuild
        let mut shuffled = entries;
        for i in (1..shuffled.len()).rev() {
            let j = rng.random_range(0..=i);
            shuffled.swap(i, j);
        }
        *map = shuffled.into_iter().collect();
    }
}

/// Phase dropout: randomly drop intermediate tool call/response pairs,
/// keeping the first and last tool call per "phase" (consecutive tool calls).
fn phase_dropout(conv: &mut SftConversation, dropout_prob: f32, rng: &mut StdRng) {
    // Find tool-call groups: sequences of (assistant-with-tool_calls, tool-response)
    // We identify tool response indices and decide which to drop
    let mut tool_response_indices: Vec<usize> = Vec::new();
    for (i, turn) in conv.messages.iter().enumerate() {
        if turn.role == "tool" {
            tool_response_indices.push(i);
        }
    }

    if tool_response_indices.len() <= 2 {
        return; // Not enough tool calls to drop any
    }

    // Never drop first or last tool response
    let droppable = &tool_response_indices[1..tool_response_indices.len() - 1];

    let mut indices_to_remove: Vec<usize> = Vec::new();
    for &idx in droppable {
        if rng.random_bool(dropout_prob as f64) {
            indices_to_remove.push(idx);

            // Also find and remove the corresponding assistant turn with the tool_call
            if let Some(tool_call_id) = &conv.messages[idx].tool_call_id {
                // Look backwards for the assistant turn containing this tool_call
                for j in (0..idx).rev() {
                    if conv.messages[j].role == "assistant"
                        && let Some(ref calls) = conv.messages[j].tool_calls
                        && calls.iter().any(|c| c.id == *tool_call_id)
                    {
                        // Only remove if this assistant turn has exactly this one tool call
                        if calls.len() == 1 {
                            indices_to_remove.push(j);
                        }
                        break;
                    }
                }
            }
        }
    }

    // Remove in reverse order to preserve indices
    indices_to_remove.sort_unstable();
    indices_to_remove.dedup();
    for &idx in indices_to_remove.iter().rev() {
        if idx < conv.messages.len() {
            conv.messages.remove(idx);
        }
    }
}
