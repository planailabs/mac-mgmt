pub mod cloud_openclaw;
pub mod cloud_opencode;
pub mod lms_openclaw;
pub mod lms_opencode;
pub mod ollama_openclaw;
pub mod ollama_opencode;
pub mod relay_ollama;
pub mod relay_openclaw;
pub mod relay_opencode;
pub mod relay_unsloth;
pub mod unsloth_openclaw;
pub mod unsloth_opencode;

use anyhow::Result;

use crate::managed_service::ManagedService;
use crate::services::{
    apprise::Apprise, lms::Lms, mcporter::McPorter, nvidia_smi::NvidiaSmi, ollama::Ollama,
    openclaw::OpenClaw, opencode::Opencode, rocm_smi::RocmSmi, unsloth::Unsloth,
};
use mac_mgmt_common::{
    AgentProvider, CloudConfig, GlobalConfig, LlmProvider, LmsConfig, OllamaConfig,
    OpenClawConfig, OpencodeConfig, UnslothConfig,
};

/// When a connector runs relative to service startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorPhase {
    /// Runs before services are spawned. Config-store deps only.
    PreStart,
    /// Runs after dependent services are healthy (post_start_done).
    PostStart,
}

/// A connector wires two services together.
///
/// Dependencies are either managed service names (must have post_start done)
/// or config provider names (must be set in the ConfigStore). When any
/// dependency changes, the connector is re-run.
pub trait Connector: Send + Sync {
    fn name(&self) -> &str;
    /// When this connector should run. Defaults to PostStart.
    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PostStart
    }
    /// Names of services and/or config providers this connector depends on.
    fn depends_on(&self) -> &[&str];
    /// Run the connector. `configs` contains the current values of all
    /// config provider dependencies.
    fn connect(&self, configs: &std::collections::HashMap<String, serde_json::Value>)
    -> Result<()>;
}

