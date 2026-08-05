//! Non-WebSocket API endpoints: health, tunnel listing, metrics proxy,
//! federated metrics.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::auth::{SelfInfo, validate_cert, validate_token};
use crate::daemon_registry::DaemonRegistry;
use crate::metrics_federation::{
    PROMETHEUS_CONTENT_TYPE, encode_families, parse_and_relabel, push_gauge_strs,
};
use crate::mtls::ClientCertInfo;
use crate::p2p::RelaySwarm;

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<DaemonRegistry>,
    pub server_api_url: String,
    pub relay_swarm: Arc<RelaySwarm>,
    /// In-memory token for the batch instances endpoint.
    pub batch_token: String,
    /// External URL of the relay (e.g. "https://relay.plan.ai"), used to
    /// build tunnel-subdomain redirect URLs for /cert-login.
    pub proxy_url: Option<String>,
}

pub fn router(
    registry: Arc<DaemonRegistry>,
    server_api_url: String,
    relay_swarm: Arc<RelaySwarm>,
    batch_token: String,
    proxy_url: Option<String>,
) -> Router {
    let state = AppState {
        registry,
        server_api_url,
        relay_swarm,
        batch_token,
        proxy_url,
    };

    use mac_mgmt_common::otel::http as otel_http;

    Router::new()
        .route(
            "/api/daemon/{instance_id}/metrics/{*path}",
            get(proxy_metrics),
        )
        .route("/cert-login/{instance}/{service}", get(cert_login))
        .route("/api/tunnels", get(list_tunnels))
        .route("/api/ssh", get(list_ssh_targets))
        .route("/api/batch/instances", get(batch_instances))
        .route("/certificate-info", get(certificate_info))
        // A layer applies to the routes registered above it, so /metrics and
        // /health below stay untraced — scrape and probe traffic is constant
        // and would be pure export volume.
        .layer(otel_http::trace_layer())
        .layer(middleware::from_fn(otel_http::record_request_metrics))
        .route("/metrics", get(federated_metrics))
        .route("/health", get(health))
        .layer(middleware::from_fn(security_headers))
        .layer(tower::limit::ConcurrencyLimitLayer::new(4096))
        .with_state(state)
}

async fn security_headers(request: axum::extract::Request, next: Next) -> axum::response::Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    // Keep a handler-set CSP (e.g. /cert-login HTML pages need inline styles).
    if !headers.contains_key(axum::http::header::CONTENT_SECURITY_POLICY) {
        headers.insert(
            axum::http::header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
        );
    }
    headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        axum::http::header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY"),
    );
    response
}

async fn health() -> &'static str {
    "ok"
}

/// Batch endpoint: returns all connected daemons with tunnel definitions.
/// Authenticated with the relay's in-memory batch token.
async fn batch_instances(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> axum::response::Response {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    match token {
        Some(t) if t == state.batch_token => {}
        _ => return StatusCode::UNAUTHORIZED.into_response(),
    }
    let instances = state.registry.list_instances();
    Json(instances).into_response()
}

fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(String::from)
}

/// Authenticate via Bearer token or client certificate.
/// When `cert_info` is provided and no Bearer token is present, falls back
/// to certificate-based auth via the server's /api/cert-auth endpoint.
async fn require_auth_ext(
    headers: &HeaderMap,
    server_api_url: &str,
    allowed_kinds: &[&str],
    cert_info: Option<&ClientCertInfo>,
) -> Result<SelfInfo, axum::response::Response> {
    // Try Bearer token first.
    if let Some(token) = extract_bearer(headers) {
        let self_info = validate_token(server_api_url, &token)
            .await
            .map_err(|s| s.into_response())?;
        if !allowed_kinds.contains(&self_info.token_kind.as_str()) {
            return Err(StatusCode::FORBIDDEN.into_response());
        }
        return Ok(self_info);
    }

    // Fall back to client certificate auth.
    if let Some(cert) = cert_info {
        let cert_auth = validate_cert(
            server_api_url,
            &cert.fingerprint_sha256,
            &cert.certificate_pem,
        )
        .await
        .map_err(|s| {
            if s == StatusCode::FORBIDDEN {
                // Return the fingerprint so the user can add it.
                Json(serde_json::json!({
                    "error": "certificate not authorized",
                    "cert_fingerprint": cert.fingerprint_sha256,
                    "hint": "Add this fingerprint to cluster or admin certificate settings"
                }))
                .into_response()
            } else {
                s.into_response()
            }
        })?;
        // Convert CertAuthInfo to SelfInfo for compatibility.
        let self_info = SelfInfo {
            cluster_id: None,
            cluster_name: None,
            organization_id: None,
            token_kind: cert_auth.token_kind,
            cluster_ids: cert_auth.cluster_ids,
            scopes: vec![],
        };
        if !allowed_kinds.contains(&self_info.token_kind.as_str())
            && !(allowed_kinds.contains(&"admin") && self_info.token_kind == "cert_admin")
            && !(allowed_kinds.contains(&"setting")
                && (self_info.token_kind == "cert_cluster"
                    || self_info.token_kind == "cert_organization"))
        {
            return Err(StatusCode::FORBIDDEN.into_response());
        }
        return Ok(self_info);
    }

    Err(StatusCode::UNAUTHORIZED.into_response())
}

