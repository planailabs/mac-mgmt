//! Fleet endpoints: live heartbeat status (dashboard), per-instance detail
//! (inventory / probes / security posture), the overview command-center
//! snapshot, and stale-instance cleanup.

use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "server")]
use super::internal;
#[cfg(feature = "server")]
use crate::api_mcp::access;
#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

// ── Fleet status (dashboard) ────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FleetStatusInput {
    /// Optional rollout stage to filter by; restricts entries to the stage's
    /// cohort clusters (intersected with the caller's accessible clusters).
    pub stage_id: Option<Uuid>,
}

/// One daemon heartbeat row as shown on the fleet dashboard.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FleetEntry {
    pub cluster_id: String,
    pub cluster_name: String,
    pub instance_id: String,
    pub hostname: String,
    pub environment: String,
    pub version: String,
    /// Git commit the daemon binary was built from. `None` on older daemons.
    #[serde(default)]
    pub git_sha: Option<String>,
    /// Number of commits leading up to git_sha (fetched from GitLab).
    #[serde(default)]
    pub commit_count: Option<u64>,
    #[serde(default)]
    pub nixpkgs_commit: Option<String>,
    pub services: serde_json::Value,
    pub tunnels: serde_json::Value,
    pub relay_proxy_hostname: Option<String>,
    pub relay_proxy_url: Option<String>,
    pub reported_at: DateTime<Utc>,
    /// Latest dynamic sample piggybacked on the heartbeat (CPU/mem/thermal).
    #[serde(default)]
    pub sample: Option<serde_json::Value>,
    /// Rolled-up extended service state from the last probe run.
    #[serde(default)]
    pub services_extended: Option<serde_json::Value>,
    /// True if the viewer may see probe error_detail (admin only).
    #[serde(default)]
    pub viewer_is_admin: bool,
}

/// Wrapped response so the UI can render a "Filtered by …" banner with
/// a human-readable label without a second round-trip per refresh.
/// `stage_label` is `None` when no stage filter is active.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FleetStatusResult {
    pub entries: Vec<FleetEntry>,
    pub stage_label: Option<String>,
}

