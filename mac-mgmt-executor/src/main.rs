mod backend;
mod incus_cli;
mod incus_common;
mod incus_https;
mod incus_unix;
mod server;
mod state;
mod types;

use std::sync::Arc;

use anyhow::{Result, bail};
use clap::Parser;
use rmcp::ServiceExt;

use crate::backend::IncusBackend;
use crate::incus_cli::CliBackend;
use crate::incus_https::HttpsBackend;
use crate::incus_unix::UnixBackend;
use crate::server::ExecutorServer;
use crate::state::SharedState;

#[derive(Parser)]
#[command(
    name = "mac-mgmt-executor",
    about = "MCP server providing ephemeral Incus containers for code execution"
)]
struct Cli {
    /// Incus project to use (defaults to Incus default project).
    #[arg(long, env = "INCUS_PROJECT")]
    project: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Logs go to stderr -- stdout is reserved for MCP JSON-RPC.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mac_mgmt_executor=info,rmcp=warn".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    let backend_type =
        std::env::var("INCUS_BACKEND").unwrap_or_else(|_| "unix".to_string());

    let backend: Arc<dyn IncusBackend> = match backend_type.as_str() {
        "unix" => {
            tracing::info!("using Unix socket backend");
            Arc::new(UnixBackend::new(cli.project.clone()))
        }
        "cli" => {
            tracing::info!("using CLI backend");
            Arc::new(CliBackend::new(cli.project.clone()))
        }
        "https" => {
            tracing::info!("using HTTPS backend");
            Arc::new(
                HttpsBackend::from_env(cli.project.clone())
                    .map_err(|e| anyhow::anyhow!("failed to initialize HTTPS backend: {e}"))?,
            )
        }
        other => bail!("unknown INCUS_BACKEND value: '{other}' (expected 'unix', 'cli', or 'https')"),
    };

    let state = SharedState::new(backend);
    let server = ExecutorServer::new(state.clone());

    // Register cleanup on shutdown signal.
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("received shutdown signal, cleaning up...");
        cleanup_all(&cleanup_state).await;
        std::process::exit(0);
    });

    tracing::info!("starting MCP server on stdio");

    let transport = rmcp::transport::io::stdio();
    let server_handle = server.serve(transport).await?;

    // Wait for the MCP transport to close.
    server_handle.waiting().await?;

    // Cleanup on normal exit.
    cleanup_all(&state).await;

    Ok(())
}

async fn cleanup_all(state: &SharedState) {
    let names = state.take_all_instance_names().await;
    if names.is_empty() {
        return;
    }
    tracing::info!("cleaning up {} container(s)...", names.len());
    for name in &names {
        if let Err(e) = state.backend.delete(name).await {
            tracing::warn!("cleanup failed for {name}: {e}");
        } else {
            tracing::info!("deleted {name}");
        }
    }
}
