pub mod nexa_openclaw;
pub mod ollama_openclaw;

use anyhow::Result;

use crate::managed_service::ManagedService;
use crate::services::{mcporter::McPorter, nexa::Nexa, ollama::Ollama, openclaw::OpenClaw};
use mac_mgmt_common::{GlobalConfig, NexaConfig, OllamaConfig, OpenClawConfig};

/// A connector wires two services together after they are both healthy.
pub trait Connector: Send {
    fn name(&self) -> &str;
    fn connect(&self) -> Result<()>;
}

/// Build the list of managed services based on global provider settings.
pub fn build_services(
    global: &GlobalConfig,
    openclaw_cfg: OpenClawConfig,
    ollama_cfg: OllamaConfig,
    nexa_cfg: NexaConfig,
) -> Vec<Box<dyn ManagedService>> {
    let mut services: Vec<Box<dyn ManagedService>> = Vec::new();

    match global.agent_provider.as_str() {
        "openclaw" => {
            tracing::info!("agent_provider=openclaw");
            services.push(Box::new(OpenClaw::new(openclaw_cfg)));
        }
        "none" => tracing::info!("agent_provider=none, skipping agent services"),
        other => tracing::warn!("unknown agent_provider '{other}', skipping"),
    }

    match global.llm_provider.as_str() {
        "ollama" => {
            tracing::info!("llm_provider=ollama");
            services.push(Box::new(Ollama::new(ollama_cfg)));
        }
        "nexa" => {
            tracing::info!("llm_provider=nexa");
            services.push(Box::new(Nexa::new(nexa_cfg)));
        }
        "none" => tracing::info!("llm_provider=none, skipping LLM services"),
        other => tracing::warn!("unknown llm_provider '{other}', skipping"),
    }

    services.push(Box::new(McPorter));
    services
}

/// Build connectors that wire services together.
/// Connectors are run after all managed services have had their post_start.
pub fn build_connectors(
    global: &GlobalConfig,
    ollama_cfg: &OllamaConfig,
    nexa_cfg: &NexaConfig,
) -> Vec<Box<dyn Connector>> {
    let mut connectors: Vec<Box<dyn Connector>> = Vec::new();

    if global.agent_provider == "openclaw" {
        match global.llm_provider.as_str() {
            "ollama" => {
                connectors.push(Box::new(ollama_openclaw::OllamaOpenClaw {
                    default_model: ollama_cfg.default_model.clone(),
                }));
            }
            "nexa" => {
                connectors.push(Box::new(nexa_openclaw::NexaOpenClaw {
                    host: nexa_cfg.host.clone(),
                    port: nexa_cfg.port,
                    default_model: nexa_cfg.default_model.clone(),
                }));
            }
            _ => {}
        }
    }

    connectors
}

/// Recursively merge `source` into `target`. For objects, keys from source
/// are merged into target. For all other types, source overwrites target.
pub fn merge_json(target: &mut serde_json::Value, source: &serde_json::Value) {
    match (target, source) {
        (serde_json::Value::Object(target), serde_json::Value::Object(source)) => {
            for (key, value) in source {
                merge_json(
                    target.entry(key.clone()).or_insert(serde_json::Value::Null),
                    value,
                );
            }
        }
        (target, source) => {
            *target = source.clone();
        }
    }
}
