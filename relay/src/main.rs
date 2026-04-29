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

mod api;
mod auth;
mod bridge;
mod config;
mod daemon_registry;
mod metrics_federation;
mod p2p;
mod proxy_handler;
mod ws_bridge;

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
        "relay starting, listen: {}",
        cfg.listen_addr,
    );

    let registry = Arc::new(daemon_registry::DaemonRegistry::new(cfg.max_daemons));

    // Start the libp2p swarm (circuit relay server + control protocol)
    let data_dir = std::path::Path::new(&cfg.data_dir);
    let key_path = cfg.p2p_key_file.as_deref().unwrap_or("relay_ed25519_key");
    let key_path = if std::path::Path::new(key_path).is_absolute() {
        std::path::PathBuf::from(key_path)
    } else {
        data_dir.join(key_path)
    };
    let relay_swarm = Arc::new(
        p2p::RelaySwarm::start(
            cfg.p2p_port,
            &key_path,
            Arc::clone(&registry),
            cfg.proxy_url.as_deref(),
        )
        .await?,
    );
    tracing::info!(peer_id = %relay_swarm.local_peer_id, "p2p relay swarm started");

    // API router: health, metrics, tunnel listing
    // Generate an in-memory token for the batch instances endpoint.
    let batch_token: String = {
        use rand::Rng;
        let bytes: [u8; 32] = rand::rng().random();
        hex::encode(bytes)
    };
    tracing::info!("batch endpoint token: {batch_token}");

    let api_router = api::router(
        Arc::clone(&registry),
        cfg.server_api_url.clone(),
        Arc::clone(&relay_swarm),
        batch_token,
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

    // Listen on two ports: the main HTTP port (axum) and a separate
    // internal listener that detects libp2p WS upgrades and bridges them.
    // We accept TCP connections manually, peek at the first bytes, and
    // either bridge to the libp2p WS listener (port p2p_port) or pass
    // to axum.
    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr).await?;
    tracing::info!("relay listening on {}", cfg.listen_addr);

    let p2p_port = cfg.p2p_port;

    // Spawn axum on a random internal port, then proxy non-WS requests to it.
    let axum_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let axum_port = axum_listener.local_addr()?.port();
    tracing::debug!("internal axum listener on 127.0.0.1:{axum_port}");

    tokio::spawn(async move {
        axum::serve(
            axum_listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("axum serve failed");
    });

    loop {
        let (tcp_stream, remote_addr) = listener.accept().await?;
        tokio::spawn(async move {
            let mut peek_buf = [0u8; 512];
            let n = match tcp_stream.peek(&mut peek_buf).await {
                Ok(n) if n > 0 => n,
                _ => return,
            };

            let is_ws_upgrade = {
                let peek_str = String::from_utf8_lossy(&peek_buf[..n]);
                let first_line = peek_str.lines().next().unwrap_or("");
                (first_line.starts_with("GET / ") || first_line.starts_with("GET /ws "))
                    && peek_str.to_lowercase().contains("upgrade: websocket")
            };

            if is_ws_upgrade {
                tracing::debug!(%remote_addr, "bridging WS to libp2p listener");
                let upstream = match tokio::net::TcpStream::connect(
                    format!("127.0.0.1:{p2p_port}"),
                )
                .await
                {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("failed to connect to libp2p WS: {e}");
                        return;
                    }
                };
                bridge::bridge_bidir(tcp_stream, upstream).await;
            } else {
                // Forward to internal axum listener.
                let upstream = match tokio::net::TcpStream::connect(
                    format!("127.0.0.1:{axum_port}"),
                )
                .await
                {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("failed to connect to internal axum: {e}");
                        return;
                    }
                };
                bridge::bridge_bidir(tcp_stream, upstream).await;
            }
        });
    }
}
