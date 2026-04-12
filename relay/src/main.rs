use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

mod bridge;
mod config;
mod daemon_registry;
mod metrics_federation;
mod proxy_handler;
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
        cfg.max_daemons,
    ));

    bridge::spawn_cleanup_task();

    // Periodically expire port reservations (every hour).
    let registry_cleanup = Arc::clone(&registry);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            registry_cleanup.expire_reservations();
        }
    });

    let mut app = ws_handler::router(
        Arc::clone(&registry),
        cfg.server_api_url.clone(),
        cfg.proxy_hostname.clone(),
    );

    // If proxy_hostname is configured, mount the browser proxy endpoints.
    if let Some(ref proxy_hostname) = cfg.proxy_hostname {
        tracing::info!("proxy hostname configured: *.{proxy_hostname}");
        let proxy_state = proxy_handler::ProxyState {
            registry: Arc::clone(&registry),
            server_api_url: cfg.server_api_url.clone(),
            proxy_hostname: proxy_hostname.clone(),
        };
        app = app.merge(proxy_handler::router(proxy_state));
    }

    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr).await?;
    tracing::info!("relay listening on {}", cfg.listen_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
