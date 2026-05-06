use arc_swap::ArcSwap;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use crate::cert_store::CertEntry;
use crate::config::ProxyConfig;

#[derive(Debug, Deserialize)]
struct RouteEntry {
    host: String,
    upstream: String,
}

fn build_client(token: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {token}").parse().unwrap(),
            );
            h
        })
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client")
}

/// Fetch routes from the server API and update the shared route table.
async fn reload_routes(
    client: &reqwest::Client,
    server_url: &str,
    routes: &ArcSwap<HashMap<String, SocketAddr>>,
) {
    match client
        .get(format!("{server_url}/api/internal/routes"))
        .send()
        .await
    {
        Ok(resp) => match resp.json::<Vec<RouteEntry>>().await {
            Ok(entries) => {
                let mut map = HashMap::new();
                for entry in entries {
                    if let Ok(addr) = entry.upstream.parse::<SocketAddr>() {
                        map.insert(entry.host, addr);
                    }
                }
                tracing::info!(count = map.len(), "loaded routes");
                routes.store(Arc::new(map));
            }
            Err(e) => tracing::error!("failed to parse routes: {e}"),
        },
        Err(e) => tracing::error!("failed to fetch routes: {e}"),
    }
}

/// Fetch certs from server and write to disk. Returns true if certs were updated.
async fn reload_certs(
    client: &reqwest::Client,
    server_url: &str,
    cert_files: &crate::cert_store::CertFiles,
) -> bool {
    match client
        .get(format!("{server_url}/api/internal/certs"))
        .send()
        .await
    {
        Ok(resp) => match resp.json::<Vec<CertEntry>>().await {
            Ok(entries) => cert_files.update_from_entries(&entries),
            Err(e) => {
                tracing::error!("failed to parse certs: {e}");
                false
            }
        },
        Err(e) => {
            tracing::error!("failed to fetch certs: {e}");
            false
        }
    }
}

/// Request cert issuance for routes that don't have certs yet.
async fn trigger_missing_certs(
    client: &reqwest::Client,
    server_url: &str,
    routes: &ArcSwap<HashMap<String, SocketAddr>>,
) {
    let route_hosts: Vec<String> = routes.load().keys().cloned().collect();
    for host in route_hosts {
        tracing::info!(domain = %host, "requesting cert issuance");
        if let Err(e) = client
            .post(format!("{server_url}/api/internal/certs/{host}"))
            .send()
            .await
        {
            tracing::warn!(domain = %host, "cert issuance request failed: {e}");
        }
    }
}

/// Do the initial load (blocking before Pingora starts accepting).
pub async fn initial_load(
    cfg: &ProxyConfig,
    routes: &ArcSwap<HashMap<String, SocketAddr>>,
    cert_files: &crate::cert_store::CertFiles,
) {
    let token = cfg.internal_token();
    let client = build_client(&token);
    reload_routes(&client, &cfg.server_url, routes).await;
    reload_certs(&client, &cfg.server_url, cert_files).await;
    trigger_missing_certs(&client, &cfg.server_url, routes).await;
}

/// Build a Pingora background service that listens to SSE events and reloads routes.
pub fn build_service(
    cfg: ProxyConfig,
    routes: Arc<ArcSwap<HashMap<String, SocketAddr>>>,
) -> pingora::services::background::GenBackgroundService<SyncTask> {
    pingora::services::background::background_service("sync", SyncTask { cfg, routes })
}

pub struct SyncTask {
    cfg: ProxyConfig,
    routes: Arc<ArcSwap<HashMap<String, SocketAddr>>>,
}

#[async_trait::async_trait]
impl pingora::services::background::BackgroundService for SyncTask {
    async fn start(&self, mut shutdown: pingora::server::ShutdownWatch) {
        let token = self.cfg.internal_token();
        let client = build_client(&token);
        let server_url = &self.cfg.server_url;

        loop {
            // Connect to SSE stream
            tracing::info!("connecting to SSE event stream");
            match client
                .get(format!("{server_url}/api/internal/events"))
                .send()
                .await
            {
                Ok(resp) => {
                    use eventsource_stream::Eventsource;
                    use futures::StreamExt;

                    let mut stream = resp.bytes_stream().eventsource();
                    loop {
                        tokio::select! {
                            event = stream.next() => {
                                match event {
                                    Some(Ok(ev)) if ev.event == "reload" => {
                                        tracing::info!("received reload event");
                                        reload_routes(&client, server_url, &self.routes).await;
                                        // Note: cert reload requires proxy restart since
                                        // Pingora loads certs at startup. A future version
                                        // using OpenSSL callbacks could hot-reload certs.
                                    }
                                    Some(Ok(_)) => {} // keepalive or other events
                                    Some(Err(e)) => {
                                        tracing::warn!("SSE error: {e}");
                                        break;
                                    }
                                    None => break, // stream ended
                                }
                            }
                            _ = shutdown.changed() => {
                                tracing::info!("shutting down sync task");
                                return;
                            }
                        }
                    }
                }
                Err(e) => tracing::error!("failed to connect to SSE: {e}"),
            }

            // Backoff before reconnecting
            tracing::info!("SSE disconnected, reconnecting in 5s");
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Full route reload on reconnect
            reload_routes(&client, server_url, &self.routes).await;
        }
    }
}
