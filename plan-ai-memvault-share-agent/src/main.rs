use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use rmcp::ServiceExt;

use plan_ai_memvault_share_agent::ShareAgentServer;

#[derive(Parser, Debug)]
#[command(
    name = "plan-ai-memvault-share-agent",
    about = "MCP server for cross-cluster share proposal review"
)]
struct Cli {
    /// Memvault HTTP API URL.
    #[arg(long, env = "MEMVAULT_URL", default_value = "http://127.0.0.1:8401")]
    url: String,

    /// Path to the agent identity directory (defaults to the daemon's
    /// built-in `_ui` agent for localhost access).
    #[arg(long, env = "MEMVAULT_IDENTITY_DIR")]
    identity_dir: Option<std::path::PathBuf>,
}

fn default_identity_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("memvault")
        .join("identity")
        .join("ui_agent")
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    let identity_dir = cli.identity_dir.unwrap_or_else(default_identity_dir);
    let identity = memvault_api::agent_identity::AgentIdentity::load(&identity_dir)
        .map_err(|e| anyhow::anyhow!("load agent identity from {}: {e}", identity_dir.display()))?;

    let client: Arc<dyn memvault_api::MemvaultClient> = Arc::new(
        memvault_api::HttpApiClient::new(&cli.url, Some(Arc::new(identity)))?,
    );

    let server = ShareAgentServer::new(client);
    let transport = rmcp::transport::io::stdio();
    let handle = server.serve(transport).await?;
    handle.waiting().await?;

    Ok(())
}
