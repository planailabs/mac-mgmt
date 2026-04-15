//! Matrix generator: cartesian product of (agent provider × LLM provider).
//!
//! Each cell yields a distinct [`mac_mgmt_common::ClusterConfig`] so that the
//! orchestrator can spin up one test instance per combination. Cells with a
//! cloud LLM are only emitted when the corresponding API key is configured.

use mac_mgmt_common::{
    AgentProvider, CloudConfig, CloudProvider, ClusterConfig, GlobalConfig, LlmProvider,
    LmsConfig, OllamaConfig,
};

use crate::config::MatrixConfig;

/// One matrix cell: a stable key and the ClusterConfig to install.
#[derive(Debug, Clone)]
pub struct MatrixCell {
    pub key: String,
    pub config: ClusterConfig,
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
                        let Some(provider) = parse_cloud_provider(cp) else {
                            continue;
                        };
                        let api_key = matrix
                            .cloud_api_keys
                            .get(cp)
                            .or_else(|| matrix.cloud_api_keys.get(provider.as_str()));
                        let Some(api_key) = api_key.cloned() else {
                            continue;
                        };
                        if agent == "none" && llm == "none" {
                            continue;
                        }
                        cells.push(build_cloud_cell(agent, provider, api_key));
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

fn parse_cloud_provider(s: &str) -> Option<CloudProvider> {
    match s {
        "anthropic" => Some(CloudProvider::Anthropic),
        "openai" => Some(CloudProvider::Openai),
        "google" => Some(CloudProvider::Google),
        "mistral" => Some(CloudProvider::Mistral),
        "groq" => Some(CloudProvider::Groq),
        "xai" => Some(CloudProvider::Xai),
        "deepseek" => Some(CloudProvider::Deepseek),
        "openrouter" => Some(CloudProvider::Openrouter),
        "together" => Some(CloudProvider::Together),
        "bedrock" => Some(CloudProvider::Bedrock),
        _ => None,
    }
}

fn parse_agent(s: &str) -> AgentProvider {
    match s {
        "openclaw" => AgentProvider::Openclaw,
        _ => AgentProvider::None,
    }
}

fn base_config(agent: &str, llm: LlmProvider) -> ClusterConfig {
    let mut c = ClusterConfig::default();
    c.global = GlobalConfig {
        llm_provider: llm,
        agent_provider: parse_agent(agent),
        agent_name: None,
        user_name: None,
    };
    c
}

fn build_cloud_cell(agent: &str, provider: CloudProvider, api_key: String) -> MatrixCell {
    let key = format!("{agent}-cloud-{}", provider.as_str());
    let mut c = base_config(agent, LlmProvider::Cloud);
    c.cloud = CloudConfig {
        provider: provider.clone(),
        api_key: Some(api_key),
        default_model: provider.default_model().to_string(),
        base_url: None,
        api: None,
        auth: None,
    };
    MatrixCell { key, config: c }
}

fn build_ollama_cell(agent: &str, model: &str) -> MatrixCell {
    let key = format!("{agent}-ollama");
    let mut c = base_config(agent, LlmProvider::Ollama);
    c.ollama = OllamaConfig {
        host: "127.0.0.1".into(),
        port: 11434,
        models: vec![model.to_string()],
        default_model: model.to_string(),
        flavour: "cpu".into(),
    };
    MatrixCell { key, config: c }
}

fn build_lms_cell(agent: &str, model: &str) -> MatrixCell {
    let key = format!("{agent}-lms");
    let mut c = base_config(agent, LlmProvider::Lms);
    c.lms = LmsConfig {
        host: "127.0.0.1".into(),
        port: 1234,
        models: vec![model.to_string()],
        default_model: model.to_string(),
    };
    MatrixCell { key, config: c }
}

fn build_none_llm_cell(agent: &str) -> MatrixCell {
    let key = format!("{agent}-nollm");
    let c = base_config(agent, LlmProvider::None);
    MatrixCell { key, config: c }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every generated cell must roundtrip through JSON cleanly against
    /// the same mac_mgmt_common::ClusterConfig the server uses — `put_config`
    /// otherwise hits 422 at runtime.
    #[test]
    fn every_cell_roundtrips_through_json() {
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
            let v = serde_json::to_value(&cell.config)
                .unwrap_or_else(|e| panic!("serialize {}: {e}", cell.key));
            let _: ClusterConfig = serde_json::from_value(v)
                .unwrap_or_else(|e| panic!("roundtrip {}: {e}", cell.key));
        }
    }
}