/// was: get_fleet_status() in web/components/fleet_dashboard.rs
#[api_mcp_dioxus_server(server = "get_fleet_status")]
pub async fn fleet_status(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: FleetStatusInput,
) -> Result<FleetStatusResult, ApiError> {
    let is_admin = p.admin;

    #[derive(sqlx::FromRow)]
    struct Row {
        cluster_id: Uuid,
        cluster_name: String,
        instance_id: String,
        hostname: String,
        environment: String,
        version: String,
        git_sha: Option<String>,
        nixpkgs_commit: Option<String>,
        services: serde_json::Value,
        tunnels: serde_json::Value,
        relay_proxy_hostname: Option<String>,
        relay_proxy_url: Option<String>,
        reported_at: DateTime<Utc>,
        sample: Option<serde_json::Value>,
        services_extended: Option<serde_json::Value>,
    }

    let accessible = access::accessible_cluster_ids(pool, p).await?;

    // Optional stage filter — resolves the stage's cohort cluster_ids and
    // a human label ("Stage 1 · canary"). Intersects with accessible
    // clusters so org-scoped users can't see hosts they couldn't reach
    // in an unfiltered view.
    let (stage_cohort, stage_label) = if let Some(sid) = input.stage_id {
        #[derive(sqlx::FromRow)]
        struct StageMeta {
            stage_order: i32,
            group_name: String,
            group_id: Uuid,
        }
        let meta: StageMeta = sqlx::query_as(
            "SELECT rs.stage_order, rg.name AS group_name, rs.group_id \
             FROM rollout_stages rs JOIN rollout_groups rg ON rg.id = rs.group_id \
             WHERE rs.id = $1",
        )
        .bind(sid)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or_else(|| ApiError::not_found("stage not found"))?;

        let cohort: Vec<Uuid> = sqlx::query_scalar(
            "SELECT cluster_id FROM rollout_group_members WHERE group_id = $1 \
             UNION ALL \
             SELECT id FROM clusters WHERE $1 = '00000000-0000-0000-0000-000000000000'::uuid",
        )
        .bind(meta.group_id)
        .fetch_all(pool)
        .await
        .map_err(internal)?;

        (
            Some(cohort),
            Some(format!("Stage {} · {}", meta.stage_order, meta.group_name)),
        )
    } else {
        (None, None)
    };

    // Compose the effective cluster_id filter from accessible ∩ cohort.
    // None on either side means "no filter from that source".
    let effective: Option<Vec<Uuid>> = match (accessible, stage_cohort) {
        (Some(a), Some(c)) => {
            let cset: std::collections::HashSet<_> = c.iter().copied().collect();
            Some(a.into_iter().filter(|id| cset.contains(id)).collect())
        }
        (Some(a), None) => Some(a),
        (None, Some(c)) => Some(c),
        (None, None) => None,
    };

    let rows = if let Some(ids) = effective {
        sqlx::query_as::<_, Row>(
            "SELECT c.id AS cluster_id, c.name AS cluster_name, dh.instance_id, dh.hostname, dh.environment, dh.version, dh.git_sha, dh.nixpkgs_commit, dh.services, dh.tunnels, dh.relay_proxy_hostname, dh.relay_proxy_url, dh.reported_at, dh.sample, dh.services_extended \
             FROM daemon_heartbeats dh \
             JOIN clusters c ON c.id = dh.cluster_id \
             WHERE dh.cluster_id = ANY($1) \
             ORDER BY dh.reported_at DESC",
        )
        .bind(&ids)
        .fetch_all(pool)
        .await
        .map_err(internal)?
    } else {
        sqlx::query_as::<_, Row>(
            "SELECT c.id AS cluster_id, c.name AS cluster_name, dh.instance_id, dh.hostname, dh.environment, dh.version, dh.git_sha, dh.nixpkgs_commit, dh.services, dh.tunnels, dh.relay_proxy_hostname, dh.relay_proxy_url, dh.reported_at, dh.sample, dh.services_extended \
             FROM daemon_heartbeats dh \
             JOIN clusters c ON c.id = dh.cluster_id \
             ORDER BY dh.reported_at DESC",
        )
        .fetch_all(pool)
        .await
        .map_err(internal)?
    };

    let entries = rows
        .into_iter()
        .map(|r| {
            FleetEntry {
                cluster_id: r.cluster_id.to_string(),
                cluster_name: r.cluster_name,
                instance_id: r.instance_id,
                hostname: r.hostname,
                environment: r.environment,
                version: r.version,
                git_sha: r.git_sha,
                commit_count: None, // fetched async in the component
                nixpkgs_commit: r.nixpkgs_commit,
                services: r.services,
                tunnels: r.tunnels,
                relay_proxy_hostname: r.relay_proxy_hostname,
                relay_proxy_url: r.relay_proxy_url,
                reported_at: r.reported_at,
                sample: r.sample,
                services_extended: r.services_extended,
                viewer_is_admin: is_admin,
            }
        })
        .collect();

    Ok(FleetStatusResult {
        entries,
        stage_label,
    })
}

// ── Fleet detail (per-instance) ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FleetDetailInput {
    /// Daemon instance id (opaque string from the heartbeat).
    pub instance_id: String,
}

/// Latest probe result for one (service, kind) pair.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProbeEntry {
    pub service: String,
    pub kind: String,
    pub ok: bool,
    pub duration_ms: i64,
    pub tokens_in: Option<i32>,
    pub tokens_out: Option<i32>,
    pub first_token_ms: Option<i64>,
    pub model: Option<String>,
    pub canary_digest: Option<String>,
    pub error_class: Option<String>,
    pub error_detail: Option<String>,
    pub collected_at: DateTime<Utc>,
}

