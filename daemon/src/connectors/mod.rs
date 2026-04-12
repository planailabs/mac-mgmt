pub mod cloud_openclaw;
pub mod lms_openclaw;
pub mod nexa_openclaw;
pub mod ollama_openclaw;
pub mod relay_openclaw;

use anyhow::Result;

use crate::managed_service::ManagedService;
use crate::services::{apprise::Apprise, lms::Lms, mcporter::McPorter, nexa::Nexa, ollama::Ollama, openclaw::OpenClaw};
use mac_mgmt_common::{AgentProvider, CloudConfig, GlobalConfig, LlmProvider, LmsConfig, NexaConfig, OllamaConfig, OpenClawConfig};

/// A connector wires two services together after they are both healthy.
///
/// Dependencies are either managed service names (must have post_start done)
/// or config provider names (must be set in the ConfigStore). When any
/// dependency changes, the connector is re-run.
pub trait Connector: Send {
    fn name(&self) -> &str;
    /// Names of services and/or config providers this connector depends on.
    fn depends_on(&self) -> &[&str];
    /// Run the connector. `configs` contains the current values of all
    /// config provider dependencies.
    fn connect(&self, configs: &std::collections::HashMap<String, serde_json::Value>) -> Result<()>;
}

/// Build the list of managed services based on global provider settings.
pub fn build_services(
    global: &GlobalConfig,
    openclaw_cfg: OpenClawConfig,
    ollama_cfg: OllamaConfig,
    nexa_cfg: NexaConfig,
    lms_cfg: LmsConfig,
) -> Vec<Box<dyn ManagedService>> {
    let mut services: Vec<Box<dyn ManagedService>> = Vec::new();

    match global.agent_provider {
        AgentProvider::Openclaw => {
            tracing::info!("agent_provider=openclaw");
            services.push(Box::new(OpenClaw::new(openclaw_cfg)));
        }
        AgentProvider::None => tracing::info!("agent_provider=none, skipping agent services"),
    }

    match global.llm_provider {
        LlmProvider::Ollama => {
            tracing::info!("llm_provider=ollama");
            services.push(Box::new(Ollama::new(ollama_cfg)));
        }
        LlmProvider::Nexa => {
            tracing::info!("llm_provider=nexa");
            services.push(Box::new(Nexa::new(nexa_cfg)));
        }
        LlmProvider::Lms => {
            tracing::info!("llm_provider=lms");
            services.push(Box::new(Lms::new(lms_cfg)));
        }
        LlmProvider::Cloud => tracing::info!("llm_provider=cloud, no local LLM service"),
        LlmProvider::None => tracing::info!("llm_provider=none, skipping LLM services"),
    }

    services.push(Box::new(McPorter));
    services.push(Box::new(Apprise));
    services
}

/// Build connectors that wire services together.
/// Connectors are run after all managed services have had their post_start.
pub fn build_connectors(
    global: &GlobalConfig,
    ollama_cfg: &OllamaConfig,
    nexa_cfg: &NexaConfig,
    lms_cfg: &LmsConfig,
    cloud_cfg: &CloudConfig,
) -> Vec<Box<dyn Connector>> {
    let mut connectors: Vec<Box<dyn Connector>> = Vec::new();

    if global.agent_provider == AgentProvider::Openclaw {
        connectors.push(Box::new(relay_openclaw::RelayOpenClaw));

        match global.llm_provider {
            LlmProvider::Ollama => {
                connectors.push(Box::new(ollama_openclaw::OllamaOpenClaw {
                    default_model: ollama_cfg.default_model.clone(),
                }));
            }
            LlmProvider::Nexa => {
                connectors.push(Box::new(nexa_openclaw::NexaOpenClaw {
                    host: nexa_cfg.host.clone(),
                    port: nexa_cfg.port,
                    default_model: nexa_cfg.default_model.clone(),
                }));
            }
            LlmProvider::Lms => {
                connectors.push(Box::new(lms_openclaw::LmsOpenClaw {
                    host: lms_cfg.host.clone(),
                    port: lms_cfg.port,
                    default_model: lms_cfg.default_model.clone(),
                }));
            }
            LlmProvider::Cloud => {
                connectors.push(Box::new(cloud_openclaw::CloudOpenClaw {
                    config: cloud_cfg.clone(),
                }));
            }
            LlmProvider::None => {}
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
