use crate::managed_service::ManagedService;
use crate::services::{mcporter::McPorter, nexa::Nexa, ollama::Ollama, openclaw::OpenClaw};
use mac_mgmt_common::{GlobalConfig, NexaConfig, OllamaConfig, OpenClawConfig};

/// Build the list of managed services based on global provider settings.
///
/// `llm_provider` selects which LLM backend to manage (ollama, nexa, or none).
/// `agent_provider` selects which agent to manage (openclaw or none).
/// McPorter is always included as an install-only service.
pub fn build_services(
    global: &GlobalConfig,
    openclaw_cfg: OpenClawConfig,
    ollama_cfg: OllamaConfig,
    nexa_cfg: NexaConfig,
) -> Vec<Box<dyn ManagedService>> {
    let mut services: Vec<Box<dyn ManagedService>> = Vec::new();

    // Agent provider
    match global.agent_provider.as_str() {
        "openclaw" => {
            tracing::info!("agent_provider=openclaw");
            services.push(Box::new(OpenClaw::new(openclaw_cfg)));
        }
        "none" => {
            tracing::info!("agent_provider=none, skipping agent services");
        }
        other => {
            tracing::warn!("unknown agent_provider '{other}', skipping");
        }
    }

    // LLM provider
    match global.llm_provider.as_str() {
        "ollama" => {
            tracing::info!("llm_provider=ollama");
            services.push(Box::new(Ollama::new(ollama_cfg)));
        }
        "nexa" => {
            tracing::info!("llm_provider=nexa");
            services.push(Box::new(Nexa::new(nexa_cfg)));
        }
        "none" => {
            tracing::info!("llm_provider=none, skipping LLM services");
        }
        other => {
            tracing::warn!("unknown llm_provider '{other}', skipping");
        }
    }

    // Always include McPorter (install-only)
    services.push(Box::new(McPorter));

    services
}