/// Per-instance extended assessment: latest heartbeat + inventory + security
/// posture + dynamic sample + per-service probe results.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FleetDetailData {
    pub instance_id: String,
    pub cluster_id: String,
    pub cluster_name: String,
    pub hostname: String,
    pub environment: String,
    pub version: String,
    #[serde(default)]
    pub git_sha: Option<String>,
    pub nixpkgs_commit: Option<String>,
    pub reported_at: DateTime<Utc>,
    pub sample: Option<serde_json::Value>,
    pub services_extended: Option<serde_json::Value>,
    /// Raw `services` JSON array from the heartbeat (name, healthy,
    /// upgrade_pending, busy). Surfaced as badges at the top of the page.
    #[serde(default)]
    pub services: Option<serde_json::Value>,
    /// Raw `tunnels` JSON array (name, port) for the relay proxy buttons.
    #[serde(default)]
    pub tunnels: Option<serde_json::Value>,
    /// Relay proxy hostname (e.g. "relay.plan.ai") from the heartbeat.
    #[serde(default)]
    pub relay_proxy_hostname: Option<String>,
    /// Full relay proxy URL (e.g. "http://localhost:7379") for building tunnel links.
    #[serde(default)]
    pub relay_proxy_url: Option<String>,
    /// Exposed file tunnels for remote config editing.
    #[serde(default)]
    pub file_tunnels: Option<serde_json::Value>,
    /// Exposed shell commands for remote execution.
    #[serde(default)]
    pub shell_tunnels: Option<serde_json::Value>,
    pub inventory: Option<serde_json::Value>,
    pub inventory_collected_at: Option<DateTime<Utc>>,
    pub security: Option<serde_json::Value>,
    /// Per-service dynamic samples from the latest heartbeat.
    #[serde(default)]
    pub service_samples: Option<serde_json::Value>,
    /// Per-service static inventory from the latest assessment.
    #[serde(default)]
    pub service_inventories: Option<serde_json::Value>,
    /// Per-service security findings from the latest assessment.
    #[serde(default)]
    pub service_security: Option<serde_json::Value>,
    pub probes: Vec<ProbeEntry>,
    pub viewer_is_admin: bool,
}

