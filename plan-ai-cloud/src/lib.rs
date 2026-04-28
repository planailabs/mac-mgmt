pub mod audit;
pub mod backend;
pub mod rehydrate;
pub mod server;
pub mod state;
pub mod types;

use anyhow::Result;
use clap::Parser;
use rmcp::ServiceExt;

use crate::server::CloudServer;
use crate::state::SharedState;

#[derive(Parser, Debug)]
#[command(
    name = "plan-ai-cloud",
    about = "MCP server for querying cloud LLMs via LiteLLM with cleaner integration"
)]
pub struct Cli {
    /// LiteLLM proxy URL.
    #[arg(long, env = "LITELLM_URL", default_value = "http://127.0.0.1:4100")]
    pub litellm_url: String,

    /// LiteLLM master API key.
    #[arg(long, env = "LITELLM_API_KEY")]
    pub litellm_key: Option<String>,

    /// Ollama host for local model fallback.
    #[arg(long, env = "OLLAMA_HOST", default_value = "127.0.0.1")]
    pub ollama_host: String,

    /// Ollama port.
    #[arg(long, env = "OLLAMA_PORT", default_value = "11434")]
    pub ollama_port: u16,

    /// Path to cleaner session storage (for rehydration).
    #[arg(long, env = "CLEANER_DATA_DIR")]
    pub cleaner_dir: Option<std::path::PathBuf>,

    /// Audit log directory.
    #[arg(long, env = "CLOUD_AUDIT_DIR")]
    pub audit_dir: Option<std::path::PathBuf>,

    /// Refuse to send text without a cleaner session (strict mode).
    #[arg(long, env = "CLOUD_STRICT")]
    pub strict: bool,
}

fn default_cleaner_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".plan-ai")
        .join("cleaner")
}

fn default_audit_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".plan-ai")
        .join("cloud")
        .join("audit")
}

/// Run the cloud MCP server with the given CLI arguments.
pub async fn run(cli: Cli) -> Result<()> {
    let cleaner_dir = cli.cleaner_dir.unwrap_or_else(default_cleaner_dir);
    let audit_dir = cli.audit_dir.unwrap_or_else(default_audit_dir);
    std::fs::create_dir_all(&audit_dir)?;

    let state = SharedState::new(
        cli.litellm_url,
        cli.litellm_key,
        cli.ollama_host,
        cli.ollama_port,
        cleaner_dir,
        audit_dir,
        cli.strict,
    );

    let server = CloudServer::new(state);

    tracing::info!("starting plan-ai-cloud MCP server on stdio");

    let transport = rmcp::transport::io::stdio();
    let server_handle = server.serve(transport).await?;
    server_handle.waiting().await?;

    Ok(())
}
