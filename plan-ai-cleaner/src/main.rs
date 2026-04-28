mod chunker;
mod ollama;
mod patterns;
mod server;
mod state;
mod storage;
mod types;

use anyhow::Result;
use clap::Parser;
use rmcp::ServiceExt;

use crate::server::CleanerServer;
use crate::state::SharedState;
use crate::storage::SessionStore;

#[derive(Parser)]
#[command(
    name = "plan-ai-cleaner",
    about = "MCP server for PII/secret detection and redaction"
)]
struct Cli {
    /// Ollama host for contextual NER.
    #[arg(long, env = "OLLAMA_HOST", default_value = "127.0.0.1")]
    ollama_host: String,

    /// Ollama port.
    #[arg(long, env = "OLLAMA_PORT", default_value = "11434")]
    ollama_port: u16,

    /// Ollama model for NER extraction.
    #[arg(long, env = "CLEANER_MODEL", default_value = "llama3.2:3b")]
    ollama_model: String,

    /// Session storage directory.
    #[arg(long, env = "CLEANER_DATA_DIR")]
    data_dir: Option<std::path::PathBuf>,

    /// Session TTL in seconds (default: 24 hours).
    #[arg(long, env = "CLEANER_TTL", default_value = "86400")]
    session_ttl: u64,
}

fn default_data_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".plan-ai")
        .join("cleaner")
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "plan_ai_cleaner=info,rmcp=warn".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    let data_dir = cli.data_dir.unwrap_or_else(default_data_dir);
    std::fs::create_dir_all(&data_dir)?;

    let store = SessionStore::new(&data_dir, cli.session_ttl)?;
    let state = SharedState::new(store);

    // GC expired sessions on startup.
    let removed = state.gc();
    if removed > 0 {
        tracing::info!("cleaned up {removed} expired session(s)");
    }

    let server = CleanerServer::new(
        state,
        cli.ollama_host,
        cli.ollama_port,
        cli.ollama_model,
    );

    tracing::info!("starting plan-ai-cleaner MCP server on stdio");

    let transport = rmcp::transport::io::stdio();
    let server_handle = server.serve(transport).await?;
    server_handle.waiting().await?;

    Ok(())
}