/// was: get_fleet_detail() in web/components/fleet_detail.rs
#[api_mcp_dioxus_server(server = "get_fleet_detail")]
pub async fn fleet_detail(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: FleetDetailInput,
) -> Result<FleetDetailData, ApiError> {
    let instance_id = input.instance_id;

    // Step 1: resolve the latest heartbeat row for this instance and check
    // the viewer is allowed to see its cluster.
    #[derive(sqlx::FromRow)]
    struct HbRow {
        cluster_id: Uuid,
        cluster_name: String,
        hostname: String,
        environment: String,
        version: String,
        git_sha: Option<String>,
        nixpkgs_commit: Option<String>,
        reported_at: DateTime<Utc>,
        sample: Option<serde_json::Value>,
        services_extended: Option<serde_json::Value>,
        services: serde_json::Value,
        tunnels: serde_json::Value,
        relay_proxy_hostname: Option<String>,
        relay_proxy_url: Option<String>,
        file_tunnels: serde_json::Value,
        shell_tunnels: serde_json::Value,
        service_samples: Option<serde_json::Value>,
    }
    let hb: HbRow = sqlx::query_as(
        "SELECT c.id AS cluster_id, c.name AS cluster_name, dh.hostname, dh.environment, \
                dh.version, dh.git_sha, dh.nixpkgs_commit, dh.reported_at, dh.sample, dh.services_extended, \
                dh.services, dh.tunnels, dh.relay_proxy_hostname, dh.relay_proxy_url, dh.file_tunnels, \
                dh.shell_tunnels, dh.service_samples \
         FROM daemon_heartbeats dh JOIN clusters c ON c.id = dh.cluster_id \
         WHERE dh.instance_id = $1 \
         ORDER BY dh.reported_at DESC LIMIT 1",
    )
    .bind(&instance_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or_else(|| ApiError::not_found("instance not found"))?;

    access::require_cluster_read(pool, p, hb.cluster_id).await?;

    // Step 2: latest assessment snapshot (static inventory + security).
    #[derive(sqlx::FromRow)]
    struct AssRow {
        inventory: serde_json::Value,
        security: serde_json::Value,
        service_inventories: Option<serde_json::Value>,
        service_security: Option<serde_json::Value>,
        collected_at: DateTime<Utc>,
    }
    let ass: Option<AssRow> = sqlx::query_as(
        "SELECT inventory, security, service_inventories, service_security, collected_at \
         FROM assessments WHERE instance_id = $1 ORDER BY collected_at DESC LIMIT 1",
    )
    .bind(&instance_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;

    // Step 3: latest probe result per service.
    #[derive(sqlx::FromRow)]
    struct ProbeRow {
        service: String,
        kind: String,
        ok: bool,
        duration_ms: i64,
        tokens_in: Option<i32>,
        tokens_out: Option<i32>,
        first_token_ms: Option<i64>,
        model: Option<String>,
        canary_digest: Option<String>,
        error_class: Option<String>,
        error_detail: Option<String>,
        collected_at: DateTime<Utc>,
    }
    let probe_rows: Vec<ProbeRow> = sqlx::query_as(
        "SELECT DISTINCT ON (service, kind) \
                service, kind, ok, duration_ms, tokens_in, tokens_out, first_token_ms, \
                model, canary_digest, error_class, error_detail, collected_at \
         FROM assessment_probes \
         WHERE instance_id = $1 \
         ORDER BY service, kind, collected_at DESC",
    )
    .bind(&instance_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    let is_admin = p.admin;
    let probes = probe_rows
        .into_iter()
        .map(|pr| ProbeEntry {
            service: pr.service,
            kind: pr.kind,
            ok: pr.ok,
            duration_ms: pr.duration_ms,
            tokens_in: pr.tokens_in,
            tokens_out: pr.tokens_out,
            first_token_ms: pr.first_token_ms,
            model: pr.model,
            canary_digest: pr.canary_digest,
            error_class: pr.error_class,
            // GDPR: error_detail can carry request bodies / paths — admin only.
            error_detail: if is_admin { pr.error_detail } else { None },
            collected_at: pr.collected_at,
        })
        .collect();

    Ok(FleetDetailData {
        instance_id,
        cluster_id: hb.cluster_id.to_string(),
        cluster_name: hb.cluster_name,
        hostname: hb.hostname,
        environment: hb.environment,
        version: hb.version,
        git_sha: hb.git_sha,
        nixpkgs_commit: hb.nixpkgs_commit,
        reported_at: hb.reported_at,
        sample: hb.sample,
        services_extended: hb.services_extended,
        services: Some(hb.services),
        tunnels: Some(hb.tunnels),
        relay_proxy_hostname: hb.relay_proxy_hostname,
        relay_proxy_url: hb.relay_proxy_url,
        file_tunnels: Some(hb.file_tunnels),
        shell_tunnels: Some(hb.shell_tunnels),
        inventory: ass.as_ref().map(|a| a.inventory.clone()),
        inventory_collected_at: ass.as_ref().map(|a| a.collected_at),
        security: ass.as_ref().map(|a| a.security.clone()),
        service_samples: hb.service_samples,
        service_inventories: ass.as_ref().and_then(|a| a.service_inventories.clone()),
        service_security: ass.as_ref().and_then(|a| a.service_security.clone()),
        probes,
        viewer_is_admin: is_admin,
    })
}

// ── Overview (command center) ───────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OverviewInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OverviewActivity {
    /// "rollout-started" | "rollout-completed" | "rollout-other" | "cluster-online"
    pub kind: String,
    pub text: String,
    /// Absolute timestamp; the page formats relative-ago at render.
    pub at: DateTime<Utc>,
}

/// Snapshot delivered to the overview page in a single round-trip.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OverviewData {
    // KPIs
    pub instances_online: i64,
    pub instances_total: i64,
    pub healthy_services_pct: f64,
    pub healthy_services_count: i64,
    pub healthy_services_total: i64,
    pub active_rollouts: i64,
    pub rollouts_completed_24h: i64,
    pub open_staff_pings: i64,
    pub open_failure_signals: i64,
    /// Recent activity, newest first.
    pub activity: Vec<OverviewActivity>,
}

/// was: get_overview() in web/components/overview.rs
#[api_mcp_dioxus_server(server = "get_overview")]
pub async fn fleet_overview(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: OverviewInput,
) -> Result<OverviewData, ApiError> {
    let accessible = access::accessible_cluster_ids(pool, p).await?;

    // Heartbeats. "Online" matches the fleet-dashboard threshold.
    let online_query = "SELECT COUNT(*) FROM daemon_heartbeats \
                        WHERE reported_at > now() - interval '5 minutes'";
    let total_query = "SELECT COUNT(*) FROM daemon_heartbeats";
    let (instances_online, instances_total) = match accessible.as_ref() {
        Some(ids) => {
            let online: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM daemon_heartbeats \
                 WHERE cluster_id = ANY($1) AND reported_at > now() - interval '5 minutes'",
            )
            .bind(ids)
            .fetch_one(pool)
            .await
            .map_err(internal)?;
            let total: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM daemon_heartbeats WHERE cluster_id = ANY($1)",
            )
            .bind(ids)
            .fetch_one(pool)
            .await
            .map_err(internal)?;
            (online, total)
        }
        None => {
            let online: i64 = sqlx::query_scalar(online_query)
                .fetch_one(pool)
                .await
                .map_err(internal)?;
            let total: i64 = sqlx::query_scalar(total_query)
                .fetch_one(pool)
                .await
                .map_err(internal)?;
            (online, total)
        }
    };

    // Service health rolled up across all heartbeats. We only consider
    // recent heartbeats so a long-dead daemon doesn't drag the number
    // down forever.
    let recent_services: Vec<serde_json::Value> = match accessible.as_ref() {
        Some(ids) => sqlx::query_scalar(
            "SELECT services FROM daemon_heartbeats \
             WHERE cluster_id = ANY($1) AND reported_at > now() - interval '5 minutes'",
        )
        .bind(ids)
        .fetch_all(pool)
        .await
        .map_err(internal)?,
        None => sqlx::query_scalar(
            "SELECT services FROM daemon_heartbeats \
             WHERE reported_at > now() - interval '5 minutes'",
        )
        .fetch_all(pool)
        .await
        .map_err(internal)?,
    };
    let mut healthy_services_count = 0i64;
    let mut healthy_services_total = 0i64;
    for v in &recent_services {
        if let Some(arr) = v.as_array() {
            for s in arr {
                healthy_services_total += 1;
                if s.get("healthy").and_then(|x| x.as_bool()) == Some(true) {
                    healthy_services_count += 1;
                }
            }
        }
    }
    let healthy_services_pct = if healthy_services_total > 0 {
        healthy_services_count as f64 * 100.0 / healthy_services_total as f64
    } else {
        0.0
    };

    // Rollouts. Two scopes: currently rolling, and "completed in the
    // last 24h" so the operator gets a sense of recent throughput.
    let active_rollouts: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM rollouts WHERE status = 'rolling'")
            .fetch_one(pool)
            .await
            .map_err(internal)?;
    // The rollouts table tracks state transitions via updated_at —
    // there's no separate completed_at column (those live on
    // rollout_stages). updated_at moves to "now" when status flips
    // to 'completed', so it's the right proxy here.
    let rollouts_completed_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM rollouts \
         WHERE status = 'completed' AND updated_at > now() - interval '24 hours'",
    )
    .fetch_one(pool)
    .await
    .map_err(internal)?;

    // Open staff pings — must match what the Staff Pings page renders
    // (see staff_pings_page.rs: filters by `resolved = false`). Earlier
    // version queried the wrong table (`staff_pings`) on the wrong
    // column (`resolved_at IS NULL`) and silently returned 0 via
    // unwrap_or, hiding all open pings from the dashboard.
    let open_staff_pings: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM healer_staff_pings WHERE resolved = false")
            .fetch_one(pool)
            .await
            .map_err(internal)?;

    // Active critical failure signals across recently-reporting instances.
    let open_failure_signals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM daemon_heartbeats h, \
                LATERAL jsonb_array_elements(h.failure_signals) sig \
         WHERE h.reported_at > now() - interval '10 minutes' \
           AND h.failure_signals IS NOT NULL \
           AND sig->>'severity' = 'critical'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    // Activity feed: union of the most recent rollout state changes
    // and the latest cluster-online heartbeats. The rollouts table
    // doesn't carry separate started_at/completed_at — we use
    // created_at as the start moment and updated_at as the
    // last-state-change moment (which is "completed_at" for completed
    // rollouts and the latest progress tick for rolling ones).
    #[derive(sqlx::FromRow)]
    struct RolloutEvent {
        id: Uuid,
        status: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        target_version: Option<String>,
    }
    let rollout_events: Vec<RolloutEvent> = sqlx::query_as(
        "SELECT id, status, created_at, updated_at, target_version \
         FROM rollouts \
         WHERE updated_at > now() - interval '24 hours' \
         ORDER BY updated_at DESC \
         LIMIT 6",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    let mut activity: Vec<OverviewActivity> = Vec::new();
    for r in &rollout_events {
        let id_short: String = r.id.to_string().chars().take(8).collect();
        let ver = r
            .target_version
            .as_deref()
            .map(|v| format!(" → v{v}"))
            .unwrap_or_default();
        match r.status.as_str() {
            "completed" => {
                activity.push(OverviewActivity {
                    kind: "rollout-completed".into(),
                    text: format!("Rollout {id_short}{ver} completed"),
                    at: r.updated_at,
                });
            }
            "rolling" => {
                activity.push(OverviewActivity {
                    kind: "rollout-started".into(),
                    text: format!("Rollout {id_short}{ver} started"),
                    at: r.created_at,
                });
            }
            other => {
                activity.push(OverviewActivity {
                    kind: "rollout-other".into(),
                    text: format!("Rollout {id_short}{ver} · {other}"),
                    at: r.updated_at,
                });
            }
        }
    }

    // Recent online heartbeats — first time we see an instance after a
    // gap of 30+ minutes counts as "came online". We approximate by
    // showing the most recent `reported_at` per instance among hosts
    // whose previous heartbeat was older than 30 minutes. For the v1
    // we just take the freshest heartbeats; the gap heuristic is a
    // follow-up.
    #[derive(sqlx::FromRow)]
    struct HbEvent {
        hostname: String,
        cluster_name: String,
        reported_at: DateTime<Utc>,
    }
    let recent_hbs: Vec<HbEvent> = sqlx::query_as(
        "SELECT dh.hostname, c.name AS cluster_name, dh.reported_at \
         FROM daemon_heartbeats dh JOIN clusters c ON c.id = dh.cluster_id \
         WHERE dh.reported_at > now() - interval '1 hour' \
         ORDER BY dh.reported_at DESC \
         LIMIT 4",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    for h in recent_hbs {
        activity.push(OverviewActivity {
            kind: "cluster-online".into(),
            text: format!("{} · {}", h.cluster_name, h.hostname),
            at: h.reported_at,
        });
    }

    activity.sort_by(|a, b| b.at.cmp(&a.at));
    activity.truncate(8);

    Ok(OverviewData {
        instances_online,
        instances_total,
        healthy_services_pct,
        healthy_services_count,
        healthy_services_total,
        active_rollouts,
        rollouts_completed_24h,
        open_staff_pings,
        open_failure_signals,
        activity,
    })
}

// ── Stale-instance cleanup ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeleteStaleInstanceInput {
    /// Daemon instance id whose heartbeat row should be removed.
    pub instance_id: String,
}

