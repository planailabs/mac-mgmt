use arc_swap::ArcSwap;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::cert_store::{CertEntry, CertKey, CertStore};
use crate::config::ProxyConfig;
use crate::proxy::{AuthMode, Route};

#[derive(Debug, Deserialize)]
struct RoutesResponse {
    routes: Vec<RouteEntry>,
    /// Seconds until the earliest relay proxy token expires.
    token_lifetime_secs: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RouteEntry {
    host: String,
    upstream: String,
    relay: Option<RelayInfoEntry>,
    auth: Option<AuthInfoEntry>,
}

#[derive(Debug, Deserialize)]
struct RelayInfoEntry {
    url: String,
    proxy_token: String,
}

#[derive(Debug, Deserialize)]
struct AuthInfoEntry {
    mode: String,
    org_id: Option<uuid::Uuid>,
    basic_credentials: Option<Vec<BasicCredentialEntry>>,
}

#[derive(Debug, Deserialize)]
struct BasicCredentialEntry {
    username: String,
    password_hash: String,
}

fn auth_headers(token: &str) -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    h
}

fn build_client(token: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .default_headers(auth_headers(token))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client")
}

/// Separate client for SSE — no response timeout (the connection is long-lived).
fn build_sse_client(token: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .default_headers(auth_headers(token))
        .build()
        .expect("failed to build SSE client")
}

/// Fetch routes from the server API and update the shared route table.
/// Returns `Some(token_lifetime_secs)` on success, `None` on failure.
async fn reload_routes(
    client: &reqwest::Client,
    server_url: &str,
    routes: &ArcSwap<HashMap<String, (Route, AuthMode)>>,
) -> Option<Option<i64>> {
    match client
        .get(format!("{server_url}/api/internal/routes"))
        .send()
        .await
    {
        Ok(resp) => match resp.json::<RoutesResponse>().await {
            Ok(body) => {
                let token_lifetime_secs = body.token_lifetime_secs;
                let entries = body.routes;
                let mut map = HashMap::new();
                for entry in entries {
                    let route = if let Some(relay) = entry.relay {
                        let relay_host = relay
                            .url
                            .strip_prefix("https://")
                            .or_else(|| relay.url.strip_prefix("http://"))
                            .and_then(|s| s.split('/').next())
                            .and_then(|s| s.split(':').next())
                            .unwrap_or("")
                            .to_string();
                        let tls = relay.url.starts_with("https://");
                        Route::Relay {
                            upstream: entry.upstream,
                            url: relay.url,
                            sni: relay_host.clone(),
                            relay_host,
                            tls,
                            proxy_token: relay.proxy_token,
                        }
                    } else {
                        Route::Direct(entry.upstream)
                    };

                    let auth = match entry.auth {
                        Some(a) if a.mode == "oidc" => {
                            AuthMode::Oidc { org_id: a.org_id.unwrap_or_default() }
                        }
                        Some(a) if a.mode == "basic" => {
                            let creds = a.basic_credentials.unwrap_or_default()
                                .into_iter()
                                .map(|c| (c.username, c.password_hash))
                                .collect();
                            AuthMode::Basic { credentials: creds }
                        }
                        _ => AuthMode::None,
                    };

                    map.insert(entry.host, (route, auth));
                }
                for (host, (route, auth)) in &map {
                    let route_desc = match route {
                        Route::Direct(upstream) => format!("direct → {upstream}"),
                        Route::Relay { upstream, tls, .. } => {
                            let scheme = if *tls { "tls" } else { "plain" };
                            format!("relay → {upstream} ({scheme})")
                        }
                    };
                    let auth_desc = match auth {
                        AuthMode::None => "none",
                        AuthMode::Oidc { .. } => "oidc",
                        AuthMode::Basic { .. } => "basic",
                    };
                    tracing::info!(host, route = %route_desc, auth = auth_desc, "route");
                }
                tracing::info!(count = map.len(), ?token_lifetime_secs, "loaded routes");
                routes.store(Arc::new(map));
                Some(token_lifetime_secs)
            }
            Err(e) => {
                tracing::error!("failed to parse routes: {e}");
                None
            }
        },
        Err(e) => {
            tracing::error!("failed to fetch routes: {e}");
            None
        }
    }
}

