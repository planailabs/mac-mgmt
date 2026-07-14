//! Rollout + rollout-group endpoints: staged daemon-version/nixpkgs rollouts
//! (create/list/detail/lifecycle actions, per-stage health gates and push
//! triggers) and the rollout groups that define stage cohorts.

use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::gate_input::HealthGateInput;

#[cfg(feature = "server")]
use super::internal;
#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

// ── Health-gate DTO ─────────────────────────────────────────────────────
//
// `web::gate_input::HealthGateInput` is the form-side type and doesn't
// implement `JsonSchema`; this DTO mirrors it for the API surface and
// converts losslessly in both directions.

/// One probed service and the minimum share of instances (0-100) whose
/// probe must pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProbeThresholdDto {
    pub service: String,
    pub min_ok_pct: u8,
}

/// A stage health gate. `enabled = false` means "no gate" (stored as SQL
/// NULL in `rollout_stages.health_gate`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HealthGateDto {
    pub enabled: bool,
    /// Minimum share of the cohort (0-100) with a fresh heartbeat.
    pub min_heartbeat_fresh_pct: u8,
    /// How recent a heartbeat must be (seconds) to count as fresh.
    pub heartbeat_freshness_secs: u32,
    /// Seconds after stage start during which failures don't auto-pause.
    pub grace_period_secs: u32,
    pub probe_thresholds: Vec<ProbeThresholdDto>,
}

impl From<HealthGateInput> for HealthGateDto {
    fn from(g: HealthGateInput) -> Self {
        HealthGateDto {
            enabled: g.enabled,
            min_heartbeat_fresh_pct: g.min_heartbeat_fresh_pct,
            heartbeat_freshness_secs: g.heartbeat_freshness_secs,
            grace_period_secs: g.grace_period_secs,
            probe_thresholds: g
                .probe_thresholds
                .into_iter()
                .map(|(service, min_ok_pct)| ProbeThresholdDto {
                    service,
                    min_ok_pct,
                })
                .collect(),
        }
    }
}

impl From<HealthGateDto> for HealthGateInput {
    fn from(g: HealthGateDto) -> Self {
        HealthGateInput {
            enabled: g.enabled,
            min_heartbeat_fresh_pct: g.min_heartbeat_fresh_pct,
            heartbeat_freshness_secs: g.heartbeat_freshness_secs,
            grace_period_secs: g.grace_period_secs,
            probe_thresholds: g
                .probe_thresholds
                .into_iter()
                .map(|t| (t.service, t.min_ok_pct))
                .collect(),
        }
    }
}

// ── Rollout list DTOs ───────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RolloutListInput {}

/// Compact health rollup for a single rollout — one row, one badge, one
/// tooltip. Rendered in the list view's Health column. Reads the most
/// recent `rollout_stage_health_evaluations` row per rolling stage rather
/// than re-evaluating live (the auto-pause loop ticks every 60s).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RolloutListHealth {
    /// "pass" | "grace" | "fail" | "no_data"
    pub state: String,
    /// Stages with a gate that have at least one evaluation.
    pub evaluated_stages: u32,
    /// Stages whose last evaluation failed.
    pub failing_stages: u32,
    /// Top reason text from any failing stage, truncated. Empty when
    /// no stage failed.
    pub summary: String,
}

/// One rollout in the list view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RolloutEntry {
    pub id: Uuid,
    #[serde(default)]
    pub name: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub stage_count: i64,
    /// Latest aggregated gate state across the rollout's rolling stages.
    /// `None` when the rollout has no rolling stages or no stage with a
    /// configured health_gate (legacy rollouts).
    #[serde(default)]
    pub health: Option<RolloutListHealth>,
}

// ── Rollout list / delete handlers ──────────────────────────────────────

/// was: get_rollouts() in web/components/rollout_list.rs
#[api_mcp_dioxus_server(server = "get_rollouts")]
pub async fn rollout_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: RolloutListInput,
) -> Result<Vec<RolloutEntry>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: Option<String>,
        status: String,
        created_at: DateTime<Utc>,
        stage_count: i64,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT r.id, r.name, r.status, r.created_at, COUNT(rs.id) AS stage_count \
         FROM rollouts r LEFT JOIN rollout_stages rs ON rs.rollout_id = r.id \
         GROUP BY r.id ORDER BY r.created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    #[derive(sqlx::FromRow)]
    struct EvalRow {
        rollout_id: Uuid,
        passed: bool,
        report: serde_json::Value,
    }
    let evals: Vec<EvalRow> = sqlx::query_as(
        "SELECT DISTINCT ON (rs.id) rs.rollout_id, e.passed, e.report \
         FROM rollout_stages rs \
         JOIN rollouts r ON r.id = rs.rollout_id \
         JOIN rollout_stage_health_evaluations e ON e.stage_id = rs.id \
         WHERE r.status = 'rolling' AND rs.status = 'rolling' \
           AND rs.health_gate IS NOT NULL \
         ORDER BY rs.id, e.evaluated_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    let mut health_by_rollout: std::collections::HashMap<Uuid, RolloutListHealth> =
        std::collections::HashMap::new();
    for ev in evals {
        let in_grace = ev
            .report
            .get("in_grace_period")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let reasons_arr = ev.report.get("reasons").and_then(|v| v.as_array());
        let has_reasons = reasons_arr
            .map(|arr| arr.iter().any(|r| r.is_string()))
            .unwrap_or(false);
        let entry = health_by_rollout
            .entry(ev.rollout_id)
            .or_insert_with(|| RolloutListHealth {
                state: "pass".into(),
                evaluated_stages: 0,
                failing_stages: 0,
                summary: String::new(),
            });
        entry.evaluated_stages += 1;
        if !ev.passed {
            entry.failing_stages += 1;
            entry.state = "fail".into();
            if entry.summary.is_empty() {
                if let Some(first) =
                    reasons_arr.and_then(|arr| arr.iter().filter_map(|r| r.as_str()).next())
                {
                    entry.summary = truncate(first, 80);
                }
            }
        } else if in_grace && has_reasons && entry.state != "fail" {
            entry.state = "grace".into();
        }
    }

    Ok(rows
        .into_iter()
        .map(|r| {
            let health = if r.status == "rolling" {
                health_by_rollout.remove(&r.id)
            } else {
                None
            };
            RolloutEntry {
                id: r.id,
                name: r.name,
                status: r.status,
                created_at: r.created_at,
                stage_count: r.stage_count,
                health,
            }
        })
        .collect())
}

#[cfg(feature = "server")]
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RolloutDeleteInput {
    pub id: Uuid,
}

