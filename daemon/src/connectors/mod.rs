pub mod backup;
pub mod cloud_openclaw;
pub mod cloud_opencode;
pub mod litellm_openclaw;
pub mod litellm_opencode;
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
    ai_proxy_svc::AiProxyService, apprise::Apprise, litellm::Litellm, lms::Lms,
    mcporter::McPorter, nvidia_smi::NvidiaSmi, ollama::Ollama, openclaw::OpenClaw,
    opencode::Opencode, restic::Restic, rocm_smi::RocmSmi, unsloth::Unsloth,
};
use mac_mgmt_common::{
    AgentProvider, AiProxyConfig, BackupConfig, CloudConfig, GlobalConfig, LitellmConfig,
    LlmProvider, LmsConfig, OllamaConfig, OpenClawConfig, OpencodeConfig, UnslothConfig,
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
    /// Return environment variables to inject into the given service's SpawnSpec.
    /// Called after `connect()`, results are merged into the SpawnSpec before spawn.
    fn service_env(
        &self,
        _service_name: &str,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, String> {
        Default::default()
    }
}

/// Build the list of managed services based on per-provider `enabled` flags.
pub fn build_services(
    _global: &GlobalConfig,
    openclaw_cfg: OpenClawConfig,
    opencode_cfg: OpencodeConfig,
    ollama_cfg: OllamaConfig,
    lms_cfg: LmsConfig,
    unsloth_cfg: UnslothConfig,
    litellm_cfg: LitellmConfig,
    cloud_cfgs: Vec<CloudConfig>,
    backup_cfg: BackupConfig,
    ai_proxy_cfg: &AiProxyConfig,
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

    if litellm_cfg.enabled {
        tracing::info!("litellm enabled");
        services.push(Box::new(Litellm::new(litellm_cfg, cloud_cfgs)));
    } else {
        tracing::info!("litellm disabled");
    }

    services.push(Box::new(McPorter));
    services.push(Box::new(Apprise));
    // GPU-tool installers. Both are install-only; each internally gates on
    // its vendor's PCI ID so GPU-less hosts don't pull the nix package.
    services.push(Box::new(NvidiaSmi));
    services.push(Box::new(RocmSmi));

    if backup_cfg.enabled {
        tracing::info!("backup enabled (restic)");
        services.push(Box::new(Restic::new(backup_cfg)));
    } else {
        tracing::info!("backup disabled");
    }

    if ai_proxy_cfg.enabled {
        tracing::info!("ai-proxy enabled (integrated)");
        services.push(Box::new(AiProxyService::new(ai_proxy_cfg)));
    } else {
        tracing::info!("ai-proxy disabled");
    }

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
    litellm_cfg: &LitellmConfig,
    cloud_cfgs: &[CloudConfig],
    backup_cfg: &BackupConfig,
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
    // When litellm is enabled, it replaces direct cloud→agent connectors.
    let use_litellm = litellm_cfg.enabled;

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
            if use_litellm {
                connectors.push(Box::new(litellm_openclaw::LitellmOpenClaw {
                    host: litellm_cfg.host.clone(),
                    port: litellm_cfg.port,
                    set_default: global.default_llm == LlmProvider::Cloud
                        || global.default_llm == LlmProvider::Litellm,
                }));
            } else if has_enabled_cloud {
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
            if use_litellm {
                connectors.push(Box::new(litellm_opencode::LitellmOpencode {
                    host: litellm_cfg.host.clone(),
                    port: litellm_cfg.port,
                    set_default: global.default_llm == LlmProvider::Cloud
                        || global.default_llm == LlmProvider::Litellm,
                }));
            } else if has_enabled_cloud {
                connectors.push(Box::new(cloud_opencode::CloudOpencode {
                    set_default: global.default_llm == LlmProvider::Cloud,
                }));
            }
        }
        AgentProvider::None => {
            tracing::debug!("default_agent=none, no agent connectors");
        }
    }

    if backup_cfg.enabled {
        connectors.push(Box::new(backup::BackupConnector));
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

/// Return Some only if the secret is present and non-empty.
pub(crate) fn non_empty_secret(s: &Option<mac_mgmt_common::Secret>) -> Option<&str> {
    s.as_ref().map(|s| s.expose()).filter(|s| !s.is_empty())
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

// Re-export for backwards compatibility with connectors that import from here.
pub use crate::validator::merge_json;