// ── Certificate info ────────────────────────────────────────────────

async fn certificate_info(
    cert: Option<axum::Extension<ClientCertInfo>>,
    State(state): State<AppState>,
) -> axum::response::Response {
    let Some(cert) = cert else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "no client certificate presented"})),
        )
            .into_response();
    };

    let (authorized, token_kind, cluster_ids, organization_ids) = match validate_cert(
        &state.server_api_url,
        &cert.fingerprint_sha256,
        &cert.certificate_pem,
    )
    .await
    {
        Ok(info) => (
            true,
            info.token_kind,
            info.cluster_ids,
            info.organization_ids,
        ),
        Err(_) => (false, String::new(), vec![], vec![]),
    };

    Json(serde_json::json!({
        "fingerprint": cert.fingerprint_sha256,
        "subject": cert.subject,
        "certificate_pem": cert.certificate_pem,
        "authorized": authorized,
        "token_kind": token_kind,
        "cluster_ids": cluster_ids,
        "organization_ids": organization_ids,
    }))
    .into_response()
}

// ── Certificate login (browser flow) ────────────────────────────────
//
// GET /cert-login/{instance}/{service} on the MAIN relay domain — the only
// place the TLS layer sends an empty CA hint list, so the browser shows its
// client-certificate picker here (and nowhere else). Exchanges the presented
// certificate for a tunnel-scoped proxy token via the server, then redirects
// into the regular /proxy?proxy_token=… bootstrap on the tunnel subdomain.

/// CSP for the HTML pages below: the plan-ai-html layout uses an inline
/// theme script and inline styles.
const CERT_LOGIN_CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; frame-ancestors 'none'";

fn cert_login_page(
    status: StatusCode,
    lang: plan_ai_html::Lang,
    title_key: &str,
    body_html: String,
) -> axum::response::Response {
    let title = plan_ai_html::tr(lang, title_key);
    let body = format!("<h1 class=\"h-page\">{title}</h1>{body_html}");
    let html = plan_ai_html::Page::new(&title, body).lang(lang).render();
    (
        status,
        [
            ("content-type", "text/html; charset=utf-8"),
            ("content-security-policy", CERT_LOGIN_CSP),
        ],
        html,
    )
        .into_response()
}