/// Fetch certs from server and update the cert store in memory.
/// Returns `true` on success.
async fn reload_certs(
    client: &reqwest::Client,
    server_url: &str,
    cert_store: &CertStore,
) -> bool {
    match client
        .get(format!("{server_url}/api/internal/certs"))
        .send()
        .await
    {
        Ok(resp) => match resp.json::<Vec<CertEntry>>().await {
            Ok(entries) => {
                let mut map = HashMap::new();
                for entry in entries {
                    match CertKey::from_pem(&entry.chain_pem, &entry.key_pem) {
                        Ok(ck) => {
                            map.insert(entry.domain, Arc::new(ck));
                        }
                        Err(e) => tracing::error!(domain = %entry.domain, "bad cert PEM: {e}"),
                    }
                }
                tracing::info!(count = map.len(), "loaded certs");
                cert_store.replace_all(map);
                true
            }
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
    routes: &ArcSwap<HashMap<String, (Route, AuthMode)>>,
    cert_store: &CertStore,
) {
    let route_hosts: Vec<String> = routes.load().keys().cloned().collect();
    for host in route_hosts {
        if !cert_store.has_cert(&host) {
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
}

/// Reload routes, certs, and trigger missing cert issuance.
/// Returns the token lifetime from the routes response (if any).
async fn reload_all(
    client: &reqwest::Client,
    server_url: &str,
    routes: &ArcSwap<HashMap<String, (Route, AuthMode)>>,
    cert_store: &CertStore,
) -> Option<i64> {
    let token_lifetime = reload_routes(client, server_url, routes)
        .await
        .and_then(|tl| tl);
    let _ = reload_certs(client, server_url, cert_store).await;
    trigger_missing_certs(client, server_url, routes, cert_store).await;
    token_lifetime
}

/// Do the initial load (blocking before Pingora starts accepting).
/// Retries every 10 seconds until both routes and certs load successfully.
pub async fn initial_load(
    cfg: &ProxyConfig,
    routes: &ArcSwap<HashMap<String, (Route, AuthMode)>>,
    cert_store: &CertStore,
) {
    let token = cfg.internal_token();
    let client = build_client(&token);
    loop {
        let routes_ok = reload_routes(&client, &cfg.server_url, routes).await.is_some();
        let certs_ok = reload_certs(&client, &cfg.server_url, cert_store).await;
        if routes_ok && certs_ok {
            trigger_missing_certs(&client, &cfg.server_url, routes, cert_store).await;
            return;
        }
        tracing::warn!("initial sync failed, retrying in 10s");
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

/// Build a Pingora background service that listens to SSE events and reloads.
pub fn build_service(
    cfg: ProxyConfig,
    routes: Arc<ArcSwap<HashMap<String, (Route, AuthMode)>>>,
    cert_store: Arc<CertStore>,
) -> pingora::services::background::GenBackgroundService<SyncTask> {
    pingora::services::background::background_service(
        "sync",
        SyncTask {
            cfg,
            routes,
            cert_store,
        },
    )
}

pub struct SyncTask {
    cfg: ProxyConfig,
    routes: Arc<ArcSwap<HashMap<String, (Route, AuthMode)>>>,
    cert_store: Arc<CertStore>,
}

#[async_trait::async_trait]
impl pingora::services::background::BackgroundService for SyncTask {
    async fn start(&self, mut shutdown: pingora::server::ShutdownWatch) {
        let token = self.cfg.internal_token();
        let client = build_client(&token);
        let sse_client = build_sse_client(&token);
        let server_url = &self.cfg.server_url;

        // Token refresh timer — re-fetch routes before proxy tokens expire.
        // Initialized to far-future; updated after each successful route load.
        let mut token_refresh = tokio::time::interval(Duration::from_secs(u64::MAX / 2));
        token_refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Consume the first immediate tick.
        token_refresh.tick().await;

        /// Schedule the next token refresh from a lifetime value.
        /// Refreshes at 90% of the lifetime (minimum 60s) so there is margin.
        fn schedule_refresh(interval: &mut tokio::time::Interval, lifetime_secs: Option<i64>) {
            if let Some(secs) = lifetime_secs {
                let refresh_in = ((secs as f64) * 0.9).max(60.0) as u64;
                tracing::info!(refresh_in_secs = refresh_in, "scheduled token refresh");
                interval.reset_after(Duration::from_secs(refresh_in));
            }
        }

        loop {
            tracing::info!("connecting to SSE event stream");

            let connect = sse_client
                .get(format!("{server_url}/api/internal/events"))
                .send();

            let resp = tokio::select! {
                result = connect => result,
                _ = shutdown.changed() => {
                    tracing::info!("shutting down sync task");
                    return;
                }
            };

            match resp {
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
                                        tokio::select! {
                                            tl = reload_all(&client, server_url, &self.routes, &self.cert_store) => {
                                                schedule_refresh(&mut token_refresh, tl);
                                            }
                                            _ = shutdown.changed() => {
                                                tracing::info!("shutting down sync task");
                                                return;
                                            }
                                        }
                                    }
                                    Some(Ok(_)) => {}
                                    Some(Err(e)) => {
                                        tracing::warn!("SSE error: {e}");
                                        break;
                                    }
                                    None => break,
                                }
                            }
                            // Token refresh timer fired — re-fetch routes for fresh tokens
                            _ = token_refresh.tick() => {
                                tracing::info!("token refresh timer fired, reloading routes");
                                tokio::select! {
                                    tl = reload_all(&client, server_url, &self.routes, &self.cert_store) => {
                                        schedule_refresh(&mut token_refresh, tl);
                                    }
                                    _ = shutdown.changed() => {
                                        tracing::info!("shutting down sync task");
                                        return;
                                    }
                                }
                            }
                            // No SSE event (including keepalive) for 60s → assume dead
                            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                                tracing::warn!("SSE inactivity timeout, reconnecting");
                                break;
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

            tracing::info!("SSE disconnected, reconnecting in 5s");
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                _ = shutdown.changed() => {
                    tracing::info!("shutting down sync task");
                    return;
                }
            }
            tokio::select! {
                tl = reload_all(&client, server_url, &self.routes, &self.cert_store) => {
                    schedule_refresh(&mut token_refresh, tl);
                }
                _ = shutdown.changed() => {
                    tracing::info!("shutting down sync task");
                    return;
                }
            }
        }
    }
}
