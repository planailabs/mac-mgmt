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

mod api;
mod auth;
mod config;
mod daemon_registry;
mod metrics_federation;
mod mtls;
mod p2p;
mod proxy_handler;
mod ssh_bridge;
mod ssh_identity;
mod tunnel_io;
mod web_ssh;
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

    // Traces, metrics and logs over OTLP when OTEL_EXPORTER_OTLP_ENDPOINT is
    // set; a plain stdout subscriber otherwise. Metrics are collected either
    // way — /metrics renders them alongside the federated daemon series.
    mac_mgmt_common::otel::init("mac-mgmt-relay", "info");

    let cli = Cli::parse();
    let mut cfg = config::load(&cli.config)?;

    // Default proxy_url to https://{listen_addr} if not explicitly set.
    // This ensures the healer gets a working URL in dev/localhost setups.
    if cfg.proxy_url.is_none() {
        let addr = &cfg.listen_addr;
        let url = if addr.starts_with("0.0.0.0:") || addr.starts_with("[::]:") {
            format!(
                "https://localhost:{}",
                addr.rsplit(':').next().unwrap_or("8080")
            )
        } else {
            format!("https://{addr}")
        };
        tracing::info!("proxy_url not configured, defaulting to {url}");
        cfg.proxy_url = Some(url);
    }
    tracing::info!("relay starting, listen: {}", cfg.listen_addr,);

    // Generate ephemeral SSH identity (rotated on every restart).
    let ssh_identity = Arc::new(ssh_identity::RelaySshIdentity::generate());

    let data_dir = std::path::Path::new(&cfg.data_dir);
    let registry = Arc::new(daemon_registry::DaemonRegistry::new(
        cfg.max_daemons,
        cfg.ssh_port_min,
        cfg.ssh_port_max,
        data_dir,
    ));

    // Start the libp2p swarm (circuit relay server + control protocol)
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
            &cfg.server_api_url,
            Arc::clone(&ssh_identity),
        )
        .await?,
    );
    tracing::info!(peer_id = %relay_swarm.local_peer_id, "p2p relay swarm started");

    // SSH TCP bridge: per-daemon port listeners
    let ssh_bridge = Arc::new(ssh_bridge::SshBridge::new(
        Arc::clone(&relay_swarm),
        Arc::clone(&registry),
    ));
    relay_swarm.set_ssh_bridge(Arc::clone(&ssh_bridge));

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
        cfg.proxy_url.clone(),
    );

    // Web SSH terminal routes (/ssh/{instance_id} and /ssh/{instance_id}/ws).
    let web_ssh_router = web_ssh::router(web_ssh::WebSshState {
        registry: Arc::clone(&registry),
        relay_swarm: Arc::clone(&relay_swarm),
        ssh_identity: Arc::clone(&ssh_identity),
        server_api_url: cfg.server_api_url.clone(),
    });
    let api_router = api_router.merge(web_ssh_router);

    // Try to fetch the server's external web URL eagerly. If it fails, the
    // OnceCell stays empty and will be lazily initialised on the first
    // unauthenticated proxy request.
    let server_web_url = Arc::new(tokio::sync::OnceCell::new());
    match proxy_handler::fetch_server_web_url(&cfg.server_api_url).await {
        Ok(url) => {
            tracing::info!("server web URL: {url}");
            let _ = server_web_url.set(url);
        }
        Err(e) => {
            tracing::warn!("failed to fetch server web URL: {e} — will retry on first auth page");
        }
    }

    // Proxy router (if proxy_hostname is configured)
    let proxy_router = cfg.proxy_hostname.as_ref().map(|proxy_hostname| {
        tracing::info!("proxy hostname configured: *.{proxy_hostname}");
        let proxy_state = proxy_handler::ProxyState {
            registry: Arc::clone(&registry),
            server_api_url: cfg.server_api_url.clone(),
            proxy_hostname: proxy_hostname.clone(),
            cors_origins: cfg.cors_origins.clone(),
            relay_swarm: Some(relay_swarm.clone()),
            server_web_url: Arc::clone(&server_web_url),
            relay_url: cfg.proxy_url.clone(),
        };
        (proxy_hostname.clone(), proxy_handler::router(proxy_state))
    });

    let p2p_port = cfg.p2p_port;

    // Virtual-host dispatcher with WS-aware routing.
    // WS upgrades on the main host → libp2p bridge (daemon p2p).
    // WS upgrades on proxy subdomains → axum proxy handler via oneshot.
    // Non-WS requests → axum via oneshot (API or proxy).
    let app = Router::new().fallback(any(move |Host(hostname): Host, mut req: Request<Body>| {
        let api = api_router.clone();
        let proxy = proxy_router.clone();
        async move {
            // Ensure the `host` header is set for HTTP/2 compatibility.
            // HTTP/2 uses the `:authority` pseudo-header instead of `Host`,
            // but downstream handlers (proxy_handler::parse_subdomain etc.)
            // read `headers.get("host")`. Inject it if missing.
            if !req.headers().contains_key("host") {
                if let Ok(val) = axum::http::HeaderValue::from_str(&hostname) {
                    req.headers_mut().insert("host", val);
                }
            }

            let host_no_port: String = hostname.split(':').next().unwrap_or(&hostname).to_string();

            // HTTP/2 connection coalescing guard: if the Host's class (main
            // domain vs proxy subdomain) doesn't match the TLS config class
            // the connection was accepted with, answer 421 so the browser
            // retries on a fresh connection with the right SNI. Otherwise
            // the per-SNI client-cert behavior could be bypassed.
            let host_is_proxy = proxy
                .as_ref()
                .is_some_and(|(ph, _)| host_no_port.ends_with(&format!(".{ph}")));
            if let Some(class) = req.extensions().get::<mtls::TlsSniClass>() {
                if class.is_proxy_subdomain != host_is_proxy {
                    return axum::http::StatusCode::MISDIRECTED_REQUEST.into_response();
                }
            }

            // Proxy subdomain.
            if let Some((ref proxy_hostname, ref proxy_router)) = proxy {
                if host_no_port.ends_with(&format!(".{proxy_hostname}")) {
                    let is_proxy_ws = req
                        .headers()
                        .get("upgrade")
                        .and_then(|v| v.to_str().ok())
                        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));

                    if is_proxy_ws {
                        use axum::extract::FromRequestParts;
                        let (mut parts, _body) = req.into_parts();
                        if let Ok(ws) =
                            axum::extract::ws::WebSocketUpgrade::from_request_parts(&mut parts, &())
                                .await
                        {
                            let prefix = host_no_port
                                .strip_suffix(&format!(".{proxy_hostname}"))
                                .unwrap_or("")
                                .to_string();
                            let relay_swarm_clone = relay_swarm.clone();
                            return ws.on_upgrade(move |socket| async move {
                                let (instance_prefix, tunnel_name) =
                                    prefix.rsplit_once('-').unwrap_or((&prefix, ""));
                                if let Some(peer_id) =
                                    relay_swarm_clone.registry_resolve_peer_id(instance_prefix)
                                {
                                    if let Ok(tunnel) =
                                        relay_swarm_clone.open_tunnel_stream(peer_id).await
                                    {
                                        let handshake = serde_json::json!({
                                            "type": "proxy",
                                            "tunnel_name": tunnel_name,
                                            "method": "WEBSOCKET",
                                            "path": "/",
                                            "headers": [],
                                        });
                                        let data =
                                            serde_json::to_vec(&handshake).unwrap_or_default();
                                        use futures_util::AsyncWriteExt;
                                        let mut tunnel = tunnel;
                                        let _ = tunnel
                                            .write_all(&(data.len() as u32).to_be_bytes())
                                            .await;
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
            // Only intercept WS connections that are NOT for named routes
            // (e.g. /ssh/* is handled by the API router's WebSocket handler).
            let is_ws = req
                .headers()
                .get("upgrade")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
            let is_p2p_ws = is_ws && !req.uri().path().starts_with("/ssh/");

            if is_p2p_ws {
                use axum::extract::FromRequestParts;
                let (mut parts, _body) = req.into_parts();
                match axum::extract::ws::WebSocketUpgrade::from_request_parts(&mut parts, &()).await
                {
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

    // Build TLS config: load from files or generate self-signed in debug mode.
    let (cert_chain, private_key) = match (&cfg.tls_cert_path, &cfg.tls_key_path) {
        (Some(cert_path), Some(key_path)) => {
            tracing::info!("loading TLS cert from {cert_path}");
            mtls::load_certs_and_key(cert_path, key_path)?
        }
        _ => {
            if cfg!(debug_assertions) {
                tracing::warn!(
                    "TLS cert/key not configured — generating self-signed certificate \
                         (development only)"
                );
                mtls::generate_self_signed()?
            } else {
                anyhow::bail!("tls_cert_path and tls_key_path are required in release builds");
            }
        }
    };

    let (tls_main, tls_quiet) = mtls::build_tls_configs(cert_chain, private_key)
        .map_err(|e| anyhow::anyhow!("failed to build TLS config: {e}"))?;

    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr).await?;
    tracing::info!("relay listening on {} (HTTPS)", cfg.listen_addr);

    let served = mtls::serve_tls(
        listener,
        tls_main,
        tls_quiet,
        cfg.proxy_hostname.clone(),
        app,
    )
    .await;

    // The batch processors buffer, so the last batch — the one covering
    // whatever brought the relay down — is exactly what's lost without this.
    mac_mgmt_common::otel::shutdown();
    served?;
    Ok(())
}