async fn cert_login(
    headers: HeaderMap,
    cert: Option<axum::Extension<ClientCertInfo>>,
    Path((instance, service)): Path<(String, String)>,
    State(state): State<AppState>,
) -> axum::response::Response {
    let lang = plan_ai_html::Lang::from_accept_language(
        headers
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    );

    // Path params feed a redirect URL — keep them to subdomain-safe chars.
    let valid = instance.len() >= 12
        && instance.chars().all(|c| c.is_ascii_hexdigit())
        && !service.is_empty()
        && service
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        return (StatusCode::BAD_REQUEST, "invalid instance or service").into_response();
    }

    let Some(cert) = cert else {
        return cert_login_page(
            StatusCode::UNAUTHORIZED,
            lang,
            "cert-login-no-cert-title",
            format!(
                "<p class=\"help\">{}</p>",
                plan_ai_html::tr(lang, "cert-login-no-cert-body"),
            ),
        );
    };

    let Some(proxy_url) = state.proxy_url.as_deref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "proxy_url not configured").into_response();
    };

    if !state.registry.has_tunnel(&instance, &service) {
        return cert_login_page(
            StatusCode::NOT_FOUND,
            lang,
            "not-found-title",
            format!(
                "<p class=\"help\">{}</p>",
                plan_ai_html::tr(lang, "not-found-body"),
            ),
        );
    }

    // Authorize: the cert was presented over THIS TLS connection (possession
    // proven by the handshake); the server only tells us its scope.
    let denied_page = |fp: &str| {
        cert_login_page(
            StatusCode::FORBIDDEN,
            lang,
            "cert-login-denied-title",
            format!(
                "<p class=\"help\">{}</p><p class=\"help\" style=\"margin-top:.6rem;word-break:break-all\"><code>{}</code></p>",
                plan_ai_html::tr(lang, "cert-login-denied-body"),
                plan_ai_html::escape(fp),
            ),
        )
    };
    let cert_auth = match validate_cert(
        &state.server_api_url,
        &cert.fingerprint_sha256,
        &cert.certificate_pem,
    )
    .await
    {
        Ok(info) => info,
        Err(StatusCode::FORBIDDEN) => return denied_page(&cert.fingerprint_sha256),
        Err(status) => return status.into_response(),
    };

    // The cert must cover the instance's cluster. If the daemon didn't report
    // a cluster ID, only admin certificates get through.
    let authorized = match state.registry.get_cluster_id(&instance) {
        Some(cid) => cert_auth.cluster_ids.contains(&cid),
        None => cert_auth.token_kind == "cert_admin",
    };
    if !authorized {
        return denied_page(&cert.fingerprint_sha256);
    }

    let token = crate::auth::mint_local_proxy_token(&instance, &service);
    tracing::info!(
        instance = %instance,
        service = %service,
        fingerprint = %cert.fingerprint_sha256,
        "cert-login: minted local proxy token"
    );

    // Same shape as the server's build_tunnel_url.
    let scheme = if proxy_url.starts_with("https://") {
        "https://"
    } else {
        "http://"
    };
    let host = proxy_url
        .strip_prefix(scheme)
        .unwrap_or(proxy_url)
        .trim_end_matches('/');
    let url = format!("{scheme}{instance}-{service}.{host}/proxy?proxy_token={token}");
    axum::response::Redirect::to(&url).into_response()
}

fn is_safe_path(path: &str) -> bool {
    !path.contains("..") && !path.contains("//")
}

// ── Metrics proxy ────────────────────────────────────────────────────

