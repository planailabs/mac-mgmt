pub mod http_client;
pub mod server;
pub mod types;

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use rmcp::ServiceExt;

use crate::http_client::HttpClient;
use crate::server::MemvaultServer;

#[derive(Parser, Debug)]
#[command(name = "plan-ai-memvault", about = "MCP server for memvault p2p memory")]
pub struct Cli {
    /// Base URL of the daemon's memvault API.
    #[arg(long, env = "MEMVAULT_URL", default_value = "http://127.0.0.1:8401")]
    pub url: String,

    /// Path to the bearer token file.
    #[arg(long, env = "MEMVAULT_TOKEN_FILE")]
    pub token_file: Option<std::path::PathBuf>,

    /// Default tags applied when the agent omits them (comma-separated, scope:label format).
    #[arg(long, env = "MEMVAULT_DEFAULT_TAGS", value_delimiter = ',')]
    pub default_tags: Vec<String>,

    /// Default visibility when the agent omits it (internal, federated, public).
    #[arg(long, env = "MEMVAULT_DEFAULT_VISIBILITY", default_value = "internal")]
    pub default_visibility: String,
}

fn default_token_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("memvault")
        .join("api.token")
}

/// Run the memvault MCP server with the given CLI arguments.
pub async fn run(cli: Cli) -> Result<()> {
    let token_path = cli.token_file.unwrap_or_else(default_token_path);
    let token = std::fs::read_to_string(&token_path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|e| {
            tracing::warn!("could not read token from {}: {e}", token_path.display());
            String::new()
        });

    let client = Arc::new(HttpClient::new(&cli.url, &token)?);
    let server = MemvaultServer::new(client, cli.default_tags, cli.default_visibility);

    tracing::info!("starting plan-ai-memvault MCP server on stdio (api={})", cli.url);

    let transport = rmcp::transport::io::stdio();
    let server_handle = server.serve(transport).await?;
    server_handle.waiting().await?;

    Ok(())
}