/// was: delete_rollout() in web/components/rollout_list.rs
#[api_mcp_dioxus_server(server = "delete_rollout")]
pub async fn rollout_delete(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: RolloutDeleteInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("DELETE FROM rollouts WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Rollout create (form pickers + create) ──────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AvailableVersionsInput {}

/// was: get_available_versions() in web/components/rollout_form.rs
#[api_mcp_dioxus_server(server = "get_available_versions")]
pub async fn rollout_available_versions(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: AvailableVersionsInput,
) -> Result<Vec<String>, ApiError> {
    p.require_admin()?;
    let versions = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT version FROM daemon_versions ORDER BY version DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(versions)
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RolloutCreateInput {
    /// Optional human-friendly rollout name.
    pub name: Option<String>,
    /// Daemon version to roll out (semver or a known channel like
    /// "rolling"). At least one of `target_version` / `nixpkgs_commit`
    /// must be set.
    pub target_version: Option<String>,
    /// Ordered stage cohorts: rollout-group UUIDs, or the sentinel
    /// "__all__" for the implicit all-clusters group.
    pub stage_ids: Vec<String>,
    /// nixpkgs commit sha (7-40 hex chars) to roll out.
    pub nixpkgs_commit: Option<String>,
    /// Health gate applied to every stage; omit (or enabled=false) for
    /// no gate.
    pub gate: Option<HealthGateDto>,
    /// Gradual-release window per stage, in minutes; omit for instant.
    pub ramp_minutes: Option<i32>,
}

/// was: create_rollout() in web/components/rollout_form.rs
///
/// `stage_ids` is an ordered list of group UUIDs or `"__all__"` sentinel.
/// `"__all__"` maps to the nil UUID (`00000000-…`) sentinel in
/// `rollout_stages.group_id`; queries that resolve stage members use a
/// subquery for all clusters when the sentinel is present instead of
/// joining through `rollout_group_members`.
#[api_mcp_dioxus_server(server = "create_rollout")]
pub async fn rollout_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: RolloutCreateInput,
) -> Result<String, ApiError> {
    p.require_admin()?;

    let target_version = input.target_version.and_then(|v| {
        let t = v.trim().to_string();
        if t.is_empty() { None } else { Some(t) }
    });
    if let Some(v) = &target_version {
        let parts: Vec<&str> = v.split('.').collect();
        let semver = parts.len() >= 3 && parts.iter().all(|p| p.parse::<u64>().is_ok());
        if !semver {
            // Non-semver channel versions ("rolling") are allowed when
            // they exist in daemon_versions (synced from xzar).
            let known: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM daemon_versions WHERE version = $1)",
            )
            .bind(v)
            .fetch_one(pool)
            .await
            .map_err(internal)?;
            if !known {
                return Err(ApiError::bad_request(
                    "version must be semver (e.g., 0.1.6) or a known channel (e.g., rolling)",
                ));
            }
        }
    }

    if input.stage_ids.is_empty() {
        return Err(ApiError::bad_request("select at least one stage"));
    }

    let nixpkgs_commit = input.nixpkgs_commit.and_then(|c| {
        let t = c.trim().to_string();
        if t.is_empty() { None } else { Some(t) }
    });
    if let Some(c) = &nixpkgs_commit {
        let valid = (7..=40).contains(&c.len()) && c.chars().all(|ch| ch.is_ascii_hexdigit());
        if !valid {
            return Err(ApiError::bad_request("nixpkgs commit must be 7-40 hex chars"));
        }
    }

    if target_version.is_none() && nixpkgs_commit.is_none() {
        return Err(ApiError::bad_request(
            "set at least one of target version or nixpkgs commit",
        ));
    }

    if matches!(input.ramp_minutes, Some(m) if m <= 0) {
        return Err(ApiError::bad_request("ramp duration must be positive"));
    }

    let mut tx = pool.begin().await.map_err(internal)?;

    let mut resolved: Vec<Uuid> = Vec::with_capacity(input.stage_ids.len());
    for sid in &input.stage_ids {
        if sid == "__all__" {
            resolved.push(Uuid::nil());
        } else {
            let gid: Uuid = sid
                .parse()
                .map_err(|e: uuid::Error| ApiError::bad_request(e.to_string()))?;
            resolved.push(gid);
        }
    }

    if let Some(ver) = &target_version {
        #[derive(sqlx::FromRow)]
        struct DowngradeRow {
            cluster_name: String,
            pinned_version: String,
            group_name: String,
        }

        let downgrades = sqlx::query_as::<_, DowngradeRow>(
            "SELECT DISTINCT c.name AS cluster_name, c.pinned_version, rg.name AS group_name \
             FROM unnest($1::uuid[]) AS gid \
             JOIN rollout_groups rg ON rg.id = gid \
             JOIN LATERAL ( \
               SELECT cluster_id FROM rollout_group_members WHERE group_id = gid \
               UNION ALL \
               SELECT id FROM clusters WHERE gid = '00000000-0000-0000-0000-000000000000'::uuid \
             ) rgm ON true \
             JOIN clusters c ON c.id = rgm.cluster_id \
             WHERE c.pinned_version IS NOT NULL \
               AND c.pinned_version > $2 \
             ORDER BY c.name, rg.name",
        )
        .bind(&resolved)
        .bind(ver)
        .fetch_all(&mut *tx)
        .await
        .map_err(internal)?;

        if !downgrades.is_empty() {
            let mut by_cluster: std::collections::BTreeMap<String, (String, Vec<String>)> =
                std::collections::BTreeMap::new();
            for d in &downgrades {
                by_cluster
                    .entry(d.cluster_name.clone())
                    .or_insert_with(|| (d.pinned_version.clone(), Vec::new()))
                    .1
                    .push(d.group_name.clone());
            }
            let details: Vec<String> = by_cluster
                .into_iter()
                .map(|(name, (cur, groups))| format!("{name} (v{cur}, in: {})", groups.join(", ")))
                .collect();
            return Err(ApiError::bad_request(format!(
                "Would downgrade to {ver}: {}",
                details.join("; ")
            )));
        }
    }

    if let Some(nix) = &nixpkgs_commit {
        let current_commits: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT c.nixpkgs_commit \
             FROM unnest($1::uuid[]) AS gid \
             JOIN LATERAL ( \
               SELECT cluster_id FROM rollout_group_members WHERE group_id = gid \
               UNION ALL \
               SELECT id FROM clusters WHERE gid = '00000000-0000-0000-0000-000000000000'::uuid \
             ) rgm ON true \
             JOIN clusters c ON c.id = rgm.cluster_id \
             WHERE c.nixpkgs_commit IS NOT NULL",
        )
        .bind(&resolved)
        .fetch_all(&mut *tx)
        .await
        .map_err(internal)?;

        let mut all_shas: std::collections::HashSet<String> = [nix.clone()].into_iter().collect();
        all_shas.extend(current_commits.iter().cloned());
        let counts = crate::commit_count::nixpkgs_commit_counts(&all_shas).await;

        let new_count = counts
            .get(nix)
            .ok_or_else(|| ApiError::bad_request(format!("unknown nixpkgs commit {nix}")))?;
        for cur in &current_commits {
            let cur_count = counts.get(cur).ok_or_else(|| {
                ApiError::internal(format!("cannot resolve commit count for current {cur}"))
            })?;
            if new_count < cur_count {
                let short_new: String = nix.chars().take(12).collect();
                let short_cur: String = cur.chars().take(12).collect();
                return Err(ApiError::bad_request(format!(
                    "nixpkgs {short_new} (#{new_count}) is older than current {short_cur} (#{cur_count}); use rollback to downgrade"
                )));
            }
        }
    }

    let name = input
        .name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());
    let rollout_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rollouts (id, name, target_version, nixpkgs_commit) VALUES ($1, $2, $3, $4)",
    )
    .bind(rollout_id)
    .bind(&name)
    .bind(&target_version)
    .bind(&nixpkgs_commit)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;

    let gate_json: Option<serde_json::Value> = input
        .gate
        .map(HealthGateInput::from)
        .as_ref()
        .and_then(|g| g.to_json());

    for (i, gid) in resolved.iter().enumerate() {
        sqlx::query(
            "INSERT INTO rollout_stages (rollout_id, group_id, stage_order, health_gate, ramp_minutes) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(rollout_id)
        .bind(gid)
        .bind(i as i32)
        .bind(&gate_json)
        .bind(input.ramp_minutes)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    }

    tx.commit().await.map_err(internal)?;
    Ok(rollout_id.to_string())
}

