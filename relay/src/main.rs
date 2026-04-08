use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

mod bridge;
mod config;
mod daemon_registry;
mod metrics_federation;
mod ssh_listener;
mod ws_handler;

#[derive(Parser)]
#[command(name = "mac-mgmt-relay", version, about = "SSH relay for mac-mgmt daemons")]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "relay.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cli = Cli::parse();
    let cfg = config::load(&cli.config)?;
    tracing::info!("relay starting, WS listen: {}, SSH port range: {}-{}", cfg.listen_addr, cfg.ssh_port_min, cfg.ssh_port_max);

    let registry = Arc::new(daemon_registry::DaemonRegistry::new(
        cfg.ssh_port_min,
        cfg.ssh_port_max,
    ));

    let app = ws_handler::router(
        Arc::clone(&registry),
        cfg.server_api_url.clone(),
    );

    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr).await?;
    tracing::info!("relay listening on {}", cfg.listen_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
