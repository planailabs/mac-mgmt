pub mod cloud_openclaw;
pub mod cloud_opencode;
pub mod lms_openclaw;
pub mod lms_opencode;
pub mod ollama_openclaw;
pub mod ollama_opencode;
pub mod relay_ollama;
pub mod relay_openclaw;
pub mod relay_opencode;

use anyhow::Result;

use crate::managed_service::ManagedService;
use crate::services::{
    apprise::Apprise, lms::Lms, mcporter::McPorter, nvidia_smi::NvidiaSmi, ollama::Ollama,
    openclaw::OpenClaw, opencode::Opencode, rocm_smi::RocmSmi,
};
use mac_mgmt_common::{AgentProvider, CloudConfig, GlobalConfig, LlmProvider, LmsConfig, OllamaConfig, OpenClawConfig, OpencodeConfig};

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

/// Build the list of managed services based on per-provider `enabled` flags.
pub fn build_services(
    _global: &GlobalConfig,
    openclaw_cfg: OpenClawConfig,
    opencode_cfg: OpencodeConfig,
    ollama_cfg: OllamaConfig,
    lms_cfg: LmsConfig,
) -> Vec<Box<dyn ManagedService>> {
    let mut services: Vec<Box<dyn ManagedService>> = Vec::new();

    if openclaw_cfg.enabled {
        tracing::info!("openclaw enabled");
        services.push(Box::new(OpenClaw::new(openclaw_cfg)));
    } else {
        tracing::info!("openclaw disabled");
    }

    if opencode_cfg.enabled {
        tracing::info!("opencode enabled");
        services.push(Box::new(Opencode::new(opencode_cfg)));
    } else {
        tracing::info!("opencode disabled");
    }

    if ollama_cfg.enabled {
        tracing::info!("ollama enabled");
        services.push(Box::new(Ollama::new(ollama_cfg)));
    } else {
        tracing::info!("ollama disabled");
    }

    if lms_cfg.enabled {
        tracing::info!("lms enabled");
        services.push(Box::new(Lms::new(lms_cfg)));
    } else {
        tracing::info!("lms disabled");
    }

    // Cloud providers don't need a local service.

    services.push(Box::new(McPorter));
    services.push(Box::new(Apprise));
    // GPU-tool installers. Both are install-only; each internally gates on
    // its vendor's PCI ID so GPU-less hosts don't pull the nix package.
    services.push(Box::new(NvidiaSmi));
    services.push(Box::new(RocmSmi));
    services
}

/// Build connectors that wire services together.
/// Connectors are run after all managed services have had their post_start.
/// Uses `default_llm` / `default_agent` to decide which LLM↔agent wiring
/// to apply, and the first enabled cloud entry when the default is `Cloud`.
pub fn build_connectors(
    global: &GlobalConfig,
    ollama_cfg: &OllamaConfig,
    lms_cfg: &LmsConfig,
    cloud_cfgs: &[CloudConfig],
) -> Vec<Box<dyn Connector>> {
    let mut connectors: Vec<Box<dyn Connector>> = Vec::new();

    // Relay→ollama connector: sets OLLAMA_ORIGINS for the tunnel proxy.
    if global.default_llm == LlmProvider::Ollama && ollama_cfg.enabled {
        connectors.push(Box::new(relay_ollama::RelayOllama));
    }

    match global.default_agent {
        AgentProvider::Openclaw => {
            connectors.push(Box::new(relay_openclaw::RelayOpenClaw));

            match global.default_llm {
                LlmProvider::Ollama if ollama_cfg.enabled => {
                    connectors.push(Box::new(ollama_openclaw::OllamaOpenClaw {
                        default_model: ollama_cfg.default_model.clone(),
                    }));
                }
                LlmProvider::Lms if lms_cfg.enabled => {
                    connectors.push(Box::new(lms_openclaw::LmsOpenClaw {
                        host: lms_cfg.host.clone(),
                        port: lms_cfg.port,
                        default_model: lms_cfg.default_model.clone(),
                    }));
                }
                LlmProvider::Cloud => {
                    if let Some(cloud_cfg) = cloud_cfgs.iter().find(|c| c.enabled) {
                        connectors.push(Box::new(cloud_openclaw::CloudOpenClaw {
                            config: cloud_cfg.clone(),
                        }));
                    }
                }
                _ => {}
            }
        }
        AgentProvider::Opencode => {
            connectors.push(Box::new(relay_opencode::RelayOpencode));

            match global.default_llm {
                LlmProvider::Ollama if ollama_cfg.enabled => {
                    connectors.push(Box::new(ollama_opencode::OllamaOpencode {
                        default_model: ollama_cfg.default_model.clone(),
                    }));
                }
                LlmProvider::Lms if lms_cfg.enabled => {
                    connectors.push(Box::new(lms_opencode::LmsOpencode {
                        host: lms_cfg.host.clone(),
                        port: lms_cfg.port,
                        default_model: lms_cfg.default_model.clone(),
                    }));
                }
                LlmProvider::Cloud => {
                    if let Some(cloud_cfg) = cloud_cfgs.iter().find(|c| c.enabled) {
                        connectors.push(Box::new(cloud_opencode::CloudOpencode {
                            config: cloud_cfg.clone(),
                        }));
                    }
                }
                _ => {}
            }
        }
        AgentProvider::None => {}
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
