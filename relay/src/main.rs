use anyhow::Result;
use axum::body::Body;
use axum::http::Request;
use axum::response::IntoResponse;
use axum::routing::any;
use axum::Router;
use axum_extra::extract::Host;
use clap::Parser;
use std::sync::Arc;
use tower::ServiceExt;
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
    tracing::info!(
        "relay starting, WS listen: {}, SSH port range: {}-{}",
        cfg.listen_addr, cfg.ssh_port_min, cfg.ssh_port_max
    );

    let data_dir = std::path::Path::new(&cfg.data_dir);
    let registry = Arc::new(daemon_registry::DaemonRegistry::new(
        cfg.ssh_port_min,
        cfg.ssh_port_max,
        cfg.max_daemons,
        data_dir,
    ));

    bridge::spawn_cleanup_task();

    let registry_cleanup = Arc::clone(&registry);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            registry_cleanup.expire_reservations();
        }
    });

    // API router: daemon WS, metrics, tunnels, health
    let api_router = ws_handler::router(
        Arc::clone(&registry),
        cfg.server_api_url.clone(),
        cfg.proxy_hostname.clone(),
    );

    // Proxy router (if proxy_hostname is configured)
    let proxy_router = cfg.proxy_hostname.as_ref().map(|proxy_hostname| {
        tracing::info!("proxy hostname configured: *.{proxy_hostname}");
        let proxy_state = proxy_handler::ProxyState {
            registry: Arc::clone(&registry),
            server_api_url: cfg.server_api_url.clone(),
            proxy_hostname: proxy_hostname.clone(),
        };
        (proxy_hostname.clone(), proxy_handler::router(proxy_state))
    });

    // Virtual-host dispatcher: route by hostname
    let app = Router::new()
        .fallback(any(move |Host(hostname): Host, req: Request<Body>| {
            let api = api_router.clone();
            let proxy = proxy_router.clone();
            async move {
                let host_no_port = hostname.split(':').next().unwrap_or(&hostname);

                // Check if this is a tunnel subdomain request
                if let Some((ref proxy_hostname, ref proxy_router)) = proxy {
                    if host_no_port.ends_with(&format!(".{proxy_hostname}")) {
                        return proxy_router.clone().oneshot(req).await.into_response();
                    }
                }

                // Default: relay API
                api.clone().oneshot(req).await.into_response()
            }
        }));

    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr).await?;
    tracing::info!("relay listening on {}", cfg.listen_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