/// was: delete_stale_instance() in web/components/fleet_dashboard.rs
///
/// Stale-instance delete. Server-side gate is the source of truth — the
/// UI hides the button when last-seen is recent, but a malicious or
/// stale browser tab can still call this directly. We:
///   1. Require write access to the cluster (admins always pass).
///   2. Re-read `reported_at` and reject if it's within the last 24h
///      so a delete can't race a fresh heartbeat.
///   3. Delete the heartbeat row. Migration 031 adds ON DELETE CASCADE
///      FKs from `assessments` and `assessment_probes` on
///      `(cluster_id, instance_id)`, so those rows go with it.
///      `rollout_stage_health_evaluations` references stage_id, not
///      instance, and cohort queries naturally exclude the missing
///      daemon.
#[api_mcp_dioxus_server(server = "delete_stale_instance")]
pub async fn fleet_delete_stale_instance(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: DeleteStaleInstanceInput,
) -> Result<(), ApiError> {
    use chrono::Duration;

    let instance_id = input.instance_id;

    #[derive(sqlx::FromRow)]
    struct Row {
        cluster_id: Uuid,
        reported_at: DateTime<Utc>,
    }
    let row: Row = sqlx::query_as(
        "SELECT cluster_id, reported_at FROM daemon_heartbeats WHERE instance_id = $1",
    )
    .bind(&instance_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or_else(|| ApiError::not_found("instance not found"))?;

    access::require_cluster_write(pool, p, row.cluster_id).await?;

    let age = Utc::now().signed_duration_since(row.reported_at);
    if age < Duration::days(1) {
        return Err(ApiError::bad_request(format!(
            "instance reported {}h ago — only stale instances (>24h) can be deleted",
            age.num_hours().max(0)
        )));
    }

    sqlx::query("DELETE FROM daemon_heartbeats WHERE instance_id = $1 AND cluster_id = $2")
        .bind(&instance_id)
        .bind(row.cluster_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Registration ────────────────────────────────────────────────────────