// ── Rollout detail DTOs ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RolloutGetInput {
    pub id: Uuid,
}

/// Full rollout detail: target, baseline, stages with health, deliveries.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RolloutInfo {
    pub id: Uuid,
    #[serde(default)]
    pub name: Option<String>,
    pub target_version: Option<String>,
    pub nixpkgs_commit: Option<String>,
    /// Captured majority version from the cohort at start-time; null when
    /// the rollout hasn't been started yet or when the cohort had no
    /// heartbeats in the 30-minute window.
    #[serde(default)]
    pub baseline_version: Option<String>,
    #[serde(default)]
    pub baseline_nixpkgs_commit: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub stages: Vec<StageInfo>,
    /// Rollup over all stages with health gates, computed from each
    /// stage's most recent stored evaluation. `None` for non-rolling
    /// rollouts and for rollouts with no gated stages.
    #[serde(default)]
    pub health_summary: Option<RolloutHealthSummary>,
    /// Clusters that fetched this rollout's target. They keep resolving
    /// to it for the rollout's lifetime, even while paused/gated.
    #[serde(default)]
    pub deliveries: Vec<DeliveryInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeliveryInfo {
    pub cluster_id: Uuid,
    pub cluster_name: String,
    pub delivered_at: DateTime<Utc>,
}

/// Mirror of the rollout-list summary with extra aggregated metrics.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RolloutHealthSummary {
    pub state: String,
    pub evaluated_stages: u32,
    pub failing_stages: u32,
    /// Aggregated metrics across the rollout's gated stages.
    #[serde(default)]
    pub total_cohort: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_heartbeat_fresh_pct: Option<u8>,
    #[serde(default)]
    pub probe_ok_pct: std::collections::HashMap<String, u8>,
    /// Top failure reason from any failing stage. Empty when no fail.
    #[serde(default)]
    pub top_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StageInfo {
    pub id: Uuid,
    pub group_name: String,
    pub group_id: Uuid,
    pub stage_order: i32,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub healthy_count: i64,
    pub total_count: i64,
    pub upgraded_count: i64,
    pub nixpkgs_upgraded_count: i64,
    /// Latest assessment-gate evaluation for this stage, if the stage has a
    /// gate configured. Populated for every stage that has ever been
    /// evaluated — we keep showing the last result after the stage completes
    /// so operators can see why a rollout auto-paused historically.
    #[serde(default)]
    pub health: Option<StageHealthInfo>,
    /// Whether this stage has a health_gate configured at all.
    #[serde(default)]
    pub has_gate: bool,
    /// Gradual-release window in minutes; None = instant release.
    #[serde(default)]
    pub ramp_minutes: Option<i32>,
    /// Share of the group currently eligible for the target (0-100),
    /// computed server-side from started_at + ramp_minutes. None when the
    /// stage has no ramp.
    #[serde(default)]
    pub ramp_pct: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StageHealthInfo {
    pub passed: bool,
    pub in_grace_period: bool,
    pub cohort_size: u32,
    pub heartbeat_fresh_pct: u8,
    pub probe_ok_pct: std::collections::HashMap<String, u8>,
    pub reasons: Vec<String>,
    pub evaluated_at: DateTime<Utc>,
    #[serde(default)]
    pub probe_stats: std::collections::HashMap<String, ProbeStatsView>,
    #[serde(default)]
    pub sample_summary: Option<SampleSummaryView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProbeStatsView {
    pub total_runs: u32,
    pub ok_count: u32,
    pub avg_duration_ms: Option<u32>,
    pub avg_tokens_out: Option<u32>,
    pub avg_first_token_ms: Option<u32>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub last_error_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SampleSummaryView {
    pub reporting_instances: u32,
    pub avg_cpu_load_1m: f32,
    pub avg_mem_used_pct: u8,
    pub max_disk_used_pct: Option<u8>,
    pub gpu_avg_util_pct: Option<u8>,
    pub thermal_alerts: u32,
}

// ── Rollout detail handler ──────────────────────────────────────────────

/// was: get_rollout_detail() in web/components/rollout_detail.rs
#[api_mcp_dioxus_server(server = "get_rollout_detail")]
pub async fn rollout_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: RolloutGetInput,
) -> Result<RolloutInfo, ApiError> {
    p.require_admin()?;
    let rid = input.id;

    #[derive(sqlx::FromRow)]
    struct RRow {
        id: Uuid,
        name: Option<String>,
        target_version: Option<String>,
        nixpkgs_commit: Option<String>,
        baseline_version: Option<String>,
        baseline_nixpkgs_commit: Option<String>,
        status: String,
        created_at: DateTime<Utc>,
    }

    let rollout = sqlx::query_as::<_, RRow>(
        "SELECT id, name, target_version, nixpkgs_commit, baseline_version, baseline_nixpkgs_commit, status, created_at FROM rollouts WHERE id = $1",
    )
    .bind(rid)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or_else(|| ApiError::not_found("rollout not found"))?;

    #[derive(sqlx::FromRow)]
    struct SRow {
        id: Uuid,
        group_name: String,
        group_id: Uuid,
        stage_order: i32,
        status: String,
        started_at: Option<DateTime<Utc>>,
        completed_at: Option<DateTime<Utc>>,
        ramp_minutes: Option<i32>,
    }

    let stages = sqlx::query_as::<_, SRow>(
        "SELECT rs.id, rg.name AS group_name, rs.group_id, rs.stage_order, rs.status, \
         rs.started_at, rs.completed_at, rs.ramp_minutes \
         FROM rollout_stages rs JOIN rollout_groups rg ON rg.id = rs.group_id \
         WHERE rs.rollout_id = $1 ORDER BY rs.stage_order",
    )
    .bind(rid)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    // Get per-group health + upgrade progress from heartbeats
    #[derive(sqlx::FromRow)]
    struct HealthRow {
        stage_order: i32,
        total: i64,
        healthy: i64,
        upgraded: i64,
        nixpkgs_upgraded: i64,
    }

    let health = sqlx::query_as::<_, HealthRow>(
        "SELECT rs.stage_order, \
         COUNT(DISTINCT dh.instance_id) AS total, \
         COUNT(DISTINCT dh.instance_id) FILTER (WHERE dh.reported_at > now() - interval '5 minutes') AS healthy, \
         COUNT(DISTINCT dh.instance_id) FILTER (WHERE dh.version = $2 AND dh.reported_at > now() - interval '5 minutes') AS upgraded, \
         COUNT(DISTINCT dh.instance_id) FILTER (WHERE dh.nixpkgs_commit = $3 AND dh.reported_at > now() - interval '5 minutes') AS nixpkgs_upgraded \
         FROM rollout_stages rs \
         JOIN LATERAL ( \
           SELECT cluster_id FROM rollout_group_members WHERE group_id = rs.group_id \
           UNION ALL \
           SELECT id FROM clusters WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
         ) rgm ON true \
         LEFT JOIN daemon_heartbeats dh ON dh.cluster_id = rgm.cluster_id \
         WHERE rs.rollout_id = $1 \
         GROUP BY rs.stage_order",
    )
    .bind(rid)
    .bind(rollout.target_version.as_deref().unwrap_or(""))
    .bind(rollout.nixpkgs_commit.as_deref().unwrap_or(""))
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let health_map: std::collections::HashMap<i32, (i64, i64, i64, i64)> = health
        .into_iter()
        .map(|h| {
            (
                h.stage_order,
                (h.healthy, h.total, h.upgraded, h.nixpkgs_upgraded),
            )
        })
        .collect();

    // Per-stage latest gate evaluation.
    #[derive(sqlx::FromRow)]
    struct GateRow {
        stage_id: Uuid,
        has_gate: bool,
        last_report: Option<serde_json::Value>,
        last_evaluated_at: Option<DateTime<Utc>>,
    }
    let gate_rows = sqlx::query_as::<_, GateRow>(
        "SELECT rs.id AS stage_id, \
                (rs.health_gate IS NOT NULL) AS has_gate, \
                e.report AS last_report, \
                e.evaluated_at AS last_evaluated_at \
         FROM rollout_stages rs \
         LEFT JOIN LATERAL ( \
             SELECT report, evaluated_at FROM rollout_stage_health_evaluations \
             WHERE stage_id = rs.id \
             ORDER BY evaluated_at DESC LIMIT 1 \
         ) e ON true \
         WHERE rs.rollout_id = $1",
    )
    .bind(rid)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut gate_map: std::collections::HashMap<Uuid, (bool, Option<StageHealthInfo>)> =
        std::collections::HashMap::new();
    for g in gate_rows {
        let info = match (g.last_report, g.last_evaluated_at) {
            (Some(r), Some(at)) => parse_eval_report(r, at),
            _ => None,
        };
        gate_map.insert(g.stage_id, (g.has_gate, info));
    }

    let health_summary = aggregate_health_summary(pool, rid, &rollout.status)
        .await
        .ok()
        .flatten();

    #[derive(sqlx::FromRow)]
    struct DeliveryRow {
        cluster_id: Uuid,
        cluster_name: String,
        delivered_at: DateTime<Utc>,
    }
    let deliveries = sqlx::query_as::<_, DeliveryRow>(
        "SELECT rd.cluster_id, c.name AS cluster_name, rd.delivered_at \
         FROM rollout_deliveries rd JOIN clusters c ON c.id = rd.cluster_id \
         WHERE rd.rollout_id = $1 ORDER BY rd.delivered_at",
    )
    .bind(rid)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|d| DeliveryInfo {
        cluster_id: d.cluster_id,
        cluster_name: d.cluster_name,
        delivered_at: d.delivered_at,
    })
    .collect();

    Ok(RolloutInfo {
        id: rollout.id,
        name: rollout.name,
        target_version: rollout.target_version,
        nixpkgs_commit: rollout.nixpkgs_commit,
        baseline_version: rollout.baseline_version,
        baseline_nixpkgs_commit: rollout.baseline_nixpkgs_commit,
        status: rollout.status,
        created_at: rollout.created_at,
        health_summary,
        deliveries,
        stages: stages
            .into_iter()
            .map(|s| {
                let (healthy, total, upgraded, nixpkgs_upgraded) = health_map
                    .get(&s.stage_order)
                    .copied()
                    .unwrap_or((0, 0, 0, 0));
                let (has_gate, gate_info) = gate_map.remove(&s.id).unwrap_or((false, None));
                // Mirrors the eligibility math in active_rollout_for_cluster:
                // elapsed-since-start over the ramp window, capped at 100.
                let ramp_pct = s.ramp_minutes.map(|mins| {
                    if s.status == "completed" {
                        return 100u8;
                    }
                    match s.started_at {
                        Some(at) => {
                            let elapsed = (Utc::now() - at).num_seconds().max(0) as f64;
                            (100.0 * elapsed / (mins as f64 * 60.0)).min(100.0) as u8
                        }
                        None => 0,
                    }
                });
                StageInfo {
                    id: s.id,
                    group_name: s.group_name,
                    group_id: s.group_id,
                    stage_order: s.stage_order,
                    status: s.status,
                    started_at: s.started_at,
                    completed_at: s.completed_at,
                    ramp_minutes: s.ramp_minutes,
                    ramp_pct,
                    healthy_count: healthy,
                    total_count: total,
                    upgraded_count: upgraded,
                    nixpkgs_upgraded_count: nixpkgs_upgraded,
                    health: gate_info,
                    has_gate,
                }
            })
            .collect(),
    })
}

