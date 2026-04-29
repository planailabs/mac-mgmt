use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::Request;
use axum::response::IntoResponse;
use axum::routing::any;
use axum_extra::extract::Host;
use clap::Parser;
use std::sync::Arc;
use tower::ServiceExt;
use tracing_subscriber::EnvFilter;

mod auth;
mod bridge;
mod config;
mod daemon_registry;
mod metrics_federation;
mod p2p;
mod proxy_handler;
mod ssh_listener;
mod ws_handler;

#[derive(Parser)]
#[command(
    name = "mac-mgmt-relay",
    version,
    about = "SSH relay for mac-mgmt daemons"
)]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "relay.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls ring crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cli = Cli::parse();
    let mut cfg = config::load(&cli.config)?;

    // Default proxy_url to http://{listen_addr} if not explicitly set.
    // This ensures the healer gets a working URL in dev/localhost setups.
    if cfg.proxy_url.is_none() {
        let addr = &cfg.listen_addr;
        let url = if addr.starts_with("0.0.0.0:") || addr.starts_with("[::]:") {
            format!("http://localhost:{}", addr.rsplit(':').next().unwrap_or("8080"))
        } else {
            format!("http://{addr}")
        };
        tracing::info!("proxy_url not configured, defaulting to {url}");
        cfg.proxy_url = Some(url);
    }
    tracing::info!(
        "relay starting, WS listen: {}, SSH port range: {}-{}",
        cfg.listen_addr,
        cfg.ssh_port_min,
        cfg.ssh_port_max
    );

    let data_dir = std::path::Path::new(&cfg.data_dir);
    let registry = Arc::new(daemon_registry::DaemonRegistry::new(
        cfg.ssh_port_min,
        cfg.ssh_port_max,
        cfg.max_daemons,
        data_dir,
    ));

    bridge::spawn_cleanup_task();

    // Start the libp2p swarm (circuit relay server + control protocol)
    let key_path = cfg.p2p_key_file.as_deref().unwrap_or("relay_ed25519_key");
    let key_path = if std::path::Path::new(key_path).is_absolute() {
        std::path::PathBuf::from(key_path)
    } else {
        data_dir.join(key_path)
    };
    let relay_swarm = Arc::new(
        p2p::RelaySwarm::start(cfg.p2p_port, &key_path, Arc::clone(&registry)).await?,
    );
    tracing::info!(peer_id = %relay_swarm.local_peer_id, "p2p relay swarm started");

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
        cfg.proxy_url.clone(),
    );

    // Proxy router (if proxy_hostname is configured)
    let proxy_router = cfg.proxy_hostname.as_ref().map(|proxy_hostname| {
        tracing::info!("proxy hostname configured: *.{proxy_hostname}");
        let proxy_state = proxy_handler::ProxyState {
            registry: Arc::clone(&registry),
            server_api_url: cfg.server_api_url.clone(),
            proxy_hostname: proxy_hostname.clone(),
            cors_origins: cfg.cors_origins.clone(),
            relay_swarm: Some(relay_swarm.clone()),
        };
        (proxy_hostname.clone(), proxy_handler::router(proxy_state))
    });

    // Virtual-host dispatcher: route by hostname
    let app = Router::new().fallback(any(move |Host(hostname): Host, req: Request<Body>| {
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

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
