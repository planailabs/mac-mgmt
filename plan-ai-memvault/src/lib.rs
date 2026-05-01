pub mod server;
pub mod types;

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use rmcp::ServiceExt;
use tokio::sync::RwLock;

use memvault_query::{QuotaManager, TextIndex};
use memvault_store::MemvaultStore;

use crate::server::MemvaultServer;

#[derive(Parser, Debug)]
#[command(name = "plan-ai-memvault", about = "MCP server for memvault p2p memory")]
pub struct Cli {
    /// Memvault data directory.
    #[arg(long, env = "MEMVAULT_DATA_DIR")]
    pub data_dir: Option<std::path::PathBuf>,
}

fn default_data_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".plan-ai")
        .join("memvault")
}

/// Run the memvault MCP server with the given CLI arguments.
pub async fn run(cli: Cli) -> Result<()> {
    let data_dir = cli.data_dir.unwrap_or_else(default_data_dir);
    std::fs::create_dir_all(&data_dir)?;

    let db_path = data_dir.join("memvault.redb");
    let store = Arc::new(MemvaultStore::open(&db_path)?);
    let index = Arc::new(RwLock::new(TextIndex::new()));
    let quotas = Arc::new(RwLock::new(QuotaManager::new(Default::default())));
    let event_bus = Arc::new(memvault_api::EventBus::new(256));

    let peer_id = vec![0u8; 32]; // Local-only placeholder peer ID
    let cluster_id = vec![0u8; 32]; // Local-only placeholder cluster ID

    let client = memvault_api::LocalClient::new(
        store,
        index,
        quotas,
        event_bus,
        peer_id,
        cluster_id,
    );

    let server = MemvaultServer::new(Arc::new(client));

    tracing::info!("starting plan-ai-memvault MCP server on stdio");

    let transport = rmcp::transport::io::stdio();
    let server_handle = server.serve(transport).await?;
    server_handle.waiting().await?;

    Ok(())
}