#[cfg(feature = "server")]
fn parse_eval_report(v: serde_json::Value, at: DateTime<Utc>) -> Option<StageHealthInfo> {
    let probe_stats: std::collections::HashMap<String, ProbeStatsView> = v
        .get("probe_stats")
        .and_then(|x| x.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| {
                    serde_json::from_value::<ProbeStatsView>(v.clone())
                        .ok()
                        .map(|s| (k.clone(), s))
                })
                .collect()
        })
        .unwrap_or_default();
    let sample_summary: Option<SampleSummaryView> = v
        .get("sample_summary")
        .and_then(|x| serde_json::from_value::<SampleSummaryView>(x.clone()).ok());

    Some(StageHealthInfo {
        passed: v.get("passed").and_then(|x| x.as_bool()).unwrap_or(false),
        in_grace_period: v
            .get("in_grace_period")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        cohort_size: v.get("cohort_size").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        heartbeat_fresh_pct: v
            .get("heartbeat_fresh_pct")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u8,
        probe_ok_pct: v
            .get("probe_ok_pct")
            .and_then(|x| x.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_u64().map(|n| (k.clone(), n as u8)))
                    .collect()
            })
            .unwrap_or_default(),
        reasons: v
            .get("reasons")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| r.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        evaluated_at: at,
        probe_stats,
        sample_summary,
    })
}

