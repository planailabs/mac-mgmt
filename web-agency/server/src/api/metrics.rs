//! Prometheus metrics endpoint.
//!
//! `GET /api/metrics` — returns Prometheus text format metrics.
//!
//! Auth: Bearer token with `kind = 'metrics'` in the `tokens` table.
//! - If the token has `organization_id` set → org-scoped (only that org's metrics).
//! - If `organization_id` is NULL → admin (all orgs + global counters).

use dioxus::fullstack::axum::{
    self as axum, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use prometheus::{Encoder, GaugeVec, IntCounterVec, IntGaugeVec, Opts, Registry, TextEncoder};
use sqlx::PgPool;
use std::sync::atomic::Ordering;
use uuid::Uuid;

#[derive(Clone)]
pub struct MetricsState {
    pub pool: PgPool,
}

pub fn router(state: MetricsState) -> Router<()> {
    Router::new()
        .route("/api/metrics", get(metrics_handler))
        .with_state(state)
}

struct MetricsAuth {
    organization_id: Option<Uuid>,
}

async fn authenticate_metrics(
    pool: &PgPool,
    headers: &HeaderMap,
) -> Result<MetricsAuth, (StatusCode, String)> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or((StatusCode::UNAUTHORIZED, "missing Bearer token".into()))?;

    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(token.as_bytes()));

    let row = sqlx::query_as::<_, (String, Option<Uuid>)>(
        "SELECT kind, organization_id FROM tokens \
         WHERE token_hash = $1 AND NOT revoked \
           AND (expires_at IS NULL OR expires_at > now())",
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::UNAUTHORIZED, "invalid token".into()))?;

    let (kind, organization_id) = row;
    if kind != "metrics" {
        return Err((StatusCode::FORBIDDEN, "token kind must be 'metrics'".into()));
    }

    Ok(MetricsAuth { organization_id })
}

async fn metrics_handler(
    State(state): State<MetricsState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let auth = authenticate_metrics(&state.pool, &headers).await?;
    let registry = Registry::new();

    register_webspace_running(&registry, &state.pool, auth.organization_id).await?;
    register_reachability(&registry, &state.pool, auth.organization_id).await?;
    register_relay_mint_status(&registry, auth.organization_id)?;

    if auth.organization_id.is_none() {
        register_admin_gauges(&registry)?;
        register_admin_counters(&registry)?;
    }

    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("encode: {e}")))?;

    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        buffer,
    ))
}

// ── Per-webspace running gauge (local hosting only) ──────────────────