async fn proxy_metrics(
    headers: HeaderMap,
    cert: Option<axum::Extension<ClientCertInfo>>,
    Path((instance_id, path)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> axum::response::Response {
    let cert_ref = cert.as_ref().map(|c| &c.0);
    if let Err(resp) = require_auth_ext(
        &headers,
        &state.server_api_url,
        &["admin", "setting"],
        cert_ref,
    )
    .await
    {
        return resp;
    }

    if !is_safe_path(&path) {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let full_path = if query.is_empty() {
        format!("/{path}")
    } else {
        let qs: String = query
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        format!("/{path}?{qs}")
    };

    let Some(peer_id) = state.registry.resolve_peer_id(&instance_id) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let handshake = serde_json::json!({
        "type": "metrics",
        "path": full_path,
    });

    match crate::tunnel_io::open_and_read_response(
        &state.relay_swarm,
        peer_id,
        handshake,
        Duration::from_secs(10),
    )
    .await
    {
        Ok((status, content_type, body)) => axum::response::Response::builder()
            .status(status)
            .header("content-type", content_type)
            .body(axum::body::Body::from(body))
            .expect("response builder")
            .into_response(),
        Err(status) => status.into_response(),
    }
}

// ── Tunnel listing ───────────────────────────────────────────────────

async fn list_tunnels(
    headers: HeaderMap,
    cert: Option<axum::Extension<ClientCertInfo>>,
    State(state): State<AppState>,
) -> axum::response::Response {
    let cert_ref = cert.as_ref().map(|c| &c.0);
    let self_info = match require_auth_ext(
        &headers,
        &state.server_api_url,
        &["admin", "setting"],
        cert_ref,
    )
    .await
    {
        Ok(info) => info,
        Err(resp) => return resp,
    };

    let mut tunnels = state.registry.list_tunnels();
    if !self_info.token_kind.contains("admin") {
        let allowed = &self_info.cluster_ids;
        tunnels.retain(|t| t.cluster_id.is_some_and(|c| allowed.contains(&c)));
    }
    Json(tunnels).into_response()
}

// ── SSH target listing ───────────────────────────────────────────────

async fn list_ssh_targets(
    headers: HeaderMap,
    cert: Option<axum::Extension<ClientCertInfo>>,
    State(state): State<AppState>,
) -> axum::response::Response {
    let cert_ref = cert.as_ref().map(|c| &c.0);
    let self_info = match require_auth_ext(
        &headers,
        &state.server_api_url,
        &["admin", "setting"],
        cert_ref,
    )
    .await
    {
        Ok(info) => info,
        Err(resp) => return resp,
    };

    let mut targets = state.registry.list_ssh_targets();
    if !self_info.token_kind.contains("admin") {
        let allowed = &self_info.cluster_ids;
        targets.retain(|t| t.cluster_id.is_some_and(|c| allowed.contains(&c)));
    }
    Json(targets).into_response()
}

// ── Federated metrics ────────────────────────────────────────────────

const FEDERATION_SCRAPE_TIMEOUT: Duration = Duration::from_secs(5);
const FEDERATION_CONCURRENCY: usize = 32;

#[allow(dead_code)]
struct ScrapeOutcome {
    instance_id: String,
    hostname: String,
    cluster_id: String,
    cluster_name: String,
    families: Vec<prometheus::proto::MetricFamily>,
    up: bool,
    duration_secs: f64,
}

async fn federated_metrics(
    headers: HeaderMap,
    cert: Option<axum::Extension<ClientCertInfo>>,
    State(state): State<AppState>,
) -> axum::response::Response {
    let cert_ref = cert.as_ref().map(|c| &c.0);
    let self_info = match require_auth_ext(
        &headers,
        &state.server_api_url,
        &["admin", "setting"],
        cert_ref,
    )
    .await
    {
        Ok(info) => info,
        Err(resp) => return resp,
    };

    let mut tunnels = state.registry.list_tunnels();
    if self_info.token_kind != "admin" {
        let allowed = &self_info.cluster_ids;
        tunnels.retain(|t| t.cluster_id.is_some_and(|c| allowed.contains(&c)));
    }

    let registry = state.registry.clone();
    let swarm = state.relay_swarm.clone();
    let outcomes: Vec<ScrapeOutcome> =
        futures_util::stream::iter(tunnels.into_iter().map(move |tunnel| {
            let registry = registry.clone();
            let swarm = swarm.clone();
            async move { scrape_one(&registry, &swarm, tunnel).await }
        }))
        .buffer_unordered(FEDERATION_CONCURRENCY)
        .collect()
        .await;

    let target_count = outcomes.len();

    let mut families: std::collections::BTreeMap<String, prometheus::proto::MetricFamily> =
        std::collections::BTreeMap::new();

    for outcome in outcomes {
        let labels: [(&str, &str); 3] = [
            ("instance_id", outcome.instance_id.as_str()),
            ("hostname", outcome.hostname.as_str()),
            ("cluster_id", outcome.cluster_id.as_str()),
        ];
        push_gauge_strs(
            &mut families,
            "mac_mgmt_relay_scrape_up",
            &labels,
            if outcome.up { 1.0 } else { 0.0 },
        );
        push_gauge_strs(
            &mut families,
            "mac_mgmt_relay_scrape_duration_seconds",
            &labels,
            outcome.duration_secs,
        );
        for fam in outcome.families {
            merge_family(&mut families, fam);
        }
    }

    push_gauge_strs(
        &mut families,
        "mac_mgmt_relay_scrape_targets",
        &[],
        target_count as f64,
    );

    // The relay's own OpenTelemetry metrics, rendered through the Prometheus
    // converter. Collecting runs the observable callbacks, so gauges read at
    // scrape time — same as the federated daemon series next to them.
    for fam in mac_mgmt_common::metrics::prom::registry().gather() {
        merge_family(&mut families, fam);
    }

    let families_vec: Vec<_> = families.into_values().collect();
    match encode_families(&families_vec) {
        Ok(buf) => axum::response::Response::builder()
            .status(StatusCode::OK)
            .header("content-type", PROMETHEUS_CONTENT_TYPE)
            .body(axum::body::Body::from(buf))
            .expect("response builder")
            .into_response(),
        Err(e) => {
            tracing::error!("federated metrics encode failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn scrape_one(
    registry: &Arc<DaemonRegistry>,
    swarm: &Arc<RelaySwarm>,
    tunnel: crate::daemon_registry::TunnelInfo,
) -> ScrapeOutcome {
    let started = std::time::Instant::now();
    let instance_id = tunnel.instance_id.clone();
    let hostname = tunnel.hostname.clone().unwrap_or_default();
    let cluster_id = tunnel.cluster_id.map(|c| c.to_string()).unwrap_or_default();
    let cluster_name = tunnel.cluster_name.clone().unwrap_or_default();

    let Some(peer_id) = registry.resolve_peer_id(&instance_id) else {
        tracing::warn!(%instance_id, "metrics scrape: no peer_id, daemon registered but not connected via p2p");
        return ScrapeOutcome {
            instance_id,
            hostname,
            cluster_id,
            cluster_name,
            families: Vec::new(),
            up: false,
            duration_secs: started.elapsed().as_secs_f64(),
        };
    };

    let handshake = serde_json::json!({
        "type": "metrics",
        "path": "/metrics",
    });

    match tokio::time::timeout(
        FEDERATION_SCRAPE_TIMEOUT,
        crate::tunnel_io::open_and_read_response(
            swarm,
            peer_id,
            handshake,
            FEDERATION_SCRAPE_TIMEOUT,
        ),
    )
    .await
    {
        Ok(Ok((status, _content_type, body))) => {
            if status == 200 {
                match parse_and_relabel(&body, &instance_id, &hostname, &cluster_id, &cluster_name)
                {
                    Ok(families) => ScrapeOutcome {
                        instance_id,
                        hostname,
                        cluster_id,
                        cluster_name,
                        families,
                        up: true,
                        duration_secs: started.elapsed().as_secs_f64(),
                    },
                    Err(e) => {
                        tracing::warn!("federated metrics parse error from {instance_id}: {e}");
                        ScrapeOutcome {
                            instance_id,
                            hostname,
                            cluster_id,
                            cluster_name,
                            families: Vec::new(),
                            up: false,
                            duration_secs: started.elapsed().as_secs_f64(),
                        }
                    }
                }
            } else {
                ScrapeOutcome {
                    instance_id,
                    hostname,
                    cluster_id,
                    cluster_name,
                    families: Vec::new(),
                    up: false,
                    duration_secs: started.elapsed().as_secs_f64(),
                }
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(%instance_id, ?e, "metrics scrape failed: tunnel error");
            ScrapeOutcome {
                instance_id,
                hostname,
                cluster_id,
                cluster_name,
                families: Vec::new(),
                up: false,
                duration_secs: started.elapsed().as_secs_f64(),
            }
        }
        Err(_) => {
            tracing::warn!(%instance_id, "metrics scrape failed: timeout");
            ScrapeOutcome {
                instance_id,
                hostname,
                cluster_id,
                cluster_name,
                families: Vec::new(),
                up: false,
                duration_secs: started.elapsed().as_secs_f64(),
            }
        }
    }
}

fn merge_family(
    families: &mut std::collections::BTreeMap<String, prometheus::proto::MetricFamily>,
    incoming: prometheus::proto::MetricFamily,
) {
    use prometheus::proto::MetricFamily;
    let name = incoming.name().to_string();
    match families.get_mut(&name) {
        Some(existing) => {
            for m in incoming.metric {
                existing.mut_metric().push(m);
            }
        }
        None => {
            let mut fam = MetricFamily::default();
            fam.set_name(name.clone());
            fam.set_field_type(incoming.type_());
            for m in incoming.metric {
                fam.mut_metric().push(m);
            }
            families.insert(name, fam);
        }
    }
}