/// Roll up the latest evaluation per stage into one summary for the whole
/// rollout. Returns `None` when the rollout isn't rolling, or when no
/// stage has a configured health_gate.
#[cfg(feature = "server")]
async fn aggregate_health_summary(
    pool: &sqlx::PgPool,
    rollout_id: Uuid,
    rollout_status: &str,
) -> Result<Option<RolloutHealthSummary>, sqlx::Error> {
    if rollout_status != "rolling" {
        return Ok(None);
    }

    #[derive(sqlx::FromRow)]
    struct EvalRow {
        passed: bool,
        report: serde_json::Value,
    }
    let evals: Vec<EvalRow> = sqlx::query_as(
        "SELECT DISTINCT ON (rs.id) e.passed, e.report \
         FROM rollout_stages rs \
         JOIN rollout_stage_health_evaluations e ON e.stage_id = rs.id \
         WHERE rs.rollout_id = $1 AND rs.status = 'rolling' \
           AND rs.health_gate IS NOT NULL \
         ORDER BY rs.id, e.evaluated_at DESC",
    )
    .bind(rollout_id)
    .fetch_all(pool)
    .await?;
    if evals.is_empty() {
        return Ok(None);
    }

    let mut summary = RolloutHealthSummary {
        state: "pass".into(),
        evaluated_stages: 0,
        failing_stages: 0,
        total_cohort: 0,
        avg_heartbeat_fresh_pct: None,
        probe_ok_pct: std::collections::HashMap::new(),
        top_reason: String::new(),
    };

    let mut hb_acc = 0u32;
    let mut hb_count = 0u32;
    let mut probe_acc: std::collections::HashMap<String, (u32, u32)> =
        std::collections::HashMap::new();

    for ev in &evals {
        summary.evaluated_stages += 1;
        let in_grace = ev
            .report
            .get("in_grace_period")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let reasons_iter = ev.report.get("reasons").and_then(|v| v.as_array());
        let has_reasons = reasons_iter
            .map(|arr| arr.iter().any(|r| r.is_string()))
            .unwrap_or(false);
        if !ev.passed {
            summary.failing_stages += 1;
            summary.state = "fail".into();
            if summary.top_reason.is_empty() {
                if let Some(first) =
                    reasons_iter.and_then(|arr| arr.iter().filter_map(|r| r.as_str()).next())
                {
                    summary.top_reason = first.chars().take(120).collect();
                }
            }
        } else if in_grace && has_reasons && summary.state != "fail" {
            // Grace is *shielding* a real reason — surface that. A clean
            // pass during grace is just a pass; no need for a yellow flag.
            summary.state = "grace".into();
        }
        if let Some(n) = ev.report.get("cohort_size").and_then(|v| v.as_u64()) {
            summary.total_cohort += n as u32;
        }
        if let Some(p) = ev
            .report
            .get("heartbeat_fresh_pct")
            .and_then(|v| v.as_u64())
        {
            hb_acc += p as u32;
            hb_count += 1;
        }
        if let Some(obj) = ev.report.get("probe_ok_pct").and_then(|v| v.as_object()) {
            for (svc, val) in obj {
                if let Some(p) = val.as_u64() {
                    let entry = probe_acc.entry(svc.clone()).or_insert((0, 0));
                    entry.0 += p as u32;
                    entry.1 += 1;
                }
            }
        }
    }

    if hb_count > 0 {
        summary.avg_heartbeat_fresh_pct = Some((hb_acc / hb_count).min(100) as u8);
    }
    summary.probe_ok_pct = probe_acc
        .into_iter()
        .map(|(svc, (acc, n))| (svc, ((acc / n).min(100)) as u8))
        .collect();

    Ok(Some(summary))
}

/// Compute the baseline (version, nixpkgs_commit) from the cohort's recent
/// heartbeats — the version most instances are running right now. Used to
/// snapshot what a rollout should rewind to on rollback. `None` on each
/// side if no heartbeat reports that field.
#[cfg(feature = "server")]
async fn cohort_baseline(
    pool: &sqlx::PgPool,
    rollout_id: Uuid,
) -> Result<(Option<String>, Option<String>), sqlx::Error> {
    let version: Option<String> = sqlx::query_scalar(
        "SELECT dh.version FROM daemon_heartbeats dh \
         WHERE dh.cluster_id IN ( \
             SELECT DISTINCT rgm.cluster_id FROM rollout_stages rs \
             JOIN LATERAL ( \
                 SELECT cluster_id FROM rollout_group_members WHERE group_id = rs.group_id \
                 UNION ALL \
                 SELECT id FROM clusters WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
             ) rgm ON true \
             WHERE rs.rollout_id = $1 \
         ) \
         AND dh.reported_at > now() - interval '30 minutes' \
         GROUP BY dh.version ORDER BY COUNT(*) DESC LIMIT 1",
    )
    .bind(rollout_id)
    .fetch_optional(pool)
    .await?;

    let commit: Option<String> = sqlx::query_scalar(
        "SELECT dh.nixpkgs_commit FROM daemon_heartbeats dh \
         WHERE dh.cluster_id IN ( \
             SELECT DISTINCT rgm.cluster_id FROM rollout_stages rs \
             JOIN LATERAL ( \
                 SELECT cluster_id FROM rollout_group_members WHERE group_id = rs.group_id \
                 UNION ALL \
                 SELECT id FROM clusters WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
             ) rgm ON true \
             WHERE rs.rollout_id = $1 \
         ) \
         AND dh.reported_at > now() - interval '30 minutes' \
         AND dh.nixpkgs_commit IS NOT NULL \
         GROUP BY dh.nixpkgs_commit ORDER BY COUNT(*) DESC LIMIT 1",
    )
    .bind(rollout_id)
    .fetch_optional(pool)
    .await?;

    Ok((version, commit))
}

// ── Stage gates ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StageGateGetInput {
    pub stage_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StageGateUpdateInput {
    pub stage_id: Uuid,
    /// New gate config; omit (or enabled=false) to remove the gate.
    pub gate: Option<HealthGateDto>,
    /// Apply the same gate to every stage of the rollout.
    pub apply_to_all: bool,
}

/// was: get_stage_gate() in web/components/rollout_detail.rs
///
/// Load one stage's current gate config. Returns `None` when the stage has
/// no gate (renders as "disabled" toggle in the form so the operator can
/// turn one on).
#[api_mcp_dioxus_server(server = "get_stage_gate")]
pub async fn rollout_stage_gate_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: StageGateGetInput,
) -> Result<Option<HealthGateDto>, ApiError> {
    p.require_admin()?;
    let gate: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT health_gate FROM rollout_stages WHERE id = $1")
            .bind(input.stage_id)
            .fetch_optional(pool)
            .await
            .map_err(internal)?
            .ok_or_else(|| ApiError::not_found("stage not found"))?;
    Ok(gate
        .as_ref()
        .map(|v| HealthGateDto::from(HealthGateInput::from_json(v))))
}

/// was: update_stage_gate() in web/components/rollout_detail.rs
///
/// Persist a gate update. When `apply_to_all` is true the payload is
/// written to every stage of the rollout so operators can ratchet one
/// threshold across the board without editing each stage individually.
#[api_mcp_dioxus_server(server = "update_stage_gate")]
pub async fn rollout_stage_gate_update(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: StageGateUpdateInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    let sid = input.stage_id;

    // `None` or enabled=false both serialise to SQL NULL (= no gate).
    let gate_json: Option<serde_json::Value> = input
        .gate
        .map(HealthGateInput::from)
        .as_ref()
        .and_then(|g| g.to_json());

    if input.apply_to_all {
        let rollout_id: Uuid =
            sqlx::query_scalar("SELECT rollout_id FROM rollout_stages WHERE id = $1")
                .bind(sid)
                .fetch_optional(pool)
                .await
                .map_err(internal)?
                .ok_or_else(|| ApiError::not_found("stage not found"))?;
        sqlx::query("UPDATE rollout_stages SET health_gate = $1 WHERE rollout_id = $2")
            .bind(&gate_json)
            .bind(rollout_id)
            .execute(pool)
            .await
            .map_err(internal)?;
    } else {
        sqlx::query("UPDATE rollout_stages SET health_gate = $1 WHERE id = $2")
            .bind(&gate_json)
            .bind(sid)
            .execute(pool)
            .await
            .map_err(internal)?;
    }
    Ok(())
}

