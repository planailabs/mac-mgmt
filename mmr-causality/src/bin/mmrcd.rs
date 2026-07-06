//! mmrcd — the mmrc orchestration daemon. Spins up antithesis clusters as
//! incus instances per run and exposes a token-authed HTTP API.

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use mmr_causality::http;
use mmr_causality::orchestrator::{MmrcdConfig, Orchestrator};

#[derive(Parser)]
#[command(name = "mmrcd", about = "mmrc incus orchestration daemon")]
struct Cli {
    /// Config path (default: ~/.config/mmrcd/config.toml).
    #[arg(long)]
    config: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mmrcd=info,mmr_causality=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();
    let cfg = MmrcdConfig::load(cli.config.as_deref())?;
    let orch = Arc::new(Orchestrator::new(cfg));
    http::serve(orch).await
}
