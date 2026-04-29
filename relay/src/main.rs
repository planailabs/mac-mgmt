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

    let p2p_port = cfg.p2p_port;

    // Virtual-host dispatcher with WS-aware routing.
    // WS upgrades on the main host → libp2p bridge (daemon p2p).
    // WS upgrades on proxy subdomains → axum proxy handler via oneshot.
    // Non-WS requests → axum via oneshot (API or proxy).
    let app = Router::new().fallback(any(move |
        Host(hostname): Host,
        req: Request<Body>,
    | {
        let api = api_router.clone();
        let proxy = proxy_router.clone();
        async move {
            let host_no_port: String = hostname.split(':').next().unwrap_or(&hostname).to_string();

            // Proxy subdomain.
            if let Some((ref proxy_hostname, ref proxy_router)) = proxy {
                if host_no_port.ends_with(&format!(".{proxy_hostname}")) {
                    let is_proxy_ws = req.headers().get("upgrade")
                        .and_then(|v| v.to_str().ok())
                        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));

                    if is_proxy_ws {
                        use axum::extract::FromRequestParts;
                        let (mut parts, _body) = req.into_parts();
                        if let Ok(ws) = axum::extract::ws::WebSocketUpgrade::from_request_parts(&mut parts, &()).await {
                            let prefix = host_no_port
                                .strip_suffix(&format!(".{proxy_hostname}"))
                                .unwrap_or("")
                                .to_string();
                            let relay_swarm_clone = relay_swarm.clone();
                            return ws.on_upgrade(move |socket| async move {
                                let (instance_prefix, tunnel_name) = prefix
                                    .rsplit_once('-')
                                    .unwrap_or((&prefix, ""));
                                if let Some(peer_id) = relay_swarm_clone.registry_resolve_peer_id(instance_prefix) {
                                    if let Ok(tunnel) = relay_swarm_clone.open_tunnel_stream(peer_id).await {
                                        let handshake = serde_json::json!({
                                            "type": "proxy",
                                            "tunnel_name": tunnel_name,
                                            "method": "WEBSOCKET",
                                            "path": "/",
                                            "headers": [],
                                        });
                                        let data = serde_json::to_vec(&handshake).unwrap_or_default();
                                        use futures_util::AsyncWriteExt;
                                        let mut tunnel = tunnel;
                                        let _ = tunnel.write_all(&(data.len() as u32).to_be_bytes()).await;
                                        let _ = tunnel.write_all(&data).await;
                                        let _ = tunnel.flush().await;
                                        crate::ws_bridge::bridge_ws_to_stream(socket, tunnel).await;
                                    }
                                }
                            });
                        }
                        // WS upgrade extraction failed — fall through to 400.
                        return axum::http::StatusCode::BAD_REQUEST.into_response();
                    }

                    return proxy_router.clone().oneshot(req).await.into_response();
                }
            }

            // Main host: check if this is a WS upgrade → bridge to libp2p.
            let is_ws = req.headers().get("upgrade")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));

            if is_ws {
                use axum::extract::FromRequestParts;
                let (mut parts, _body) = req.into_parts();
                match axum::extract::ws::WebSocketUpgrade::from_request_parts(&mut parts, &()).await {
                    Ok(ws) => {
                        return ws.on_upgrade(move |socket| async move {
                            tracing::debug!("p2p WS bridge started");
                            crate::ws_bridge::bridge_ws_to_libp2p_listener(socket, p2p_port).await;
                            tracing::debug!("p2p WS bridge closed");
                        });
                    }
                    Err(e) => {
                        tracing::warn!("WS upgrade failed: {e}");
                        return axum::http::StatusCode::BAD_REQUEST.into_response();
                    }
                }
            }

            // Main host non-WS → API router.
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