// ── Stage pushes / re-evaluation ────────────────────────────────────────

/// Result of a stage-cohort push. `cohort_size` is the number of clusters
/// targeted by the stage; `dispatched` is the subset that actually had an
/// open SSE channel to receive the push. A value of 0 almost always means
/// no daemon in the cohort is currently connected — the most common
/// reason an operator clicks the button and sees nothing.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RequestAssessmentResult {
    pub cohort_size: u32,
    pub dispatched: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StagePushInput {
    pub stage_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StageReevaluateInput {
    pub stage_id: Uuid,
}

/// Push one message to every currently-connected daemon in the stage's
/// cohort. `broadcast::Sender::send` returns Err when there are no active
/// receivers — a daemon that connected then closed its SSE. Treat that as
/// "not dispatched" so the UI counter is honest.
#[cfg(feature = "server")]
async fn push_to_stage_cohort(
    pool: &sqlx::PgPool,
    stage_id: Uuid,
    msg: crate::api::push::PushMessage,
) -> Result<RequestAssessmentResult, ApiError> {
    let group_id: Uuid = sqlx::query_scalar("SELECT group_id FROM rollout_stages WHERE id = $1")
        .bind(stage_id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or_else(|| ApiError::not_found("stage not found"))?;

    let cohort: Vec<Uuid> = sqlx::query_scalar(
        "SELECT cluster_id FROM rollout_group_members WHERE group_id = $1 \
         UNION ALL \
         SELECT id FROM clusters WHERE $1 = '00000000-0000-0000-0000-000000000000'::uuid",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    let cohort_size = cohort.len() as u32;

    let channels = crate::push_channels().map_err(internal)?;
    let map = channels.read().await;
    let mut dispatched = 0u32;
    for cid in cohort {
        if let Some(tx) = map.get(&cid) {
            if tx.send(msg.clone()).is_ok() {
                dispatched += 1;
            }
        }
    }
    Ok(RequestAssessmentResult {
        cohort_size,
        dispatched,
    })
}

/// was: request_stage_assessment() in web/components/rollout_detail.rs
#[api_mcp_dioxus_server(server = "request_stage_assessment")]
pub async fn rollout_stage_request_assessment(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: StagePushInput,
) -> Result<RequestAssessmentResult, ApiError> {
    p.require_admin()?;
    push_to_stage_cohort(
        pool,
        input.stage_id,
        crate::api::push::PushMessage::RequestAssessment,
    )
    .await
}

/// was: trigger_stage_self_update() in web/components/rollout_detail.rs
#[api_mcp_dioxus_server(server = "trigger_stage_self_update")]
pub async fn rollout_stage_self_update(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: StagePushInput,
) -> Result<RequestAssessmentResult, ApiError> {
    p.require_admin()?;
    push_to_stage_cohort(
        pool,
        input.stage_id,
        crate::api::push::PushMessage::SelfUpdate,
    )
    .await
}

/// was: trigger_stage_sync_nixpkgs() in web/components/rollout_detail.rs
#[api_mcp_dioxus_server(server = "trigger_stage_sync_nixpkgs")]
pub async fn rollout_stage_sync_nixpkgs(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: StagePushInput,
) -> Result<RequestAssessmentResult, ApiError> {
    p.require_admin()?;
    push_to_stage_cohort(
        pool,
        input.stage_id,
        crate::api::push::PushMessage::SyncNixpkgs,
    )
    .await
}

/// was: reevaluate_stage() in web/components/rollout_detail.rs
///
/// Re-evaluate this stage *and every other rolling stage* of the same
/// rollout. A per-stage button suggests per-stage scope, but the rollout
/// detail page renders both a stage panel and a rollout-wide summary
/// that aggregates across every rolling stage. Evaluating just one
/// would leave the summary showing stale data from the 60s auto-pause
/// loop's last pass. Evaluate all so the whole page refreshes coherently.
#[api_mcp_dioxus_server(server = "reevaluate_stage")]
pub async fn rollout_stage_reevaluate(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: StageReevaluateInput,
) -> Result<(), ApiError> {
    p.require_admin()?;

    let rollout_id: Uuid =
        sqlx::query_scalar("SELECT rollout_id FROM rollout_stages WHERE id = $1")
            .bind(input.stage_id)
            .fetch_optional(pool)
            .await
            .map_err(internal)?
            .ok_or_else(|| ApiError::not_found("stage not found"))?;

    evaluate_all_rolling_stages(pool, rollout_id)
        .await
        .map_err(internal)?;
    Ok(())
}

/// Seed a stored evaluation for every currently-rolling stage in this
/// rollout so the UI's health card renders right away rather than
/// waiting up to 60s for the auto-pause loop. Non-fatal: caller uses
/// `let _ =` so a failed evaluation doesn't block the state transition.
#[cfg(feature = "server")]
async fn evaluate_all_rolling_stages(
    pool: &sqlx::PgPool,
    rollout_id: Uuid,
) -> Result<(), sqlx::Error> {
    let stage_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM rollout_stages WHERE rollout_id = $1 AND status = 'rolling'",
    )
    .bind(rollout_id)
    .fetch_all(pool)
    .await?;
    for sid in stage_ids {
        // Ignore errors for any one stage — a degraded gate-eval path
        // shouldn't stop the rest of the rollout from getting its data.
        let _ = crate::rollout_health::evaluate_and_store(pool, sid).await;
    }
    Ok(())
}

// ── Rollout lifecycle actions ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RolloutActionInput {
    pub id: Uuid,
    /// One of: "start", "advance", "pause", "resume", "complete",
    /// "rollback", "delete".
    pub action: String,
}

/// was: rollout_action() in web/components/rollout_detail.rs
#[api_mcp_dioxus_server(server = "rollout_action")]
pub async fn rollout_action_apply(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: RolloutActionInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    let rid = input.id;

    match input.action.as_str() {
        "start" => {
            // Capture the cohort's current majority version + nixpkgs commit
            // before we start rolling — this is what "rollback" will restore.
            // Skip if baseline is already set (e.g. stopped-then-restarted
            // rollout) to avoid overwriting with post-partial-rollout noise.
            let (baseline_version, baseline_commit) =
                cohort_baseline(pool, rid).await.unwrap_or((None, None));

            let mut tx = pool.begin().await.map_err(internal)?;
            sqlx::query(
                "UPDATE rollouts SET status = 'rolling', updated_at = now(), \
                    baseline_version = COALESCE(baseline_version, $2), \
                    baseline_nixpkgs_commit = COALESCE(baseline_nixpkgs_commit, $3) \
                 WHERE id = $1",
            )
            .bind(rid)
            .bind(&baseline_version)
            .bind(&baseline_commit)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            sqlx::query("UPDATE rollout_stages SET status = 'rolling', started_at = now() WHERE rollout_id = $1 AND stage_order = 0")
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            tx.commit().await.map_err(internal)?;
            // Seed a first evaluation so the Rollout health card renders
            // immediately instead of waiting for the 60s auto-pause tick.
            let _ = evaluate_all_rolling_stages(pool, rid).await;
            crate::api::push::notify_rollout_global(rid, crate::api::push::PushMessage::SelfUpdate)
                .await;
            crate::api::push::notify_rollout_global(
                rid,
                crate::api::push::PushMessage::SyncNixpkgs,
            )
            .await;
        }
        "rollback" => {
            // Fetch baseline + cohort ids so we can rewind cluster pins.
            #[derive(sqlx::FromRow)]
            struct BaselineRow {
                baseline_version: Option<String>,
                baseline_nixpkgs_commit: Option<String>,
            }
            let baseline: BaselineRow = sqlx::query_as(
                "SELECT baseline_version, baseline_nixpkgs_commit FROM rollouts WHERE id = $1",
            )
            .bind(rid)
            .fetch_optional(pool)
            .await
            .map_err(internal)?
            .ok_or_else(|| ApiError::not_found("rollout not found"))?;

            let cohort: Vec<uuid::Uuid> = sqlx::query_scalar(
                "SELECT DISTINCT rgm.cluster_id FROM rollout_stages rs \
                 JOIN LATERAL ( \
                   SELECT cluster_id FROM rollout_group_members WHERE group_id = rs.group_id \
                   UNION ALL \
                   SELECT id FROM clusters WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
                 ) rgm ON true \
                 WHERE rs.rollout_id = $1",
            )
            .bind(rid)
            .fetch_all(pool)
            .await
            .map_err(internal)?;

            let mut tx = pool.begin().await.map_err(internal)?;
            // Mark every stage that actually shipped (rolling/paused/completed)
            // as rolled_back. Pending stages stay pending — they never sent
            // anything out, so there's nothing to undo.
            sqlx::query(
                "UPDATE rollout_stages SET status = 'rolled_back' \
                 WHERE rollout_id = $1 AND status IN ('rolling', 'paused', 'completed')",
            )
            .bind(rid)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            sqlx::query(
                "UPDATE rollouts SET status = 'rolled_back', updated_at = now() WHERE id = $1",
            )
            .bind(rid)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            // Rewind cluster pins to the captured baseline. A NULL baseline
            // clears the pin so the cluster falls back to the latest
            // daemon_versions row — which matches the no-rollout default.
            sqlx::query(
                "UPDATE clusters SET pinned_version = $1, nixpkgs_commit = $2 \
                 WHERE id = ANY($3)",
            )
            .bind(&baseline.baseline_version)
            .bind(&baseline.baseline_nixpkgs_commit)
            .bind(&cohort)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            tx.commit().await.map_err(internal)?;
            // Tell the cohort to refetch both so daemons that already took
            // the bad version downgrade, and daemons that hadn't yet stay
            // put.
            crate::api::push::notify_all_rollout_global(
                rid,
                crate::api::push::PushMessage::SelfUpdate,
            )
            .await;
            crate::api::push::notify_all_rollout_global(
                rid,
                crate::api::push::PushMessage::SyncNixpkgs,
            )
            .await;
        }
        "advance" => {
            #[derive(sqlx::FromRow)]
            struct SO {
                stage_order: i32,
            }
            let current = sqlx::query_as::<_, SO>(
                "SELECT stage_order FROM rollout_stages WHERE rollout_id = $1 AND status = 'rolling' LIMIT 1",
            )
            .bind(rid)
            .fetch_one(pool)
            .await
            .map_err(internal)?;

            let mut tx = pool.begin().await.map_err(internal)?;
            sqlx::query("UPDATE rollout_stages SET status = 'completed', completed_at = now() WHERE rollout_id = $1 AND stage_order = $2")
                .bind(rid)
                .bind(current.stage_order)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            let next = current.stage_order + 1;
            let updated = sqlx::query("UPDATE rollout_stages SET status = 'rolling', started_at = now() WHERE rollout_id = $1 AND stage_order = $2")
                .bind(rid)
                .bind(next)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            if updated.rows_affected() == 0 {
                sqlx::query(
                    "UPDATE rollouts SET status = 'completed', updated_at = now() WHERE id = $1",
                )
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            } else {
                sqlx::query("UPDATE rollouts SET updated_at = now() WHERE id = $1")
                    .bind(rid)
                    .execute(&mut *tx)
                    .await
                    .map_err(internal)?;
            }
            tx.commit().await.map_err(internal)?;
            // Seed evaluation for the newly-rolling stage.
            let _ = evaluate_all_rolling_stages(pool, rid).await;
            crate::api::push::notify_rollout_global(rid, crate::api::push::PushMessage::SelfUpdate)
                .await;
            crate::api::push::notify_rollout_global(
                rid,
                crate::api::push::PushMessage::SyncNixpkgs,
            )
            .await;
        }
        "pause" => {
            let mut tx = pool.begin().await.map_err(internal)?;
            sqlx::query("UPDATE rollout_stages SET status = 'paused' WHERE rollout_id = $1 AND status = 'rolling'")
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            sqlx::query("UPDATE rollouts SET status = 'paused', updated_at = now() WHERE id = $1")
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            tx.commit().await.map_err(internal)?;
        }
        "resume" => {
            let mut tx = pool.begin().await.map_err(internal)?;
            sqlx::query("UPDATE rollout_stages SET status = 'rolling' WHERE rollout_id = $1 AND status = 'paused'")
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            sqlx::query("UPDATE rollouts SET status = 'rolling', updated_at = now() WHERE id = $1")
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            tx.commit().await.map_err(internal)?;
            // Seed an evaluation for every stage that's resumed so the
            // health card comes back without waiting for the next tick.
            let _ = evaluate_all_rolling_stages(pool, rid).await;
            crate::api::push::notify_rollout_global(rid, crate::api::push::PushMessage::SelfUpdate)
                .await;
            crate::api::push::notify_rollout_global(
                rid,
                crate::api::push::PushMessage::SyncNixpkgs,
            )
            .await;
        }
        "complete" => {
            let (target_version, nixpkgs_commit): (Option<String>, Option<String>) =
                sqlx::query_as("SELECT target_version, nixpkgs_commit FROM rollouts WHERE id = $1")
                    .bind(rid)
                    .fetch_one(pool)
                    .await
                    .map_err(internal)?;

            let mut tx = pool.begin().await.map_err(internal)?;
            sqlx::query("UPDATE rollout_stages SET status = 'completed', completed_at = COALESCE(completed_at, now()) WHERE rollout_id = $1")
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            sqlx::query(
                "UPDATE rollouts SET status = 'completed', updated_at = now() WHERE id = $1",
            )
            .bind(rid)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            if let Some(version) = &target_version {
                sqlx::query(
                    "UPDATE clusters SET pinned_version = $1 WHERE id IN (\
                     SELECT DISTINCT rgm.cluster_id FROM rollout_stages rs \
                     JOIN LATERAL ( \
                       SELECT cluster_id FROM rollout_group_members WHERE group_id = rs.group_id \
                       UNION ALL \
                       SELECT id FROM clusters WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
                     ) rgm ON true \
                     WHERE rs.rollout_id = $2)",
                )
                .bind(version)
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            }
            if let Some(commit) = &nixpkgs_commit {
                sqlx::query(
                    "UPDATE clusters SET nixpkgs_commit = $1 WHERE id IN (\
                     SELECT DISTINCT rgm.cluster_id FROM rollout_stages rs \
                     JOIN LATERAL ( \
                       SELECT cluster_id FROM rollout_group_members WHERE group_id = rs.group_id \
                       UNION ALL \
                       SELECT id AS cluster_id FROM clusters WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
                     ) rgm ON true \
                     WHERE rs.rollout_id = $2)",
                )
                .bind(commit)
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            }
            tx.commit().await.map_err(internal)?;
            crate::api::push::notify_all_rollout_global(
                rid,
                crate::api::push::PushMessage::SelfUpdate,
            )
            .await;
            if nixpkgs_commit.is_some() {
                crate::api::push::notify_all_rollout_global(
                    rid,
                    crate::api::push::PushMessage::SyncNixpkgs,
                )
                .await;
            }
        }
        "delete" => {
            sqlx::query("DELETE FROM rollouts WHERE id = $1")
                .bind(rid)
                .execute(pool)
                .await
                .map_err(internal)?;
        }
        _ => return Err(ApiError::bad_request("unknown action")),
    }

    Ok(())
}