async fn register_webspace_running(
    registry: &Registry,
    pool: &PgPool,
    org_filter: Option<Uuid>,
) -> Result<(), (StatusCode, String)> {
    let rows = if let Some(org_id) = org_filter {
        sqlx::query_as::<_, (String, String, String)>(
            "SELECT w.name, o.name, w.local_status \
             FROM webspaces w \
             JOIN organizations o ON o.id = w.organization_id \
             WHERE w.hosting_type = 'local' AND w.organization_id = $1",
        )
        .bind(org_id)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, (String, String, String)>(
            "SELECT w.name, o.name, w.local_status \
             FROM webspaces w \
             JOIN organizations o ON o.id = w.organization_id \
             WHERE w.hosting_type = 'local'",
        )
        .fetch_all(pool)
        .await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if rows.is_empty() {
        return Ok(());
    }

    let gauge = IntGaugeVec::new(
        Opts::new(
            "web_agency_webspace_running",
            "Whether the local webspace is running (1) or not (0)",
        ),
        &["org", "webspace"],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    registry
        .register(Box::new(gauge.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (webspace_name, org_name, status) in &rows {
        let val = if status == "running" { 1 } else { 0 };
        gauge.with_label_values(&[org_name, webspace_name]).set(val);
    }

    Ok(())
}

// ── Reachability gauges ──────────────────────────────────────────────

async fn register_reachability(
    registry: &Registry,
    pool: &PgPool,
    org_filter: Option<Uuid>,
) -> Result<(), (StatusCode, String)> {
    let rows = if let Some(org_id) = org_filter {
        sqlx::query_as::<_, (String, String, String, bool, bool, bool, Option<i32>, f64)>(
            "SELECT h.name, o.name, r.hostname, r.http_ok, r.ssl_ok, r.proxy_ok, r.latency_ms, \
                    EXTRACT(EPOCH FROM r.checked_at)::float8 \
             FROM reachability_results r \
             JOIN webspace_hosts h ON h.id = r.webspace_host_id \
             JOIN organizations o ON o.id = r.organization_id \
             WHERE r.organization_id = $1",
        )
        .bind(org_id)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, (String, String, String, bool, bool, bool, Option<i32>, f64)>(
            "SELECT h.name, o.name, r.hostname, r.http_ok, r.ssl_ok, r.proxy_ok, r.latency_ms, \
                    EXTRACT(EPOCH FROM r.checked_at)::float8 \
             FROM reachability_results r \
             JOIN webspace_hosts h ON h.id = r.webspace_host_id \
             JOIN organizations o ON o.id = r.organization_id",
        )
        .fetch_all(pool)
        .await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if rows.is_empty() {
        return Ok(());
    }

    let labels = &["org", "webspace", "hostname"];

    let http_ok = IntGaugeVec::new(
        Opts::new(
            "web_agency_reachability_http_ok",
            "HTTP reachability (1=ok, 0=fail)",
        ),
        labels,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let ssl_ok = IntGaugeVec::new(
        Opts::new(
            "web_agency_reachability_ssl_ok",
            "SSL validity (1=ok, 0=fail)",
        ),
        labels,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let proxy_ok = IntGaugeVec::new(
        Opts::new(
            "web_agency_reachability_proxy_ok",
            "Proxy verification via .well-known (1=ok, 0=fail)",
        ),
        labels,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let latency = GaugeVec::new(
        Opts::new(
            "web_agency_reachability_latency_ms",
            "Reachability check latency in milliseconds",
        ),
        labels,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let checked_at = GaugeVec::new(
        Opts::new(
            "web_agency_reachability_checked_at",
            "Unix timestamp of last reachability check",
        ),
        labels,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    registry
        .register(Box::new(http_ok.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    registry
        .register(Box::new(ssl_ok.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    registry
        .register(Box::new(proxy_ok.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    registry
        .register(Box::new(latency.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    registry
        .register(Box::new(checked_at.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (ws_name, org_name, hostname, h_ok, s_ok, p_ok, lat_ms, ts) in &rows {
        let lv = &[org_name.as_str(), ws_name.as_str(), hostname.as_str()];
        http_ok.with_label_values(lv).set(if *h_ok { 1 } else { 0 });
        ssl_ok.with_label_values(lv).set(if *s_ok { 1 } else { 0 });
        proxy_ok
            .with_label_values(lv)
            .set(if *p_ok { 1 } else { 0 });
        if let Some(ms) = lat_ms {
            latency.with_label_values(lv).set(*ms as f64);
        }
        checked_at.with_label_values(lv).set(*ts);
    }

    Ok(())
}

// ── Per-webspace relay mint status ───────────────────────────────────

fn register_relay_mint_status(
    registry: &Registry,
    org_filter: Option<Uuid>,
) -> Result<(), (StatusCode, String)> {
    let map = super::counters::RELAY_MINT_RESULTS.lock().unwrap();
    let filtered: Vec<_> = map
        .iter()
        .filter(|(_, result)| org_filter.is_none() || org_filter == Some(result.org_id))
        .collect();

    if filtered.is_empty() {
        return Ok(());
    }

    let mint_ok = IntGaugeVec::new(
        Opts::new(
            "web_agency_relay_token_mint_ok",
            "Whether the last relay token mint succeeded (1) or failed (0)",
        ),
        &["hostname"],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    registry
        .register(Box::new(mint_ok.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (hostname, result) in &filtered {
        mint_ok
            .with_label_values(&[hostname])
            .set(if result.success { 1 } else { 0 });
    }

    Ok(())
}

// ── Admin-only per-cycle gauges ──────────────────────────────────────

fn register_admin_gauges(registry: &Registry) -> Result<(), (StatusCode, String)> {
    let counters = &super::counters::COUNTERS;

    let sync_errors = IntGaugeVec::new(
        Opts::new(
            "web_agency_sync_last_errors",
            "Number of errors in the last sync cycle",
        ),
        &["sync_type"],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    registry
        .register(Box::new(sync_errors.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sync_errors
        .with_label_values(&["dns"])
        .set(counters.sync_dns_errors.load(Ordering::Relaxed) as i64);
    sync_errors
        .with_label_values(&["expiry"])
        .set(counters.sync_expiry_errors.load(Ordering::Relaxed) as i64);
    sync_errors
        .with_label_values(&["nameserver"])
        .set(counters.sync_ns_errors.load(Ordering::Relaxed) as i64);
    sync_errors
        .with_label_values(&["bot_protection"])
        .set(counters.sync_bot_errors.load(Ordering::Relaxed) as i64);

    let cert_renewal = IntGaugeVec::new(
        Opts::new(
            "web_agency_cert_renewal_last_results",
            "Results from the last cert renewal tick",
        ),
        &["status"],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    registry
        .register(Box::new(cert_renewal.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    cert_renewal
        .with_label_values(&["success"])
        .set(counters.cert_renewal_success.load(Ordering::Relaxed) as i64);
    cert_renewal
        .with_label_values(&["failed"])
        .set(counters.cert_renewal_failed.load(Ordering::Relaxed) as i64);

    let cert_issuance = IntGaugeVec::new(
        Opts::new(
            "web_agency_cert_issuance_last_results",
            "Results from the last cert issuance tick",
        ),
        &["status"],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    registry
        .register(Box::new(cert_issuance.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    cert_issuance
        .with_label_values(&["success"])
        .set(counters.cert_issuance_success.load(Ordering::Relaxed) as i64);
    cert_issuance
        .with_label_values(&["failed"])
        .set(counters.cert_issuance_failed.load(Ordering::Relaxed) as i64);

    Ok(())
}

// ── Admin-only cumulative counters ───────────────────────────────────

fn register_admin_counters(registry: &Registry) -> Result<(), (StatusCode, String)> {
    let counters = &super::counters::COUNTERS;

    let deployments = IntCounterVec::new(
        Opts::new(
            "web_agency_deployments_total",
            "Total deployments since server start",
        ),
        &["status"],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    registry
        .register(Box::new(deployments.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let success = counters.deploy_success.load(Ordering::Relaxed);
    let failed = counters.deploy_failed.load(Ordering::Relaxed);
    if success > 0 {
        deployments.with_label_values(&["success"]).inc_by(success);
    }
    if failed > 0 {
        deployments.with_label_values(&["failed"]).inc_by(failed);
    }

    let relay_mints = IntCounterVec::new(
        Opts::new(
            "web_agency_relay_token_mint_total",
            "Total relay token mint attempts since server start",
        ),
        &["status"],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    registry
        .register(Box::new(relay_mints.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mint_success = counters.relay_mint_success.load(Ordering::Relaxed);
    let mint_failed = counters.relay_mint_failed.load(Ordering::Relaxed);
    if mint_success > 0 {
        relay_mints
            .with_label_values(&["success"])
            .inc_by(mint_success);
    }
    if mint_failed > 0 {
        relay_mints
            .with_label_values(&["failed"])
            .inc_by(mint_failed);
    }

    Ok(())
}
