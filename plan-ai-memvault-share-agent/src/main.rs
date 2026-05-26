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

    /// Path to bearer token file.
    #[arg(long, env = "MEMVAULT_TOKEN_FILE")]
    token_file: Option<std::path::PathBuf>,
}

fn default_token_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("memvault")
        .join("api.token")
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    let token_path = cli.token_file.unwrap_or_else(default_token_path);
    let token = std::fs::read_to_string(&token_path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|e| {
            tracing::warn!("could not read token from {}: {e}", token_path.display());
            String::new()
        });

    let client: Arc<dyn memvault_api::MemvaultClient> =
        Arc::new(memvault_api::HttpApiClient::new(&cli.url, &token)?);

    let server = ShareAgentServer::new(client);
    let transport = rmcp::transport::io::stdio();
    let handle = server.serve(transport).await?;
    handle.waiting().await?;

    Ok(())
}