// ── Rollout groups: DTOs ────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GroupListInput {}

/// One rollout group in the list view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GroupEntry {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub member_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GroupCreateInput {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GroupOptionsInput {}

/// A rollout group as a picker option (rollout-form stage selector).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GroupOption {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GroupGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GroupInfo {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub members: Vec<MemberEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemberEntry {
    pub member_id: Uuid,
    pub cluster_id: Uuid,
    pub cluster_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterOption {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GroupDeleteInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GroupAvailableClustersInput {
    pub group_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemberAddInput {
    pub group_id: Uuid,
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemberAddAllInput {
    pub group_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemberRemoveInput {
    pub member_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GroupSetDescriptionInput {
    pub group_id: Uuid,
    pub description: String,
}

// ── Rollout groups: handlers ────────────────────────────────────────────

/// was: get_rollout_groups() in web/components/rollout_group_list.rs
#[api_mcp_dioxus_server(server = "get_rollout_groups")]
pub async fn group_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: GroupListInput,
) -> Result<Vec<GroupEntry>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
        description: String,
        member_count: i64,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT rg.id, rg.name, rg.description, COUNT(rgm.id) AS member_count \
         FROM rollout_groups rg LEFT JOIN rollout_group_members rgm ON rgm.group_id = rg.id \
         WHERE rg.id != '00000000-0000-0000-0000-000000000000'::uuid \
         GROUP BY rg.id ORDER BY rg.name",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| GroupEntry {
            id: r.id,
            name: r.name,
            description: r.description,
            member_count: r.member_count,
        })
        .collect())
}

