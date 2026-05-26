pub mod backend;
pub mod http_client;
pub mod local_backend;
pub mod server;
pub mod types;
pub mod vfs;

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use rmcp::ServiceExt;

use crate::http_client::HttpClient;
use crate::local_backend::LocalBackend;
use crate::server::MemvaultServer;

#[derive(Parser, Debug)]
#[command(name = "plan-ai-memvault", about = "MCP server for memvault p2p memory")]
pub struct Cli {
    /// Base URL of the daemon's memvault API (used when --db is not set).
    #[arg(long, env = "MEMVAULT_URL", default_value = "http://127.0.0.1:8401")]
    pub url: String,

    /// Path to the bearer token file (HTTP mode only).
    #[arg(long, env = "MEMVAULT_TOKEN_FILE")]
    pub token_file: Option<std::path::PathBuf>,

    /// Path to the redb database file for direct local access (bypasses HTTP).
    #[arg(long, env = "MEMVAULT_DB")]
    pub db: Option<std::path::PathBuf>,

    /// Cluster ID (hex) when using --db. Defaults to all zeros.
    #[arg(long, env = "MEMVAULT_CLUSTER_ID")]
    pub cluster_id: Option<String>,

    /// Default tags applied when the agent omits them (comma-separated, scope:label format).
    #[arg(long, env = "MEMVAULT_DEFAULT_TAGS", value_delimiter = ',')]
    pub default_tags: Vec<String>,

    /// Default visibility when the agent omits it (internal, federated, public).
    #[arg(long, env = "MEMVAULT_DEFAULT_VISIBILITY", default_value = "internal")]
    pub default_visibility: String,

    /// Agent identifier for authenticated operations.
    #[arg(long, env = "MEMVAULT_AGENT_ID")]
    pub agent_id: Option<String>,

    /// Path to the agent identity directory (contains private_key.pem, attestation.cbor, etc.).
    #[arg(long, env = "MEMVAULT_IDENTITY_DIR")]
    pub identity_dir: Option<std::path::PathBuf>,
}

fn default_token_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("memvault")
        .join("api.token")
}

/// Run the memvault MCP server with the given CLI arguments.
pub async fn run(cli: Cli) -> Result<()> {
    // Load agent identity if configured
    if let Some(ref agent_id) = cli.agent_id {
        let identity_dir = cli.identity_dir.clone().unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("memvault")
                .join("agents")
                .join(agent_id)
        });

        if memvault_api::agent_identity::AgentIdentity::exists(&identity_dir) {
            match memvault_api::agent_identity::AgentIdentity::load(&identity_dir) {
                Ok(id) => {
                    tracing::info!(
                        agent_id = %id.agent_id.0,
                        cluster = %hex::encode(id.cluster_id.0),
                        "loaded agent identity"
                    );
                }
                Err(e) => {
                    tracing::warn!("failed to load agent identity from {}: {e}", identity_dir.display());
                }
            }
        } else {
            tracing::warn!(
                "agent identity not found at {} — running without agent auth",
                identity_dir.display()
            );
        }
    }

    let backend: Arc<dyn crate::backend::Backend> = if let Some(ref db_path) = cli.db {
        // Local mode: direct redb access.
        let cluster_id = if let Some(ref hex_str) = cli.cluster_id {
            hex::decode(hex_str.trim())?
        } else {
            vec![0u8; 32]
        };
        tracing::info!("starting in local mode (db={})", db_path.display());
        Arc::new(LocalBackend::open(db_path, cluster_id).await?)
    } else {
        // HTTP mode: talk to a running daemon.
        let token_path = cli.token_file.unwrap_or_else(default_token_path);
        let token = std::fs::read_to_string(&token_path)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|e| {
                tracing::warn!("could not read token from {}: {e}", token_path.display());
                String::new()
            });
        tracing::info!("starting in HTTP mode (api={})", cli.url);
        Arc::new(HttpClient::new(&cli.url, &token)?)
    };

    let server = MemvaultServer::new(backend, cli.default_tags, cli.default_visibility);

    let transport = rmcp::transport::io::stdio();
    let server_handle = server.serve(transport).await?;
    server_handle.waiting().await?;

    Ok(())
}
