use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::ExportedSession;

// Special token IDs
pub const PAD: u32 = 0;
pub const UNK: u32 = 1;
pub const CLS: u32 = 2;
pub const SEP: u32 = 3;

// Role tokens (offset 4)
pub const ROLE_SYSTEM: u32 = 4;
pub const ROLE_USER: u32 = 5;
pub const ROLE_ASSISTANT: u32 = 6;
pub const ROLE_TOOL_RESULT: u32 = 7;
pub const ROLE_PIN: u32 = 8;
pub const ROLE_SUMMARY: u32 = 9;

// Phase tokens (offset 10)
pub const PHASE_CREATED: u32 = 10;
pub const PHASE_INITIALIZING: u32 = 11;
pub const PHASE_DIAGNOSING: u32 = 12;
pub const PHASE_REMEDIATING: u32 = 13;
pub const PHASE_VERIFYING: u32 = 14;
pub const PHASE_DONE: u32 = 15;
pub const PHASE_COMPLETED: u32 = 16;
pub const PHASE_FAILED: u32 = 17;
pub const PHASE_CANCELLED: u32 = 18;
pub const PHASE_PAUSED: u32 = 19;
pub const PHASE_NEEDS_HUMAN: u32 = 20;

const _FIXED_TOKENS_END: u32 = 21;

/// All known healer tool names, in a fixed order for stable indexing.
pub const TOOL_NAMES: &[&str] = &[
    "list_files",
    "read_file",
    "write_file",
    "run_command",
    "fetch_logs",
    "list_file_tunnels",
    "list_shell_commands",
    "fetch_cluster_logs",
    "run_cluster_command",
    "check_node_online",
    "wait_for_node",
    "pin",
    "staff_ping",
    "set_phase",
    "name_session",
    "get_probe_status",
    "get_inventory",
    "get_system_sample",
    "get_probe_history",
    "get_metrics",
    "use_skill",
    "list_builtin_skills",
    "read_doc",
    "list_docs",
    "wait",
    "request_assessment",
    "get_config",
    "patch_config",
    "set_config",
    "list_skills",
    "list_mcp_servers",
    "add_skill",
    "remove_skill",
    "add_mcp_server",
    "remove_mcp_server",
    "send_push",
    "get_version_info",
    "get_heartbeat",
    "get_cluster_instances",
    "get_service_state",
    "nix_check_upgrades",
];

pub const NUM_TOOLS: usize = 41;

/// A domain-specific vocabulary for encoding healer session data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vocabulary {
    /// Token → ID mapping.
    token_to_id: HashMap<String, u32>,
    /// ID → Token mapping.
    id_to_token: Vec<String>,
}

impl Vocabulary {
    /// Build a vocabulary from the training corpus.
    ///
    /// Fixed tokens (special, roles, phases, tools) come first,
    /// then the top-K most frequent words from message content.
    pub fn build_from_sessions(sessions: &[ExportedSession]) -> Self {
        let mut token_to_id = HashMap::new();
        let mut id_to_token = Vec::new();

        // 1. Special tokens
        let specials = ["<pad>", "<unk>", "<cls>", "<sep>",
            "<system>", "<user>", "<assistant>", "<tool_result>", "<pin>", "<summary>",
            "<created>", "<initializing>", "<diagnosing>", "<remediating>", "<verifying>",
            "<done>", "<completed>", "<failed>", "<cancelled>", "<paused>", "<needs_human>"];

        for tok in &specials {
            let id = id_to_token.len() as u32;
            token_to_id.insert(tok.to_string(), id);
            id_to_token.push(tok.to_string());
        }

        // 2. Tool name tokens
        for name in TOOL_NAMES {
            let tok = format!("<tool:{name}>");
            let id = id_to_token.len() as u32;
            token_to_id.insert(tok.clone(), id);
            id_to_token.push(tok);
        }

        // 3. Collect word frequencies from message content
        let mut freq: HashMap<String, u64> = HashMap::new();
        for session in sessions {
            for msg in &session.messages {
                for word in tokenize_text(&msg.content) {
                    *freq.entry(word).or_default() += 1;
                }
            }
        }

        // 4. Take top-K words (target ~8K total vocab)
        let max_vocab: usize = 8192;
        let remaining = max_vocab.saturating_sub(id_to_token.len());
        let mut words: Vec<_> = freq.into_iter().collect();
        words.sort_by_key(|b| std::cmp::Reverse(b.1));

        for (word, _count) in words.into_iter().take(remaining) {
            if !token_to_id.contains_key(&word) {
                let id = id_to_token.len() as u32;
                token_to_id.insert(word.clone(), id);
                id_to_token.push(word);
            }
        }

        Self {
            token_to_id,
            id_to_token,
        }
    }

    pub fn size(&self) -> usize {
        self.id_to_token.len()
    }

    pub fn encode(&self, token: &str) -> u32 {
        self.token_to_id.get(token).copied().unwrap_or(UNK)
    }

    pub fn encode_text(&self, text: &str) -> Vec<u32> {
        tokenize_text(text)
            .into_iter()
            .map(|w| self.encode(&w))
            .collect()
    }

    pub fn tool_id(&self, tool_name: &str) -> u32 {
        let tok = format!("<tool:{tool_name}>");
        self.encode(&tok)
    }

    /// Get the index of a tool in TOOL_NAMES (for classification labels).
    pub fn tool_class(tool_name: &str) -> Option<usize> {
        TOOL_NAMES.iter().position(|&n| n == tool_name)
    }

    pub fn role_token(role: &str) -> u32 {
        match role {
            "system" => ROLE_SYSTEM,
            "user" => ROLE_USER,
            "assistant" => ROLE_ASSISTANT,
            "tool_result" => ROLE_TOOL_RESULT,
            "pin" => ROLE_PIN,
            "summary" => ROLE_SUMMARY,
            _ => UNK,
        }
    }

    pub fn phase_token(state: &str) -> u32 {
        match state {
            "created" => PHASE_CREATED,
            "initializing" => PHASE_INITIALIZING,
            "diagnosing" => PHASE_DIAGNOSING,
            "remediating" => PHASE_REMEDIATING,
            "verifying" => PHASE_VERIFYING,
            "done" => PHASE_DONE,
            "completed" => PHASE_COMPLETED,
            "failed" => PHASE_FAILED,
            "cancelled" => PHASE_CANCELLED,
            "paused" => PHASE_PAUSED,
            "needs_human_attention" => PHASE_NEEDS_HUMAN,
            _ => UNK,
        }
    }
}

/// Simple whitespace + lowercasing tokenization.
fn tokenize_text(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| c.is_whitespace() || c == '\n')
        .filter(|s| !s.is_empty())
        .take(256) // cap per-message tokens
        .map(|s| {
            // Strip punctuation from edges
            s.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}