/// was: create_group() in web/components/rollout_group_list.rs
#[api_mcp_dioxus_server(server = "create_group")]
pub async fn group_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: GroupCreateInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("INSERT INTO rollout_groups (name, description) VALUES ($1, $2)")
        .bind(&input.name)
        .bind(&input.description)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

/// was: get_group_options() in web/components/rollout_form.rs
#[api_mcp_dioxus_server(server = "get_group_options")]
pub async fn group_options(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: GroupOptionsInput,
) -> Result<Vec<GroupOption>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name FROM rollout_groups \
         WHERE id != '00000000-0000-0000-0000-000000000000'::uuid \
         ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| GroupOption {
            id: r.id,
            name: r.name,
        })
        .collect())
}

/// was: get_group_detail() in web/components/rollout_group_detail.rs
#[api_mcp_dioxus_server(server = "get_group_detail")]
pub async fn group_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: GroupGetInput,
) -> Result<GroupInfo, ApiError> {
    p.require_admin()?;
    let gid = input.id;
    if gid == Uuid::nil() {
        return Err(ApiError::bad_request(
            "the all-clusters group is implicit and has no detail page",
        ));
    }

    #[derive(sqlx::FromRow)]
    struct GRow {
        id: Uuid,
        name: String,
        description: String,
    }

    let group =
        sqlx::query_as::<_, GRow>("SELECT id, name, description FROM rollout_groups WHERE id = $1")
            .bind(gid)
            .fetch_optional(pool)
            .await
            .map_err(internal)?
            .ok_or_else(|| ApiError::not_found("rollout group not found"))?;

    #[derive(sqlx::FromRow)]
    struct MRow {
        member_id: Uuid,
        cluster_id: Uuid,
        cluster_name: String,
    }

    let members = sqlx::query_as::<_, MRow>(
        "SELECT rgm.id AS member_id, rgm.cluster_id, c.name AS cluster_name \
         FROM rollout_group_members rgm \
         JOIN clusters c ON c.id = rgm.cluster_id \
         WHERE rgm.group_id = $1 ORDER BY c.name",
    )
    .bind(gid)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(GroupInfo {
        id: group.id,
        name: group.name,
        description: group.description,
        members: members
            .into_iter()
            .map(|m| MemberEntry {
                member_id: m.member_id,
                cluster_id: m.cluster_id,
                cluster_name: m.cluster_name,
            })
            .collect(),
    })
}

/// was: get_available_clusters() in web/components/rollout_group_detail.rs
#[api_mcp_dioxus_server(server = "get_available_clusters")]
pub async fn group_available_clusters(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: GroupAvailableClustersInput,
) -> Result<Vec<ClusterOption>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name FROM clusters \
         WHERE id NOT IN (SELECT cluster_id FROM rollout_group_members WHERE group_id = $1) \
         ORDER BY name",
    )
    .bind(input.group_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| ClusterOption {
            id: r.id,
            name: r.name,
        })
        .collect())
}

/// was: add_member() in web/components/rollout_group_detail.rs
#[api_mcp_dioxus_server(server = "add_member")]
pub async fn group_member_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: MemberAddInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("INSERT INTO rollout_group_members (group_id, cluster_id) VALUES ($1, $2)")
        .bind(input.group_id)
        .bind(input.cluster_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

/// was: add_all_clusters() in web/components/rollout_group_detail.rs
#[api_mcp_dioxus_server(server = "add_all_clusters")]
pub async fn group_member_add_all(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: MemberAddAllInput,
) -> Result<u64, ApiError> {
    p.require_admin()?;
    let result = sqlx::query(
        "INSERT INTO rollout_group_members (group_id, cluster_id) \
         SELECT $1, id FROM clusters \
         WHERE id NOT IN (SELECT cluster_id FROM rollout_group_members WHERE group_id = $1) \
         ON CONFLICT DO NOTHING",
    )
    .bind(input.group_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(result.rows_affected())
}

/// was: remove_member() in web/components/rollout_group_detail.rs
#[api_mcp_dioxus_server(server = "remove_member")]
pub async fn group_member_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: MemberRemoveInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("DELETE FROM rollout_group_members WHERE id = $1")
        .bind(input.member_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

/// was: update_description() in web/components/rollout_group_detail.rs
#[api_mcp_dioxus_server(server = "update_description")]
pub async fn group_set_description(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: GroupSetDescriptionInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("UPDATE rollout_groups SET description = $2 WHERE id = $1")
        .bind(input.group_id)
        .bind(&input.description)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

/// was: delete_group() in web/components/rollout_group_detail.rs
#[api_mcp_dioxus_server(server = "delete_group")]
pub async fn group_delete(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: GroupDeleteInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    if input.id.is_nil() {
        return Err(ApiError::bad_request(
            "the All Clusters group cannot be deleted",
        ));
    }
    sqlx::query("DELETE FROM rollout_groups WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Registration ────────────────────────────────────────────────────────
