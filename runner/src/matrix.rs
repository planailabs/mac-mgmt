//! Matrix generator: cartesian product of (agent provider × LLM provider).
//!
//! Each cell emits a **sparse** ClusterConfig JSON fragment containing only
//! the sections the cell cares about. Everything else falls back to the
//! server's `#[serde(default)]` values — crucially we don't round-trip a
//! fully-defaulted `CloudConfig` with `default_model: ""` through every
//! request.

use serde_json::{Value, json};

use crate::config::MatrixConfig;

#[derive(Debug, Clone)]
pub struct MatrixCell {
    pub key: String,
    /// JSON body for PUT /api/setting/config (already unwrapped — the
    /// server's SetConfigBody is `#[serde(flatten)]`).
    pub config: Value,
    /// How many Incus instances this cell launches under one mgmt cluster.
    pub node_count: u32,
}

pub fn generate(matrix: &MatrixConfig) -> Vec<MatrixCell> {
    let agents = matrix
        .agents
        .clone()
        .unwrap_or_else(|| vec!["openclaw".into(), "opencode".into(), "none".into()]);
    let mut llms = matrix
        .llms
        .clone()
        .unwrap_or_else(|| vec!["ollama".into(), "lms".into(), "cloud".into()]);
    if !matrix.lms_enabled {
        llms.retain(|l| l != "lms");
    }
    let cloud_providers = matrix.cloud_providers.clone().unwrap_or_else(|| {
        vec![
            "anthropic",
            "openai",
            "google",
            "mistral",
            "groq",
            "xai",
            "deepseek",
            "openrouter",
            "together",
            "bedrock",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    });
    let sizes: Vec<u32> = matrix
        .cluster_sizes
        .iter()
        .copied()
        .filter(|&n| n >= 1)
        .collect();
    let sizes = if sizes.is_empty() { vec![1] } else { sizes };

    // Interleave sizes so every (agent × llm) pair alternates between its
    // 1-node and 2-node variants in the output. Without this, size=1 cells
    // come first and starve size=2 at the max_concurrent_launches throttle.
    let mut cells = Vec::new();
    for agent in &agents {
        for llm in &llms {
            for size in &sizes {
                match llm.as_str() {
                    "cloud" => {
                        for cp in &cloud_providers {
                            if !is_known_cloud_provider(cp) {
                                continue;
                            }
                            let Some(api_key) = matrix.cloud_api_keys.get(cp).cloned() else {
                                continue;
                            };
                            cells.push(build_cloud_cell(agent, cp, &api_key, *size));
                        }
                    }
                    "ollama" => {
                        cells.push(build_ollama_cell(agent, &matrix.ollama_model, *size));
                    }
                    "lms" => {
                        cells.push(build_lms_cell(agent, &matrix.lms_model, *size));
                    }
                    "none" => {
                        if agent != "none" {
                            cells.push(build_none_llm_cell(agent, *size));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    cells
}

/// Cells with `size == 1` keep their legacy key for continuity with existing
/// state files; larger sizes append `-n{size}`.
fn with_size(key: String, size: u32) -> String {
    if size <= 1 {
        key
    } else {
        format!("{key}-n{size}")
    }
}

fn is_known_cloud_provider(s: &str) -> bool {
    matches!(
        s,
        "anthropic"
            | "openai"
            | "google"
            | "mistral"
            | "groq"
            | "xai"
            | "deepseek"
            | "openrouter"
            | "together"
            | "bedrock"
    )
}

fn cloud_default_model(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "anthropic/claude-sonnet-4-6",
        "openai" => "openai/gpt-5.4",
        "google" => "google/gemini-3-flash-preview",
        "mistral" => "mistral/mistral-large-latest",
        "groq" => "groq/llama-4-scout-17b-16e-instruct",
        "xai" => "xai/grok-3-mini",
        "deepseek" => "deepseek/deepseek-chat",
        "openrouter" => "openrouter/auto",
        "together" => "together/meta-llama/Llama-4-Maverick-17B-128E-Instruct-Turbo",
        "bedrock" => "amazon-bedrock/us.anthropic.claude-sonnet-4-6-v1:0",
        _ => "",
    }
}

fn global(agent: &str, llm: &str) -> Value {
    json!({
        "default_llm": llm,
        "default_agent": agent,
    })
}

fn relay() -> Value {
    json!({
        "url": "wss://relay.plan.ai",
        "remote_ssh_enabled": true,
    })
}

/// AI proxy config with a default test key.
/// Raw key: sk-1234 (SHA2-256 multihash below).
fn ai_proxy() -> Value {
    json!({
        "enabled": true,
        "keys": [{
            "name": "default",
            "key_hash": "122088dc28d0f030c55ed4ab77ed8faf098196cb1c05df778539800c9f1243fe6b4b",
        }],
    })
}

fn memvault_if_multi(size: u32) -> Option<Value> {
    if size > 1 {
        Some(json!({ "enabled": true }))
    } else {
        None
    }
}

fn build_cloud_cell(agent: &str, provider: &str, api_key: &str, size: u32) -> MatrixCell {
    let key = with_size(format!("{agent}-cloud-{provider}"), size);
    let mut config = json!({
        "global": global(agent, "cloud"),
        "cloud": [{
            "enabled": true,
            "provider": provider,
            "api_key": api_key,
            "default_model": cloud_default_model(provider),
        }],
        "relay": relay(),
        "ai_proxy": ai_proxy(),
    });
    if let Some(mv) = memvault_if_multi(size) {
        config["memvault"] = mv;
    }
    MatrixCell {
        key,
        config,
        node_count: size,
    }
}

fn build_ollama_cell(agent: &str, model: &str, size: u32) -> MatrixCell {
    let key = with_size(format!("{agent}-ollama"), size);
    let mut config = json!({
        "global": global(agent, "ollama"),
        "ollama": {
            "enabled": true,
            "models": [model],
            "default_model": model,
        },
        "relay": relay(),
        "ai_proxy": ai_proxy(),
    });
    if let Some(mv) = memvault_if_multi(size) {
        config["memvault"] = mv;
    }
    MatrixCell {
        key,
        config,
        node_count: size,
    }
}

fn build_lms_cell(agent: &str, model: &str, size: u32) -> MatrixCell {
    let key = with_size(format!("{agent}-lms"), size);
    let mut config = json!({
        "global": global(agent, "lms"),
        "lms": {
            "enabled": true,
            "models": [model],
            "default_model": model,
        },
        "relay": relay(),
        "ai_proxy": ai_proxy(),
    });
    if let Some(mv) = memvault_if_multi(size) {
        config["memvault"] = mv;
    }
    MatrixCell {
        key,
        config,
        node_count: size,
    }
}

fn build_none_llm_cell(agent: &str, size: u32) -> MatrixCell {
    let key = with_size(format!("{agent}-nollm"), size);
    let mut config = json!({
        "global": global(agent, "none"),
        "relay": relay(),
        "ai_proxy": ai_proxy(),
    });
    if let Some(mv) = memvault_if_multi(size) {
        config["memvault"] = mv;
    }
    MatrixCell {
        key,
        config,
        node_count: size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mac_mgmt_common::ClusterConfig;

    /// Every generated cell must parse cleanly as a ClusterConfig against
    /// the same mac_mgmt_common the server uses — catches any field drift
    /// between the runner's sparse JSON and the expected schema.
    #[test]
    fn every_cell_parses_as_cluster_config() {
        let mut m = MatrixConfig::default();
        m.ollama_model = "qwen2.5:0.5b".into();
        m.lms_model = "qwen2.5-0.5b-instruct".into();
        m.cluster_sizes = vec![1, 2];
        m.cloud_api_keys
            .insert("anthropic".into(), "sk-ant-test".into());
        m.cloud_api_keys.insert("openai".into(), "sk-test".into());

        let cells = generate(&m);
        assert!(!cells.is_empty(), "matrix should not be empty");
        for cell in cells {
            let _: ClusterConfig = serde_json::from_value(cell.config.clone())
                .unwrap_or_else(|e| panic!("parse {}: {e}\n  payload: {}", cell.key, cell.config));
        }
    }

    #[test]
    fn cluster_sizes_multiply_matrix() {
        let mut m = MatrixConfig::default();
        m.ollama_model = "qwen2.5:0.5b".into();
        m.lms_model = "qwen2.5-0.5b-instruct".into();
        m.agents = Some(vec!["openclaw".into()]);
        m.llms = Some(vec!["ollama".into()]);

        m.cluster_sizes = vec![1];
        assert_eq!(generate(&m).len(), 1);

        m.cluster_sizes = vec![1, 2];
        let cells = generate(&m);
        assert_eq!(cells.len(), 2);
        assert!(
            cells
                .iter()
                .any(|c| c.key == "openclaw-ollama" && c.node_count == 1)
        );
        assert!(
            cells
                .iter()
                .any(|c| c.key == "openclaw-ollama-n2" && c.node_count == 2)
        );
    }

    /// End-to-end fairness simulation: take the interleaved matrix output
    /// and run it through a throttled reconcile loop. Every cell must
    /// reach "running" within a bounded number of ticks, and size=2 cells
    /// must not all settle strictly after all size=1 cells.
    #[test]
    fn throttle_does_not_starve_n2_cells() {
        let mut m = MatrixConfig::default();
        m.ollama_model = "qwen2.5:0.5b".into();
        m.lms_enabled = true;
        m.lms_model = "qwen2.5-0.5b-instruct".into();
        m.agents = Some(vec!["openclaw".into(), "none".into()]);
        m.llms = Some(vec!["ollama".into(), "lms".into()]);
        m.cluster_sizes = vec![1, 2];
        let cells = generate(&m);
        let keys: Vec<String> = cells.iter().map(|c| c.key.clone()).collect();

        #[derive(Clone, Copy, Debug, PartialEq)]
        enum Stage {
            Pending,
            ConfigPushed,
            Launching(u32), // tick it entered Launching
            Running,
        }
        // Parameters matching production-ish defaults.
        let cap: usize = 3;
        let launch_ticks: u32 = 5;
        let max_ticks: u32 = 200;

        let mut stages: Vec<Stage> = vec![Stage::Pending; keys.len()];
        let mut settled_at: Vec<Option<u32>> = vec![None; keys.len()];

        for tick in 0..max_ticks {
            // Pending → ConfigPushed (cheap, no throttle)
            for s in stages.iter_mut() {
                if *s == Stage::Pending {
                    *s = Stage::ConfigPushed;
                }
            }
            // ConfigPushed → Launching (throttle on current Launching count),
            // iterated in the matrix generator's order — the property under test.
            for i in 0..stages.len() {
                if stages[i] != Stage::ConfigPushed {
                    continue;
                }
                let launching = stages
                    .iter()
                    .filter(|s| matches!(s, Stage::Launching(_)))
                    .count();
                if launching < cap {
                    stages[i] = Stage::Launching(tick);
                }
            }
            // Launching → Running after fixed launch_ticks
            for (i, s) in stages.iter_mut().enumerate() {
                if let Stage::Launching(started) = *s {
                    if tick.saturating_sub(started) >= launch_ticks {
                        *s = Stage::Running;
                        settled_at[i] = Some(tick);
                    }
                }
            }
            if stages.iter().all(|s| *s == Stage::Running) {
                break;
            }
        }

        for (i, key) in keys.iter().enumerate() {
            assert!(
                settled_at[i].is_some(),
                "cell {key} never reached Running within {max_ticks} ticks",
            );
        }

        let n2_ticks: Vec<u32> = keys
            .iter()
            .zip(settled_at.iter())
            .filter(|(k, _)| k.ends_with("-n2"))
            .filter_map(|(_, t)| *t)
            .collect();
        let n1_ticks: Vec<u32> = keys
            .iter()
            .zip(settled_at.iter())
            .filter(|(k, _)| !k.ends_with("-n2"))
            .filter_map(|(_, t)| *t)
            .collect();
        assert!(
            !n2_ticks.is_empty() && !n1_ticks.is_empty(),
            "need both sizes"
        );
        let n2_min = n2_ticks.iter().min().copied().unwrap();
        let n1_max = n1_ticks.iter().max().copied().unwrap();
        assert!(
            n2_min <= n1_max,
            "interleaving broken: earliest n2 tick {n2_min} > latest n1 tick {n1_max}"
        );
    }

    /// Sizes must interleave within an (agent, llm) pair so the launch
    /// throttle doesn't starve 2-node cells behind all the 1-node ones.
    #[test]
    fn cells_interleave_sizes_per_agent_llm_pair() {
        let mut m = MatrixConfig::default();
        m.ollama_model = "qwen2.5:0.5b".into();
        m.lms_enabled = true;
        m.lms_model = "qwen2.5-0.5b-instruct".into();
        m.agents = Some(vec!["openclaw".into(), "none".into()]);
        m.llms = Some(vec!["ollama".into(), "lms".into()]);
        m.cluster_sizes = vec![1, 2];
        let keys: Vec<String> = generate(&m).into_iter().map(|c| c.key).collect();
        // 1-node and 2-node for the same (agent, llm) must be adjacent.
        assert_eq!(
            keys,
            vec![
                "openclaw-ollama",
                "openclaw-ollama-n2",
                "openclaw-lms",
                "openclaw-lms-n2",
                "none-ollama",
                "none-ollama-n2",
                "none-lms",
                "none-lms-n2",
            ]
        );
    }

    #[test]
    fn sparse_configs_omit_irrelevant_sections() {
        let mut m = MatrixConfig::default();
        m.ollama_model = "qwen2.5:0.5b".into();
        m.lms_model = "qwen2.5-0.5b-instruct".into();
        m.cluster_sizes = vec![1];
        let cells = generate(&m);
        for cell in cells {
            let obj = cell.config.as_object().expect("object");
            let present: Vec<&String> = obj.keys().collect();
            // global is required; cloud/ollama/lms/daemon/metrics/etc. should
            // only appear when the cell actively sets them.
            assert!(
                present.iter().any(|k| *k == "global"),
                "{}: no global",
                cell.key
            );
            assert!(
                !present.iter().any(|k| *k == "daemon"),
                "{}: daemon leaked",
                cell.key
            );
            assert!(
                !present.iter().any(|k| *k == "metrics"),
                "{}: metrics leaked",
                cell.key
            );
            assert!(
                present.iter().any(|k| *k == "relay"),
                "{}: relay missing",
                cell.key
            );
            assert!(
                !present.iter().any(|k| *k == "notifications"),
                "{}: notifications leaked",
                cell.key
            );
            if !cell.key.contains("-cloud-") {
                assert!(
                    !present.iter().any(|k| *k == "cloud"),
                    "{}: cloud leaked",
                    cell.key
                );
            }
        }
    }
}
