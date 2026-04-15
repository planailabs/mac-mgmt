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
    /// JSON body sent as `{"config": <this>}` to PUT /api/setting/config.
    pub config: Value,
}

pub fn generate(matrix: &MatrixConfig) -> Vec<MatrixCell> {
    let agents = matrix
        .agents
        .clone()
        .unwrap_or_else(|| vec!["openclaw".into(), "none".into()]);
    let llms = matrix
        .llms
        .clone()
        .unwrap_or_else(|| vec!["ollama".into(), "lms".into(), "cloud".into()]);
    let cloud_providers = matrix.cloud_providers.clone().unwrap_or_else(|| {
        vec![
            "anthropic", "openai", "google", "mistral", "groq", "xai", "deepseek",
            "openrouter", "together", "bedrock",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    });

    let mut cells = Vec::new();
    for agent in &agents {
        for llm in &llms {
            match llm.as_str() {
                "cloud" => {
                    for cp in &cloud_providers {
                        if !is_known_cloud_provider(cp) {
                            continue;
                        }
                        let Some(api_key) = matrix.cloud_api_keys.get(cp).cloned() else {
                            continue;
                        };
                        cells.push(build_cloud_cell(agent, cp, &api_key));
                    }
                }
                "ollama" => {
                    cells.push(build_ollama_cell(agent, &matrix.ollama_model));
                }
                "lms" => {
                    cells.push(build_lms_cell(agent, &matrix.lms_model));
                }
                "none" => {
                    if agent != "none" {
                        cells.push(build_none_llm_cell(agent));
                    }
                }
                _ => {}
            }
        }
    }

    cells
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
        "llm_provider": llm,
        "agent_provider": agent,
    })
}

fn build_cloud_cell(agent: &str, provider: &str, api_key: &str) -> MatrixCell {
    let key = format!("{agent}-cloud-{provider}");
    let config = json!({
        "global": global(agent, "cloud"),
        "cloud": {
            "provider": provider,
            "api_key": api_key,
            "default_model": cloud_default_model(provider),
        },
    });
    MatrixCell { key, config }
}

fn build_ollama_cell(agent: &str, model: &str) -> MatrixCell {
    let key = format!("{agent}-ollama");
    let config = json!({
        "global": global(agent, "ollama"),
        "ollama": {
            "models": [model],
            "default_model": model,
        },
    });
    MatrixCell { key, config }
}

fn build_lms_cell(agent: &str, model: &str) -> MatrixCell {
    let key = format!("{agent}-lms");
    let config = json!({
        "global": global(agent, "lms"),
        "lms": {
            "models": [model],
            "default_model": model,
        },
    });
    MatrixCell { key, config }
}

fn build_none_llm_cell(agent: &str) -> MatrixCell {
    let key = format!("{agent}-nollm");
    let config = json!({
        "global": global(agent, "none"),
    });
    MatrixCell { key, config }
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
        m.ollama_model = "smollm2:1.7b".into();
        m.lms_model = "smollm2-1.7b-instruct".into();
        m.cloud_api_keys
            .insert("anthropic".into(), "sk-ant-test".into());
        m.cloud_api_keys
            .insert("openai".into(), "sk-test".into());

        let cells = generate(&m);
        assert!(!cells.is_empty(), "matrix should not be empty");
        for cell in cells {
            let _: ClusterConfig = serde_json::from_value(cell.config.clone())
                .unwrap_or_else(|e| panic!("parse {}: {e}\n  payload: {}", cell.key, cell.config));
        }
    }

    #[test]
    fn sparse_configs_omit_irrelevant_sections() {
        let mut m = MatrixConfig::default();
        m.ollama_model = "smollm2:1.7b".into();
        m.lms_model = "smollm2-1.7b-instruct".into();
        let cells = generate(&m);
        for cell in cells {
            let obj = cell.config.as_object().expect("object");
            let present: Vec<&String> = obj.keys().collect();
            // global is required; cloud/ollama/lms/daemon/metrics/etc. should
            // only appear when the cell actively sets them.
            assert!(present.iter().any(|k| *k == "global"), "{}: no global", cell.key);
            assert!(!present.iter().any(|k| *k == "daemon"), "{}: daemon leaked", cell.key);
            assert!(!present.iter().any(|k| *k == "metrics"), "{}: metrics leaked", cell.key);
            assert!(!present.iter().any(|k| *k == "relay"), "{}: relay leaked", cell.key);
            assert!(
                !present.iter().any(|k| *k == "notifications"),
                "{}: notifications leaked", cell.key
            );
            if !cell.key.contains("-cloud-") {
                assert!(!present.iter().any(|k| *k == "cloud"), "{}: cloud leaked", cell.key);
            }
        }
    }
}