/// Build the list of managed services based on per-provider `enabled` flags.
pub fn build_services(
    _global: &GlobalConfig,
    openclaw_cfg: OpenClawConfig,
    opencode_cfg: OpencodeConfig,
    ollama_cfg: OllamaConfig,
    lms_cfg: LmsConfig,
    unsloth_cfg: UnslothConfig,
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

    if unsloth_cfg.enabled {
        tracing::info!("unsloth enabled");
        services.push(Box::new(Unsloth::new(unsloth_cfg)));
    } else {
        tracing::info!("unsloth disabled");
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
    unsloth_cfg: &UnslothConfig,
    cloud_cfgs: &[CloudConfig],
) -> Vec<Box<dyn Connector>> {
    let mut connectors: Vec<Box<dyn Connector>> = Vec::new();

    // Relay→ollama connector: sets OLLAMA_ORIGINS for the tunnel proxy.
    // Always enabled when ollama is enabled (regardless of default_llm).
    if ollama_cfg.enabled {
        connectors.push(Box::new(relay_ollama::RelayOllama));
    }

    // Relay→unsloth connector: wires the relay tunnel for Unsloth Studio.
    if unsloth_cfg.enabled {
        connectors.push(Box::new(relay_unsloth::RelayUnsloth));
    }

    tracing::info!(
        "building connectors: default_agent={}, default_llm={}",
        global.default_agent.as_str(),
        global.default_llm.as_str(),
    );

    let has_enabled_cloud = cloud_cfgs.iter().any(|c| c.enabled);

    // Register ALL enabled LLM providers with each enabled agent.
    // Only the provider matching default_llm gets set_default=true.
    match global.default_agent {
        AgentProvider::Openclaw => {
            connectors.push(Box::new(relay_openclaw::RelayOpenClaw));

            if ollama_cfg.enabled {
                connectors.push(Box::new(ollama_openclaw::OllamaOpenClaw {
                    host: ollama_cfg.host.clone(),
                    port: ollama_cfg.port,
                    default_model: ollama_cfg.default_model.clone(),
                    set_default: global.default_llm == LlmProvider::Ollama,
                }));
            }
            if lms_cfg.enabled {
                connectors.push(Box::new(lms_openclaw::LmsOpenClaw {
                    host: lms_cfg.host.clone(),
                    port: lms_cfg.port,
                    default_model: lms_cfg.default_model.clone(),
                    set_default: global.default_llm == LlmProvider::Lms,
                }));
            }
            if unsloth_cfg.enabled {
                connectors.push(Box::new(unsloth_openclaw::UnslothOpenClaw {
                    host: unsloth_cfg.host.clone(),
                    port: unsloth_cfg.port,
                    default_model: unsloth_cfg.default_model.clone(),
                    set_default: global.default_llm == LlmProvider::Unsloth,
                }));
            }
            if has_enabled_cloud {
                connectors.push(Box::new(cloud_openclaw::CloudOpenClaw {
                    set_default: global.default_llm == LlmProvider::Cloud,
                }));
            }
        }
        AgentProvider::Opencode => {
            connectors.push(Box::new(relay_opencode::RelayOpencode));

            if ollama_cfg.enabled {
                connectors.push(Box::new(ollama_opencode::OllamaOpencode {
                    default_model: ollama_cfg.default_model.clone(),
                    set_default: global.default_llm == LlmProvider::Ollama,
                }));
            }
            if lms_cfg.enabled {
                connectors.push(Box::new(lms_opencode::LmsOpencode {
                    host: lms_cfg.host.clone(),
                    port: lms_cfg.port,
                    default_model: lms_cfg.default_model.clone(),
                    set_default: global.default_llm == LlmProvider::Lms,
                }));
            }
            if unsloth_cfg.enabled {
                connectors.push(Box::new(unsloth_opencode::UnslothOpencode {
                    host: unsloth_cfg.host.clone(),
                    port: unsloth_cfg.port,
                    default_model: unsloth_cfg.default_model.clone(),
                    set_default: global.default_llm == LlmProvider::Unsloth,
                }));
            }
            if has_enabled_cloud {
                connectors.push(Box::new(cloud_opencode::CloudOpencode {
                    set_default: global.default_llm == LlmProvider::Cloud,
                }));
            }
        }
        AgentProvider::None => {
            tracing::debug!("default_agent=none, no agent connectors");
        }
    }

    tracing::info!(
        "built {} connector(s): [{}]",
        connectors.len(),
        connectors.iter().map(|c| c.name()).collect::<Vec<_>>().join(", ")
    );

    connectors
}

// ── Shared cloud-connector helpers ───────────────────────────────────

/// Return Some only if the string is non-empty.
pub(crate) fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|s| !s.is_empty())
}

/// Extract all enabled CloudConfigs from the configs map.
pub(crate) fn enabled_cloud_configs(
    configs: &std::collections::HashMap<String, serde_json::Value>,
) -> Vec<CloudConfig> {
    let Some(v) = configs.get("cloud") else {
        tracing::warn!("cloud config not found in config store");
        return Vec::new();
    };
    let all = match serde_json::from_value::<Vec<CloudConfig>>(v.clone()) {
        Ok(list) => list,
        Err(e) => {
            tracing::error!("failed to deserialize cloud configs: {e}");
            return Vec::new();
        }
    };
    let enabled: Vec<CloudConfig> = all.into_iter().filter(|c| c.enabled).collect();
    if enabled.is_empty() {
        tracing::debug!("no enabled cloud provider entries in config store");
    }
    enabled
}

/// Resolve the model for a provider, falling back to the provider's default
/// if the configured model doesn't match the provider prefix.
pub(crate) fn resolve_model(config: &CloudConfig) -> String {
    let provider = config.provider.as_str();
    if config.default_model.is_empty() || !config.default_model.starts_with(provider) {
        config.provider.default_model().to_string()
    } else {
        config.default_model.clone()
    }
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
