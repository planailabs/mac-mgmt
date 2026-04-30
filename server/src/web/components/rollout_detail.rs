use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;
use crate::web::components::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, ErrorText, HelpText,
    SectionHeading,
};
use crate::web::gate_input::HealthGateInput;
#[cfg(feature = "server")]
use crate::web::user::current_user;

fn status_variant(status: &str) -> BadgeVariant {
    match status {
        "rolling" => BadgeVariant::Info,
        "completed" => BadgeVariant::Success,
        "paused" | "rolled_back" => BadgeVariant::Warn,
        "failed" => BadgeVariant::Danger,
        _ => BadgeVariant::Neutral,
    }
}

fn health_state_variant(state: &str) -> BadgeVariant {
    match state {
        "pass" => BadgeVariant::Success,
        "fail" => BadgeVariant::Danger,
        "grace" => BadgeVariant::Warn,
        _ => BadgeVariant::Neutral,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RolloutInfo {
    id: Uuid,
    #[serde(default)]
    name: Option<String>,
    target_version: Option<String>,
    nixpkgs_commit: Option<String>,
    /// Captured majority version from the cohort at start-time; null when
    /// the rollout hasn't been started yet or when the cohort had no
    /// heartbeats in the 30-minute window.
    #[serde(default)]
    baseline_version: Option<String>,
    #[serde(default)]
    baseline_nixpkgs_commit: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    stages: Vec<StageInfo>,
    /// Rollup over all stages with health gates, computed from each
    /// stage's most recent stored evaluation. `None` for non-rolling
    /// rollouts and for rollouts with no gated stages.
    #[serde(default)]
    health_summary: Option<RolloutHealthSummary>,
}

/// Mirror of the rollout-list summary so the same renderer can be reused
/// here. Both pages stay in sync if the field set evolves.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RolloutHealthSummary {
    state: String,
    evaluated_stages: u32,
    failing_stages: u32,
    /// Aggregated metrics across the rollout's gated stages.
    #[serde(default)]
    total_cohort: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    avg_heartbeat_fresh_pct: Option<u8>,
    #[serde(default)]
    probe_ok_pct: std::collections::HashMap<String, u8>,
    /// Top failure reason from any failing stage. Empty when no fail.
    #[serde(default)]
    top_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StageInfo {
    id: Uuid,
    group_name: String,
    group_id: Uuid,
    stage_order: i32,
    status: String,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    healthy_count: i64,
    total_count: i64,
    upgraded_count: i64,
    nixpkgs_upgraded_count: i64,
    /// Latest assessment-gate evaluation for this stage, if the stage has a
    /// gate configured. Populated for every stage that has ever been
    /// evaluated — we keep showing the last result after the stage completes
    /// so operators can see why a rollout auto-paused historically.
    #[serde(default)]
    health: Option<StageHealthInfo>,
    /// Whether this stage has a health_gate configured at all.
    #[serde(default)]
    has_gate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StageHealthInfo {
    passed: bool,
    in_grace_period: bool,
    cohort_size: u32,
    heartbeat_fresh_pct: u8,
    probe_ok_pct: std::collections::HashMap<String, u8>,
    reasons: Vec<String>,
    evaluated_at: DateTime<Utc>,
    #[serde(default)]
    probe_stats: std::collections::HashMap<String, ProbeStatsView>,
    #[serde(default)]
    sample_summary: Option<SampleSummaryView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProbeStatsView {
    total_runs: u32,
    ok_count: u32,
    avg_duration_ms: Option<u32>,
    avg_tokens_out: Option<u32>,
    avg_first_token_ms: Option<u32>,
    last_failure_at: Option<DateTime<Utc>>,
    last_error_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SampleSummaryView {
    reporting_instances: u32,
    avg_cpu_load_1m: f32,
    avg_mem_used_pct: u8,
    max_disk_used_pct: Option<u8>,
    gpu_avg_util_pct: Option<u8>,
    thermal_alerts: u32,
}

#[server]
async fn get_nixpkgs_commit_count_rollout(sha: String) -> Result<Option<u64>, ServerFnError> {
    let shas = std::collections::HashSet::from([sha.clone()]);
    let counts = crate::commit_count::nixpkgs_commit_counts(&shas).await;
    Ok(counts.get(&sha).copied())
}

#[server]
async fn get_rollout_detail(id: String) -> Result<RolloutInfo, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let rid: Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

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
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct SRow {
        id: Uuid,
        group_name: String,
        group_id: Uuid,
        stage_order: i32,
        status: String,
        started_at: Option<DateTime<Utc>>,
        completed_at: Option<DateTime<Utc>>,
    }

    let stages = sqlx::query_as::<_, SRow>(
        "SELECT rs.id, rg.name AS group_name, rs.group_id, rs.stage_order, rs.status, \
         rs.started_at, rs.completed_at \
         FROM rollout_stages rs JOIN rollout_groups rg ON rg.id = rs.group_id \
         WHERE rs.rollout_id = $1 ORDER BY rs.stage_order",
    )
    .bind(rid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

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
    .fetch_all(&pool)
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
    .fetch_all(&pool)
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

    let health_summary = aggregate_health_summary(&pool, rid, &rollout.status)
        .await
        .ok()
        .flatten();

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
        stages: stages
            .into_iter()
            .map(|s| {
                let (healthy, total, upgraded, nixpkgs_upgraded) = health_map
                    .get(&s.stage_order)
                    .copied()
                    .unwrap_or((0, 0, 0, 0));
                let (has_gate, gate_info) = gate_map.remove(&s.id).unwrap_or((false, None));
                StageInfo {
                    id: s.id,
                    group_name: s.group_name,
                    group_id: s.group_id,
                    stage_order: s.stage_order,
                    status: s.status,
                    started_at: s.started_at,
                    completed_at: s.completed_at,
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

/// Render the rollout-wide health summary as a card above the action
/// buttons. Mirrors the shape used in the rollout-list Health column but
/// with extra detail (heartbeat-freshness avg, per-service probe %)
/// surfaced as inline metrics.
fn render_health_summary_card(hs: &RolloutHealthSummary) -> Element {
    let variant = health_state_variant(&hs.state);
    let label = match hs.state.as_str() {
        "pass" => t!("rollout-health-pass"),
        "fail" => t!("rollout-health-fail"),
        "grace" => t!("rollout-health-grace"),
        _ => t!("rollout-health-no-data"),
    };
    let hb = hs
        .avg_heartbeat_fresh_pct
        .map(|v| format!("{v}%"))
        .unwrap_or_else(|| t!("em-dash"));

    rsx! {
        div { class: "mb-6 p-4 card",
            div { class: "flex items-center gap-3 mb-2",
                span { class: "text-lg font-semibold text-fg-strong",
                    {t!("rollout-detail-health")}
                }
                Badge { variant, "{label}" }
                if hs.failing_stages > 0 {
                    span { class: "text-xs font-mono text-danger-strong",
                        {t!("rollout-detail-stages-failing", failing: hs.failing_stages, total: hs.evaluated_stages)}
                    }
                } else if hs.evaluated_stages > 0 {
                    span { class: "text-xs font-mono text-fg-muted",
                        {t!("rollout-detail-stages-passing", evaluated: hs.evaluated_stages, total: hs.evaluated_stages)}
                    }
                }
            }
            div { class: "flex flex-wrap gap-4 text-xs text-fg",
                span {
                    span { class: "font-medium", {t!("rollout-detail-cohort")} }
                    "{hs.total_cohort}"
                }
                span { class: "font-mono",
                    span { class: "font-medium font-sans", {t!("rollout-detail-heartbeats")} }
                    "{hb}"
                }
                {
                    let mut probe_pairs: Vec<(&String, &u8)> = hs.probe_ok_pct.iter().collect();
                    probe_pairs.sort_by(|a, b| a.0.cmp(b.0));
                    rsx! {
                        for (svc, pct) in probe_pairs {
                            span { class: "font-mono",
                                span { class: "font-medium font-sans", {t!("rollout-detail-service-probe", service: svc.clone())} }
                                "{pct}%"
                            }
                        }
                    }
                }
            }
            if !hs.top_reason.is_empty() {
                p { class: "mt-2 text-xs text-danger-strong",
                    {t!("rollout-detail-top-reason", reason: hs.top_reason.clone())}
                }
            }
        }
    }
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

/// Result of a `RequestAssessment` push. `cohort_size` is the number of
/// clusters targeted by the stage; `dispatched` is the subset that
/// actually had an open SSE channel to receive the push. A value of 0
/// almost always means no daemon in the cohort is currently connected —
/// the most common reason an operator clicks the button and sees nothing.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RequestAssessmentResult {
    cohort_size: u32,
    dispatched: u32,
}

/// Load one stage's current gate config as a `HealthGateInput`. Returns
/// `None` when the stage has no gate (renders as "disabled" toggle in
/// the form so the operator can turn one on).
#[server]
async fn get_stage_gate(stage_id: String) -> Result<Option<HealthGateInput>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let sid: Uuid = stage_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let gate: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT health_gate FROM rollout_stages WHERE id = $1")
            .bind(sid)
            .fetch_optional(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .ok_or_else(|| ServerFnError::new("stage not found"))?;
    Ok(gate.as_ref().map(HealthGateInput::from_json))
}

/// Persist a gate update. When `apply_to_all` is true the payload is
/// written to every stage of the rollout so operators can ratchet one
/// threshold across the board without editing each stage individually.
#[server]
async fn update_stage_gate(
    stage_id: String,
    gate: Option<HealthGateInput>,
    apply_to_all: bool,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let sid: Uuid = stage_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    // `None` or enabled=false both serialise to SQL NULL (= no gate).
    let gate_json: Option<serde_json::Value> = gate.as_ref().and_then(|g| g.to_json());

    if apply_to_all {
        let rollout_id: Uuid =
            sqlx::query_scalar("SELECT rollout_id FROM rollout_stages WHERE id = $1")
                .bind(sid)
                .fetch_optional(&pool)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?
                .ok_or_else(|| ServerFnError::new("stage not found"))?;
        sqlx::query("UPDATE rollout_stages SET health_gate = $1 WHERE rollout_id = $2")
            .bind(&gate_json)
            .bind(rollout_id)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    } else {
        sqlx::query("UPDATE rollout_stages SET health_gate = $1 WHERE id = $2")
            .bind(&gate_json)
            .bind(sid)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }
    Ok(())
}

#[server]
async fn request_stage_assessment(
    _rollout_id: String,
    stage_id: String,
) -> Result<RequestAssessmentResult, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let sid: Uuid = stage_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct StageRow {
        group_id: Uuid,
    }
    let stage: StageRow = sqlx::query_as("SELECT group_id FROM rollout_stages WHERE id = $1")
        .bind(sid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let cohort: Vec<Uuid> = sqlx::query_scalar(
        "SELECT cluster_id FROM rollout_group_members WHERE group_id = $1 \
         UNION ALL \
         SELECT id FROM clusters WHERE $1 = '00000000-0000-0000-0000-000000000000'::uuid",
    )
    .bind(stage.group_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    let cohort_size = cohort.len() as u32;

    let channels = crate::push_channels()?;
    let map = channels.read().await;
    let mut dispatched = 0u32;
    for cid in cohort {
        if let Some(tx) = map.get(&cid) {
            // broadcast::Sender::send returns Err when there are no active
            // receivers — a daemon that connected then closed its SSE.
            // Treat that as "not dispatched" so the UI counter is honest.
            if tx
                .send(crate::api::push::PushMessage::RequestAssessment)
                .is_ok()
            {
                dispatched += 1;
            }
        }
    }
    Ok(RequestAssessmentResult {
        cohort_size,
        dispatched,
    })
}

#[server]
async fn trigger_stage_self_update(
    _rollout_id: String,
    stage_id: String,
) -> Result<RequestAssessmentResult, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let sid: Uuid = stage_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct StageRow { group_id: Uuid }
    let stage: StageRow = sqlx::query_as("SELECT group_id FROM rollout_stages WHERE id = $1")
        .bind(sid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let cohort: Vec<Uuid> = sqlx::query_scalar(
        "SELECT cluster_id FROM rollout_group_members WHERE group_id = $1 \
         UNION ALL \
         SELECT id FROM clusters WHERE $1 = '00000000-0000-0000-0000-000000000000'::uuid",
    )
    .bind(stage.group_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    let cohort_size = cohort.len() as u32;

    let channels = crate::push_channels()?;
    let map = channels.read().await;
    let mut dispatched = 0u32;
    for cid in cohort {
        if let Some(tx) = map.get(&cid) {
            if tx.send(crate::api::push::PushMessage::SelfUpdate).is_ok() {
                dispatched += 1;
            }
        }
    }
    Ok(RequestAssessmentResult { cohort_size, dispatched })
}

#[server]
async fn trigger_stage_sync_nixpkgs(
    _rollout_id: String,
    stage_id: String,
) -> Result<RequestAssessmentResult, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let sid: Uuid = stage_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct StageRow { group_id: Uuid }
    let stage: StageRow = sqlx::query_as("SELECT group_id FROM rollout_stages WHERE id = $1")
        .bind(sid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let cohort: Vec<Uuid> = sqlx::query_scalar(
        "SELECT cluster_id FROM rollout_group_members WHERE group_id = $1 \
         UNION ALL \
         SELECT id FROM clusters WHERE $1 = '00000000-0000-0000-0000-000000000000'::uuid",
    )
    .bind(stage.group_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    let cohort_size = cohort.len() as u32;

    let channels = crate::push_channels()?;
    let map = channels.read().await;
    let mut dispatched = 0u32;
    for cid in cohort {
        if let Some(tx) = map.get(&cid) {
            if tx.send(crate::api::push::PushMessage::SyncNixpkgs).is_ok() {
                dispatched += 1;
            }
        }
    }
    Ok(RequestAssessmentResult { cohort_size, dispatched })
}

/// Re-evaluate this stage *and every other rolling stage* of the same
/// rollout. A per-stage button suggests per-stage scope, but the rollout
/// detail page renders both a stage panel and a rollout-wide summary
/// that aggregates across every rolling stage. Evaluating just one
/// would leave the summary showing stale data from the 60s auto-pause
/// loop's last pass. Evaluate all so the whole page refreshes coherently.
#[server]
async fn reevaluate_stage(stage_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let sid: Uuid = stage_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let rollout_id: Uuid =
        sqlx::query_scalar("SELECT rollout_id FROM rollout_stages WHERE id = $1")
            .bind(sid)
            .fetch_optional(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .ok_or_else(|| ServerFnError::new("stage not found"))?;

    evaluate_all_rolling_stages(&pool, rollout_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
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

#[server]
async fn rollout_action(id: String, action: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let rid: Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    match action.as_str() {
        "start" => {
            // Capture the cohort's current majority version + nixpkgs commit
            // before we start rolling — this is what "rollback" will restore.
            // Skip if baseline is already set (e.g. stopped-then-restarted
            // rollout) to avoid overwriting with post-partial-rollout noise.
            let (baseline_version, baseline_commit) =
                cohort_baseline(&pool, rid).await.unwrap_or((None, None));

            let mut tx = pool
                .begin()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
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
            .map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollout_stages SET status = 'rolling', started_at = now() WHERE rollout_id = $1 AND stage_order = 0")
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            // Seed a first evaluation so the Rollout health card renders
            // immediately instead of waiting for the 60s auto-pause tick.
            let _ = evaluate_all_rolling_stages(&pool, rid).await;
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
            .fetch_optional(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .ok_or_else(|| ServerFnError::new("rollout not found"))?;

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
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

            let mut tx = pool
                .begin()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
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
            .map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query(
                "UPDATE rollouts SET status = 'rolled_back', updated_at = now() WHERE id = $1",
            )
            .bind(rid)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
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
            .map_err(|e| ServerFnError::new(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
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
            .fetch_one(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

            let mut tx = pool
                .begin()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollout_stages SET status = 'completed', completed_at = now() WHERE rollout_id = $1 AND stage_order = $2")
                .bind(rid)
                .bind(current.stage_order)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            let next = current.stage_order + 1;
            let updated = sqlx::query("UPDATE rollout_stages SET status = 'rolling', started_at = now() WHERE rollout_id = $1 AND stage_order = $2")
                .bind(rid)
                .bind(next)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            if updated.rows_affected() == 0 {
                sqlx::query(
                    "UPDATE rollouts SET status = 'completed', updated_at = now() WHERE id = $1",
                )
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            } else {
                sqlx::query("UPDATE rollouts SET updated_at = now() WHERE id = $1")
                    .bind(rid)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;
            }
            tx.commit()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            // Seed evaluation for the newly-rolling stage.
            let _ = evaluate_all_rolling_stages(&pool, rid).await;
            crate::api::push::notify_rollout_global(rid, crate::api::push::PushMessage::SelfUpdate)
                .await;
            crate::api::push::notify_rollout_global(
                rid,
                crate::api::push::PushMessage::SyncNixpkgs,
            )
            .await;
        }
        "pause" => {
            let mut tx = pool
                .begin()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollout_stages SET status = 'paused' WHERE rollout_id = $1 AND status = 'rolling'")
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollouts SET status = 'paused', updated_at = now() WHERE id = $1")
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
        }
        "resume" => {
            let mut tx = pool
                .begin()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollout_stages SET status = 'rolling' WHERE rollout_id = $1 AND status = 'paused'")
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollouts SET status = 'rolling', updated_at = now() WHERE id = $1")
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            // Seed an evaluation for every stage that's resumed so the
            // health card comes back without waiting for the next tick.
            let _ = evaluate_all_rolling_stages(&pool, rid).await;
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
                    .fetch_one(&pool)
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;

            let mut tx = pool
                .begin()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollout_stages SET status = 'completed', completed_at = COALESCE(completed_at, now()) WHERE rollout_id = $1")
                .bind(rid)
                .execute(&mut *tx)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query(
                "UPDATE rollouts SET status = 'completed', updated_at = now() WHERE id = $1",
            )
            .bind(rid)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
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
                .map_err(|e| ServerFnError::new(e.to_string()))?;
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
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            }
            tx.commit()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
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
                .execute(&pool)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
        }
        _ => return Err(ServerFnError::new("unknown action")),
    }

    Ok(())
}

/// Block on a browser `confirm()` dialog. Returns `true` on OK, `false`
/// on Cancel or any JS hiccup. Used to gate destructive rollout actions
/// (rollback, complete, delete) so a misclick doesn't instantly rewind
/// cluster pins or remove historical rows.
async fn confirm_prompt(message: &str) -> bool {
    // Escape single quotes so the message doesn't break the JS literal.
    // Browsers treat confirm() as synchronous; the outer dioxus.send
    // ships the bool back to Rust via the channel recv below.
    let script = format!(
        "dioxus.send(confirm('{}'))",
        message.replace('\\', "\\\\").replace('\'', "\\'")
    );
    document::eval(&script)
        .recv::<serde_json::Value>()
        .await
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

#[component]
pub fn RolloutDetail(id: String) -> Element {
    let id_clone = id.clone();
    let mut detail = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_rollout_detail(id).await }
    })?;
    let nav = navigator();
    // Inline feedback for "Request fresh assessment" — Option<(stage_id,
    // message, is_error)>. Cleared when the operator clicks a different
    // stage's button. Avoids the silent click that prompted this work.
    let mut request_status = use_signal(|| Option::<(String, String, bool)>::None);
    // Stage_id of the currently in-flight Reevaluate call. Rapid clicks on
    // the same button used to stack up concurrent detail.restart()s, each
    // of which flashed the page back to Loading for a moment. Gating the
    // click handler on this signal turns spam into a no-op.
    let mut reevaluating_stage = use_signal(|| Option::<String>::None);
    // Per-stage gate editor: Option<(stage_id, HealthGateInput, apply_to_all)>
    // is None when no editor is open. Only one stage edits at a time — opening
    // another closes the first without prompting (the form has Cancel anyway).
    let mut edit_gate = use_signal(|| Option::<(String, HealthGateInput, bool)>::None);
    let mut edit_gate_error = use_signal(|| Option::<String>::None);

    match &*detail.read() {
        Some(Ok(info)) => {
            let rid = info.id.to_string();
            let status = info.status.clone();
            let created = info.created_at.format("%Y-%m-%d %H:%M").to_string();
            let has_version_update = info.target_version.is_some();
            let has_nixpkgs_update = info.nixpkgs_commit.is_some();

            let badge_variant = status_variant(&info.status);

            let display_name = info.name.clone().unwrap_or_else(|| t!("rollout-detail-rollout-prefix", id: &rid[..8]));
            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    div {
                        h2 { class: "h-page mb-1", "{display_name}" }
                        div { class: "flex items-center gap-2",
                            Badge { variant: badge_variant, "{status}" }
                            span { class: "help", {t!("rollout-detail-created-label", date: created.clone())} }
                        }
                    }
                    div { class: "flex gap-2",
                        if matches!(info.status.as_str(), "rolling" | "paused" | "completed") {
                            Button { variant: ButtonVariant::Warn, size: ButtonSize::Sm,
                                title: "Mark rollout as rolled-back and rewind cluster pins to the baseline captured at start time",
                                onclick: {
                                    let rid = rid.clone();
                                    move |_| {
                                        let rid = rid.clone();
                                        let msg = t!("rollout-detail-rollback-confirm");
                                        async move {
                                            let ok = confirm_prompt(&msg).await;
                                            if !ok { return; }
                                            let _ = rollout_action(rid, "rollback".into()).await;
                                            detail.restart();
                                        }
                                    }
                                },
                                {t!("rollout-detail-rollback")}
                            }
                        }
                        if matches!(info.status.as_str(), "pending" | "completed" | "failed" | "rolled_back") {
                            Button { variant: ButtonVariant::Danger, size: ButtonSize::Sm,
                                onclick: {
                                    let rid = rid.clone();
                                    move |_| {
                                        let rid = rid.clone();
                                        let msg = t!("rollout-detail-delete-confirm");
                                        async move {
                                            let ok = confirm_prompt(&msg).await;
                                            if !ok { return; }
                                            let _ = rollout_action(rid, "delete".into()).await;
                                            nav.push(Route::RolloutList {});
                                        }
                                    }
                                },
                                {t!("delete")}
                            }
                        }
                    }
                }

                // Rollout-wide health summary — visible only for rolling
                // rollouts with at least one gated stage. Aggregates the
                // most recent stored evaluation per stage (auto-pause loop
                // refreshes these every 60s).
                if let Some(hs) = info.health_summary.as_ref() {
                    {render_health_summary_card(hs)}
                }

                // Action buttons
                div { class: "flex gap-2 mb-6",
                    if info.status == "pending" {
                        Button { size: ButtonSize::Sm,
                            onclick: {
                                let rid = rid.clone();
                                move |_| {
                                    let rid = rid.clone();
                                    async move {
                                        let _ = rollout_action(rid, "start".into()).await;
                                        detail.restart();
                                    }
                                }
                            },
                            {t!("rollout-detail-start")}
                        }
                    }
                    if info.status == "rolling" {
                        Button { size: ButtonSize::Sm,
                            onclick: {
                                let rid = rid.clone();
                                move |_| {
                                    let rid = rid.clone();
                                    async move {
                                        let _ = rollout_action(rid, "advance".into()).await;
                                        detail.restart();
                                    }
                                }
                            },
                            {t!("rollout-detail-advance")}
                        }
                        Button { variant: ButtonVariant::Warn, size: ButtonSize::Sm,
                            onclick: {
                                let rid = rid.clone();
                                move |_| {
                                    let rid = rid.clone();
                                    async move {
                                        let _ = rollout_action(rid, "pause".into()).await;
                                        detail.restart();
                                    }
                                }
                            },
                            {t!("rollout-detail-pause")}
                        }
                    }
                    if info.status == "paused" {
                        Button { size: ButtonSize::Sm,
                            onclick: {
                                let rid = rid.clone();
                                move |_| {
                                    let rid = rid.clone();
                                    async move {
                                        let _ = rollout_action(rid, "resume".into()).await;
                                        detail.restart();
                                    }
                                }
                            },
                            {t!("rollout-detail-resume")}
                        }
                    }
                    if info.status != "completed" {
                        Button { variant: ButtonVariant::Secondary, size: ButtonSize::Sm,
                            onclick: {
                                let rid = rid.clone();
                                move |_| {
                                    let rid = rid.clone();
                                    async move {
                                        let msg = t!("rollout-detail-complete-confirm");
                                        let ok = confirm_prompt(&msg).await;
                                        if !ok { return; }
                                        let _ = rollout_action(rid, "complete".into()).await;
                                        detail.restart();
                                    }
                                }
                            },
                            {t!("rollout-detail-complete-all")}
                        }
                    }
                }

                // Stages
                SectionHeading { {t!("rollout-detail-stages")} }
                div { class: "space-y-3 mb-6",
                    for stage in &info.stages {
                        {
                            let stage_badge_variant = status_variant(&stage.status);
                            let started = stage
                                .started_at
                                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let completed = stage
                                .completed_at
                                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let health_text = if stage.total_count > 0 {
                                t!("rollout-detail-online", healthy: stage.healthy_count, total: stage.total_count)
                            } else {
                                t!("rollout-detail-no-heartbeats")
                            };
                            let health_color = if stage.total_count == 0 {
                                "text-fg-faint"
                            } else if stage.healthy_count == stage.total_count {
                                "text-success"
                            } else {
                                "text-danger"
                            };
                            let version_upgrade_text = if stage.total_count > 0 {
                                t!("rollout-detail-version-progress", upgraded: stage.upgraded_count, total: stage.total_count)
                            } else {
                                String::new()
                            };
                            let version_upgrade_color = if stage.total_count == 0 {
                                "text-fg-faint"
                            } else if stage.upgraded_count == stage.total_count {
                                "text-success"
                            } else if stage.upgraded_count > 0 {
                                "text-info"
                            } else {
                                "text-fg-faint"
                            };
                            let nixpkgs_upgrade_text = if stage.total_count > 0 {
                                t!("rollout-detail-nixpkgs-progress", upgraded: stage.nixpkgs_upgraded_count, total: stage.total_count)
                            } else {
                                String::new()
                            };
                            let nixpkgs_upgrade_color = if stage.total_count == 0 {
                                "text-fg-faint"
                            } else if stage.nixpkgs_upgraded_count == stage.total_count {
                                "text-success"
                            } else if stage.nixpkgs_upgraded_count > 0 {
                                "text-info"
                            } else {
                                "text-fg-faint"
                            };

                            let stage_id_str = stage.id.to_string();
                            let rid_clone = rid.clone();

                            rsx! {
                                Card { class: "p-4",
                                    div { class: "flex justify-between items-center mb-2",
                                        div { class: "flex items-center gap-2",
                                            span { class: "font-medium text-sm",
                                                {t!("rollout-detail-stage-num", num: stage.stage_order)}
                                            }
                                            span { class: "text-fg", "{stage.group_name}" }
                                            Badge { variant: stage_badge_variant, "{stage.status}" }
                                        }
                                        div { class: "flex gap-3",
                                            if !version_upgrade_text.is_empty() {
                                                span { class: "text-sm font-medium {version_upgrade_color}",
                                                    "{version_upgrade_text}"
                                                }
                                            }
                                            if !nixpkgs_upgrade_text.is_empty() {
                                                span { class: "text-sm font-medium {nixpkgs_upgrade_color}",
                                                    "{nixpkgs_upgrade_text}"
                                                }
                                            }
                                            span { class: "text-sm font-medium {health_color}",
                                                "{health_text}"
                                            }
                                        }
                                    }
                                    div { class: "text-xs text-fg-faint flex gap-4",
                                        span { {t!("rollout-detail-started", date: started.clone())} }
                                        span { {t!("rollout-detail-completed", date: completed.clone())} }
                                    }

                                    // No-gate affordance: one-line row with an
                                    // {t!("rollout-detail-add-gate")} button. Mirrors where the gate
                                    // panel would have sat, so the card layout
                                    // stays consistent across stages.
                                    if !stage.has_gate {
                                        div { class: "mt-3 pt-3 border-t border-line-soft flex items-center justify-between",
                                            span { class: "text-xs text-fg-muted",
                                                {t!("rollout-detail-no-gate")}
                                            }
                                            div { class: "flex gap-2",
                                                Link {
                                                    to: Route::FleetDashboard { stage_id: Some(stage_id_str.clone()) },
                                                    class: "btn btn-xs btn-secondary",
                                                    title: "Open the fleet dashboard filtered to this stage's cohort",
                                                    {t!("rollout-detail-view-fleet")}
                                                }
                                                button { class: "btn btn-xs btn-info-soft",
                                                    onclick: {
                                                        let sid = stage_id_str.clone();
                                                        move |_| {
                                                            let sid = sid.clone();
                                                            edit_gate_error.set(None);
                                                            edit_gate.set(Some((sid, HealthGateInput::default(), false)));
                                                        }
                                                    },
                                                    {t!("rollout-detail-add-gate")}
                                                }
                                            }
                                        }
                                    }

                                    // ── System-assessment health gate ──
                                    if stage.has_gate {
                                        {
                                            let evaluated_at_text = stage.health.as_ref().map(|h| h.evaluated_at.format("%Y-%m-%d %H:%M:%S").to_string());
                                            // "grace" is reserved for the case where the gate would have
                                            // failed if not for the grace period — i.e. there are reasons
                                            // but they're being shielded. A clean pass during grace renders
                                            // as a normal pass, since there's nothing for the grace period
                                            // to actually shield.
                                            let (gate_variant, gate_text) = match stage.health.as_ref() {
                                                Some(h) if h.passed && h.in_grace_period && !h.reasons.is_empty() =>
                                                    (BadgeVariant::Warn, t!("rollout-detail-gate-grace")),
                                                Some(h) if h.passed =>
                                                    (BadgeVariant::Success, t!("rollout-detail-gate-pass")),
                                                Some(_) =>
                                                    (BadgeVariant::Danger, t!("rollout-detail-gate-fail")),
                                                None =>
                                                    (BadgeVariant::Neutral, t!("rollout-detail-gate-no-data")),
                                            };
                                            rsx! {
                                                div { class: "mt-3 pt-3 border-t border-line-soft",
                                                    div { class: "flex justify-between items-center mb-2",
                                                        div { class: "flex items-center gap-2",
                                                            span { class: "text-sm font-semibold text-fg-strong",
                                                                {t!("rollout-detail-assessment-gate")}
                                                            }
                                                            Badge { variant: gate_variant, "{gate_text}" }
                                                            if let Some(ts) = evaluated_at_text.as_ref() {
                                                                span { class: "text-xs text-fg-muted",
                                                                    {t!("rollout-detail-evaluated", ts: ts.clone())}
                                                                }
                                                            }
                                                        }
                                                        div { class: "flex gap-2",
                                                            button { class: "btn btn-xs btn-secondary",
                                                                title: "Change thresholds, add/remove probed services, or disable the gate entirely",
                                                                onclick: {
                                                                    let sid = stage_id_str.clone();
                                                                    move |_| {
                                                                        let sid_inner = sid.clone();
                                                                        async move {
                                                                            edit_gate_error.set(None);
                                                                            match get_stage_gate(sid_inner.clone()).await {
                                                                                Ok(Some(existing)) => {
                                                                                    edit_gate.set(Some((sid_inner, existing, false)));
                                                                                }
                                                                                Ok(None) => {
                                                                                    edit_gate.set(Some((sid_inner, HealthGateInput::default(), false)));
                                                                                }
                                                                                Err(e) => {
                                                                                    edit_gate_error.set(Some(e.to_string()));
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                },
                                                                {t!("rollout-detail-edit-gate")}
                                                            }
                                                            {
                                                                let busy = reevaluating_stage
                                                                    .read()
                                                                    .as_ref()
                                                                    .map(|s| s == &stage_id_str)
                                                                    .unwrap_or(false);
                                                                let label = if busy { t!("rollout-detail-reevaluating") } else { t!("rollout-detail-reevaluate-now") };
                                                                rsx! {
                                                                    button { class: "btn btn-xs btn-secondary",
                                                                        disabled: busy,
                                                                        onclick: {
                                                                            let sid = stage_id_str.clone();
                                                                            move |_| {
                                                                                let sid = sid.clone();
                                                                                async move {
                                                                                    // Gate so spam turns into a no-op rather
                                                                                    // than stacking up detail.restart()s, each
                                                                                    // of which flashes Loading.
                                                                                    if reevaluating_stage.read().as_ref() == Some(&sid) {
                                                                                        return;
                                                                                    }
                                                                                    reevaluating_stage.set(Some(sid.clone()));
                                                                                    let _ = reevaluate_stage(sid).await;
                                                                                    reevaluating_stage.set(None);
                                                                                    detail.restart();
                                                                                }
                                                                            }
                                                                        },
                                                                        "{label}"
                                                                    }
                                                                }
                                                            }
                                                            button { class: "btn btn-xs btn-info-soft",
                                                                onclick: {
                                                                    let rid = rid_clone.clone();
                                                                    let sid = stage_id_str.clone();
                                                                    move |_| {
                                                                        let rid = rid.clone();
                                                                        let sid = sid.clone();
                                                                        request_status.set(Some((sid.clone(), t!("rollout-detail-requesting"), false)));
                                                                        async move {
                                                                            match request_stage_assessment(rid, sid.clone()).await {
                                                                                Ok(r) => {
                                                                                    let msg = if r.dispatched == 0 {
                                                                                        if r.cohort_size == 0 {
                                                                                            t!("rollout-detail-no-clusters")
                                                                                        } else {
                                                                                            t!("rollout-detail-no-daemons", cohort: r.cohort_size)
                                                                                        }
                                                                                    } else {
                                                                                        t!("rollout-detail-pushed", dispatched: r.dispatched, cohort: r.cohort_size)
                                                                                    };
                                                                                    let is_err = r.dispatched == 0;
                                                                                    request_status.set(Some((sid, msg, is_err)));
                                                                                }
                                                                                Err(e) => {
                                                                                    request_status.set(Some((sid, e.to_string(), true)));
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                },
                                                                {t!("rollout-detail-request-assessment")}
                                                            }
                                                            if has_version_update {
                                                                button { class: "btn btn-xs btn-warn-soft",
                                                                    onclick: {
                                                                        let rid = rid_clone.clone();
                                                                        let sid = stage_id_str.clone();
                                                                        move |_| {
                                                                            let rid = rid.clone();
                                                                            let sid = sid.clone();
                                                                            request_status.set(Some((sid.clone(), t!("rollout-detail-triggering"), false)));
                                                                            async move {
                                                                                match trigger_stage_self_update(rid, sid.clone()).await {
                                                                                    Ok(r) => {
                                                                                        let msg = if r.dispatched == 0 {
                                                                                            t!("rollout-detail-push-none", cohort: r.cohort_size)
                                                                                        } else {
                                                                                            t!("rollout-detail-push-result", dispatched: r.dispatched, cohort: r.cohort_size)
                                                                                        };
                                                                                        request_status.set(Some((sid, msg, r.dispatched == 0)));
                                                                                    }
                                                                                    Err(e) => {
                                                                                        request_status.set(Some((sid, e.to_string(), true)));
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    },
                                                                    {t!("rollout-detail-trigger-self-update")}
                                                                }
                                                            }
                                                            if has_nixpkgs_update {
                                                                button { class: "btn btn-xs btn-accent",
                                                                    onclick: {
                                                                        let rid = rid_clone.clone();
                                                                        let sid = stage_id_str.clone();
                                                                        move |_| {
                                                                            let rid = rid.clone();
                                                                            let sid = sid.clone();
                                                                            request_status.set(Some((sid.clone(), t!("rollout-detail-triggering"), false)));
                                                                            async move {
                                                                                match trigger_stage_sync_nixpkgs(rid, sid.clone()).await {
                                                                                    Ok(r) => {
                                                                                        let msg = if r.dispatched == 0 {
                                                                                            t!("rollout-detail-push-none", cohort: r.cohort_size)
                                                                                        } else {
                                                                                            t!("rollout-detail-push-result", dispatched: r.dispatched, cohort: r.cohort_size)
                                                                                        };
                                                                                        request_status.set(Some((sid, msg, r.dispatched == 0)));
                                                                                    }
                                                                                    Err(e) => {
                                                                                        request_status.set(Some((sid, e.to_string(), true)));
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    },
                                                                    {t!("rollout-detail-trigger-sync-nixpkgs")}
                                                                }
                                                            }
                                                            Link {
                                                                to: Route::FleetDashboard { stage_id: Some(stage_id_str.clone()) },
                                                                class: "btn btn-xs btn-secondary",
                                                                title: "Open the fleet dashboard filtered to this stage's cohort",
                                                                {t!("rollout-detail-view-fleet")}
                                                            }
                                                        }
                                                    }
                                                    // Inline feedback for the just-clicked button. Render
                                                    // only when the latest request targeted *this* stage so
                                                    // each card carries its own status.
                                                    {
                                                        let status_for_this_stage = request_status
                                                            .read()
                                                            .as_ref()
                                                            .filter(|(sid, _, _)| sid == &stage_id_str)
                                                            .map(|(_, msg, is_err)| (msg.clone(), *is_err));
                                                        rsx! {
                                                            if let Some((msg, is_err)) = status_for_this_stage {
                                                                {
                                                                    let cls = if is_err {
                                                                        "mt-2 text-xs text-danger-strong"
                                                                    } else {
                                                                        "mt-2 text-xs text-fg"
                                                                    };
                                                                    rsx! {
                                                                        p { class: "{cls}", "{msg}" }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    if let Some(h) = stage.health.as_ref() {
                                                        div { class: "flex flex-wrap gap-4 text-xs text-fg",
                                                            span {
                                                                span { class: "font-medium", {t!("rollout-detail-cohort")} }
                                                                "{h.cohort_size}"
                                                            }
                                                            span {
                                                                span { class: "font-medium", {t!("rollout-detail-heartbeats")} }
                                                                "{h.heartbeat_fresh_pct}%"
                                                            }
                                                            {
                                                                let mut pairs: Vec<(&String, &u8)> = h.probe_ok_pct.iter().collect();
                                                                pairs.sort_by(|a, b| a.0.cmp(b.0));
                                                                rsx! {
                                                                    for (svc, pct) in pairs {
                                                                        span { class: "font-mono",
                                                                            span { class: "font-medium font-sans", {t!("rollout-detail-service-probe", service: svc.clone())} }
                                                                            "{pct}%"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        if !h.reasons.is_empty() {
                                                            ul { class: "mt-2 text-xs text-danger-strong list-disc list-inside",
                                                                for reason in h.reasons.iter() {
                                                                    li { "{reason}" }
                                                                }
                                                            }
                                                        }
                                                        // Per-service probe aggregates over the gate window.
                                                        if !h.probe_stats.is_empty() {
                                                            div { class: "mt-3 overflow-x-auto",
                                                                table { class: "min-w-full text-xs",
                                                                    thead {
                                                                        tr { class: "text-left text-fg-muted",
                                                                            th { class: "py-1 pr-3 font-medium", {t!("rollout-detail-col-service")} }
                                                                            th { class: "py-1 pr-3 font-medium", {t!("rollout-detail-col-ok-total")} }
                                                                            th { class: "py-1 pr-3 font-medium", {t!("rollout-detail-col-avg-ms")} }
                                                                            th { class: "py-1 pr-3 font-medium", {t!("rollout-detail-col-ttft")} }
                                                                            th { class: "py-1 pr-3 font-medium", {t!("rollout-detail-col-tokens-out")} }
                                                                            th { class: "py-1 font-medium", {t!("rollout-detail-col-last-failure")} }
                                                                        }
                                                                    }
                                                                    tbody { class: "text-fg font-mono",
                                                                        {
                                                                            let mut stat_pairs: Vec<(&String, &ProbeStatsView)> = h.probe_stats.iter().collect();
                                                                            stat_pairs.sort_by(|a, b| a.0.cmp(b.0));
                                                                            rsx! {
                                                                                for (svc, ps) in stat_pairs {
                                                                                    {
                                                                                        let dur = ps.avg_duration_ms.map(|v| format!("{v}")).unwrap_or_else(|| t!("em-dash"));
                                                                                        let ttft = ps.avg_first_token_ms.map(|v| format!("{v}ms")).unwrap_or_else(|| t!("em-dash"));
                                                                                        let tokens = ps.avg_tokens_out.map(|v| format!("{v}")).unwrap_or_else(|| t!("em-dash"));
                                                                                        let failure = match (ps.last_failure_at, &ps.last_error_class) {
                                                                                            (Some(at), Some(cls)) => format!("{} ({cls})", at.format("%H:%M:%S")),
                                                                                            (Some(at), None) => at.format("%H:%M:%S").to_string(),
                                                                                            _ => t!("em-dash"),
                                                                                        };
                                                                                        rsx! {
                                                                                            tr {
                                                                                                td { class: "py-1 pr-3 font-sans font-medium", "{svc}" }
                                                                                                td { class: "py-1 pr-3", "{ps.ok_count}/{ps.total_runs}" }
                                                                                                td { class: "py-1 pr-3", "{dur}" }
                                                                                                td { class: "py-1 pr-3", "{ttft}" }
                                                                                                td { class: "py-1 pr-3", "{tokens}" }
                                                                                                td { class: "py-1 text-fg-muted font-sans", "{failure}" }
                                                                                            }
                                                                                        }
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        // Cohort sample summary — derived from the most recent
                                                        // heartbeat sample of every reporting instance.
                                                        if let Some(s) = h.sample_summary.as_ref() {
                                                            {
                                                                let disk = s.max_disk_used_pct.map(|v| format!("{v}%")).unwrap_or_else(|| t!("em-dash"));
                                                                let gpu = s.gpu_avg_util_pct.map(|v| format!("{v}%")).unwrap_or_else(|| t!("em-dash"));
                                                                let cpu = format!("{:.2}", s.avg_cpu_load_1m);
                                                                rsx! {
                                                                    div { class: "mt-3 flex flex-wrap gap-4 text-xs text-fg",
                                                                        span {
                                                                            span { class: "font-medium", {t!("rollout-detail-samples")} }
                                                                            "{s.reporting_instances}"
                                                                        }
                                                                        span { class: "font-mono",
                                                                            span { class: "font-medium font-sans", {t!("rollout-detail-cpu-load")} }
                                                                            "{cpu}"
                                                                        }
                                                                        span { class: "font-mono",
                                                                            span { class: "font-medium font-sans", {t!("rollout-detail-mem")} }
                                                                            "{s.avg_mem_used_pct}%"
                                                                        }
                                                                        span { class: "font-mono",
                                                                            span { class: "font-medium font-sans", {t!("rollout-detail-max-disk")} }
                                                                            "{disk}"
                                                                        }
                                                                        span { class: "font-mono",
                                                                            span { class: "font-medium font-sans", {t!("rollout-detail-gpu-util")} }
                                                                            "{gpu}"
                                                                        }
                                                                        if s.thermal_alerts > 0 {
                                                                            span { class: "text-warn-strong font-medium",
                                                                                {t!("rollout-detail-thermal-alerts", count: s.thermal_alerts)}
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // ── Inline gate editor ──
                                    // Renders when edit_gate's target matches
                                    // this stage_id. Kept under the gate panel
                                    // so save/cancel lands in the operator's
                                    // natural reading flow.
                                    {
                                        let editing_here = edit_gate
                                            .read()
                                            .as_ref()
                                            .map(|(sid, _, _)| sid == &stage_id_str)
                                            .unwrap_or(false);
                                        let err = if editing_here {
                                            edit_gate_error.read().clone()
                                        } else {
                                            None
                                        };
                                        rsx! {
                                            if editing_here {
                                                div { class: "mt-3 pt-3 border-t border-line-soft",
                                                    div { class: "flex items-center justify-between mb-2",
                                                        span { class: "text-sm font-semibold text-fg-strong",
                                                            {t!("rollout-detail-gate-config")}
                                                        }
                                                        button { class: "text-xs text-fg-muted hover:underline",
                                                            onclick: move |_| {
                                                                edit_gate.set(None);
                                                                edit_gate_error.set(None);
                                                            },
                                                            {t!("cancel")}
                                                        }
                                                    }

                                                    // Read snapshot for display; writes flow through
                                                    // edit_gate.write() on each input change.
                                                    {
                                                        let snap = edit_gate
                                                            .read()
                                                            .as_ref()
                                                            .map(|(_, g, a)| (g.clone(), *a))
                                                            .unwrap_or_else(|| (HealthGateInput::default(), false));
                                                        let (g, apply_all) = snap;
                                                        rsx! {
                                                            div { class: "flex items-center gap-2 mb-3",
                                                                input {
                                                                    r#type: "checkbox",
                                                                    checked: g.enabled,
                                                                    class: "rounded border-line text-brand focus:ring-brand",
                                                                    onchange: move |e| {
                                                                        let mut w = edit_gate.write();
                                                                        if let Some(t) = w.as_mut() {
                                                                            t.1.enabled = e.value() == "true";
                                                                        }
                                                                    },
                                                                }
                                                                label { class: "text-xs text-fg-strong",
                                                                    {t!("rollout-detail-gate-enabled")}
                                                                }
                                                            }

                                                            if g.enabled {
                                                                div { class: "grid grid-cols-1 sm:grid-cols-3 gap-3 mb-3",
                                                                    div {
                                                                        label { class: "block text-xs font-medium text-fg mb-1",
                                                                            {t!("rollout-form-heartbeat-pct")}
                                                                        }
                                                                        input {
                                                                            r#type: "number", min: "0", max: "100",
                                                                            class: "input input-sm",
                                                                            value: "{g.min_heartbeat_fresh_pct}",
                                                                            oninput: move |e| {
                                                                                if let Ok(v) = e.value().parse::<u8>() {
                                                                                    let mut w = edit_gate.write();
                                                                                    if let Some(t) = w.as_mut() {
                                                                                        t.1.min_heartbeat_fresh_pct = v.min(100);
                                                                                    }
                                                                                }
                                                                            },
                                                                        }
                                                                    }
                                                                    div {
                                                                        label { class: "block text-xs font-medium text-fg mb-1",
                                                                            {t!("rollout-detail-freshness-window")}
                                                                        }
                                                                        input {
                                                                            r#type: "number", min: "10",
                                                                            class: "input input-sm",
                                                                            value: "{g.heartbeat_freshness_secs}",
                                                                            oninput: move |e| {
                                                                                if let Ok(v) = e.value().parse::<u32>() {
                                                                                    let mut w = edit_gate.write();
                                                                                    if let Some(t) = w.as_mut() {
                                                                                        t.1.heartbeat_freshness_secs = v.max(10);
                                                                                    }
                                                                                }
                                                                            },
                                                                        }
                                                                    }
                                                                    div {
                                                                        label { class: "block text-xs font-medium text-fg mb-1",
                                                                            {t!("rollout-detail-grace-period")}
                                                                        }
                                                                        input {
                                                                            r#type: "number", min: "0",
                                                                            class: "input input-sm",
                                                                            value: "{g.grace_period_secs}",
                                                                            oninput: move |e| {
                                                                                if let Ok(v) = e.value().parse::<u32>() {
                                                                                    let mut w = edit_gate.write();
                                                                                    if let Some(t) = w.as_mut() {
                                                                                        t.1.grace_period_secs = v;
                                                                                    }
                                                                                }
                                                                            },
                                                                        }
                                                                    }
                                                                }

                                                                div { class: "mb-3",
                                                                    label { class: "block text-xs font-medium text-fg mb-1",
                                                                        {t!("rollout-form-probe-thresholds")}
                                                                    }
                                                                    {
                                                                        let rows: Vec<(usize, String, u8)> = g
                                                                            .probe_thresholds
                                                                            .iter()
                                                                            .enumerate()
                                                                            .map(|(i, (s, p))| (i, s.clone(), *p))
                                                                            .collect();
                                                                        rsx! {
                                                                            for (idx, svc, pct) in rows {
                                                                                div { class: "flex items-center gap-2 mb-1",
                                                                                    input { class: "input input-sm flex-1 w-auto",
                                                                                        placeholder: t!("rollout-form-service-placeholder"),
                                                                                        value: "{svc}",
                                                                                        oninput: move |e| {
                                                                                            let mut w = edit_gate.write();
                                                                                            if let Some(t) = w.as_mut() {
                                                                                                if let Some(row) = t.1.probe_thresholds.get_mut(idx) {
                                                                                                    row.0 = e.value();
                                                                                                }
                                                                                            }
                                                                                        },
                                                                                    }
                                                                                    input {
                                                                                        r#type: "number", min: "0", max: "100",
                                                                                        class: "input input-sm w-20",
                                                                                        value: "{pct}",
                                                                                        oninput: move |e| {
                                                                                            if let Ok(v) = e.value().parse::<u8>() {
                                                                                                let mut w = edit_gate.write();
                                                                                                if let Some(t) = w.as_mut() {
                                                                                                    if let Some(row) = t.1.probe_thresholds.get_mut(idx) {
                                                                                                        row.1 = v.min(100);
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                        },
                                                                                    }
                                                                                    span { class: "text-xs text-fg-muted", {t!("rollout-form-pct-symbol")} }
                                                                                    button { class: "link-danger text-xs",
                                                                                        onclick: move |_| {
                                                                                            let mut w = edit_gate.write();
                                                                                            if let Some(t) = w.as_mut() {
                                                                                                if idx < t.1.probe_thresholds.len() {
                                                                                                    t.1.probe_thresholds.remove(idx);
                                                                                                }
                                                                                            }
                                                                                        },
                                                                                        {t!("rollout-form-remove-service")}
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                    button { class: "link text-xs mt-1",
                                                                        onclick: move |_| {
                                                                            let mut w = edit_gate.write();
                                                                            if let Some(t) = w.as_mut() {
                                                                                t.1.probe_thresholds.push((String::new(), 90));
                                                                            }
                                                                        },
                                                                        {t!("rollout-form-add-service")}
                                                                    }
                                                                }
                                                            }

                                                            // Apply-to-all + Save row.
                                                            div { class: "flex items-center gap-3 mt-3",
                                                                div { class: "flex items-center gap-2",
                                                                    input {
                                                                        r#type: "checkbox",
                                                                        checked: apply_all,
                                                                        class: "rounded border-line text-brand focus:ring-brand",
                                                                        onchange: move |e| {
                                                                            let mut w = edit_gate.write();
                                                                            if let Some(t) = w.as_mut() {
                                                                                t.2 = e.value() == "true";
                                                                            }
                                                                        },
                                                                    }
                                                                    label { class: "text-xs text-fg",
                                                                        {t!("rollout-detail-apply-all")}
                                                                    }
                                                                }
                                                                button { class: "ml-auto btn btn-xs btn-primary",
                                                                    onclick: move |_| {
                                                                        let snap = edit_gate
                                                                            .read()
                                                                            .as_ref()
                                                                            .map(|(sid, g, a)| (sid.clone(), g.clone(), *a));
                                                                        async move {
                                                                            let Some((sid, g, apply_all)) = snap else { return; };
                                                                            match update_stage_gate(sid, Some(g), apply_all).await {
                                                                                Ok(()) => {
                                                                                    edit_gate.set(None);
                                                                                    edit_gate_error.set(None);
                                                                                    detail.restart();
                                                                                }
                                                                                Err(e) => {
                                                                                    edit_gate_error.set(Some(e.to_string()));
                                                                                }
                                                                            }
                                                                        }
                                                                    },
                                                                    {t!("save")}
                                                                }
                                                            }
                                                            if let Some(msg) = err.as_ref() {
                                                                p { class: "mt-2 text-xs text-danger-strong",
                                                                    "{msg}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Target
                SectionHeading { class: "mb-2", {t!("rollout-detail-target")} }
                div { class: "bg-surface-2 p-4 rounded text-sm space-y-1",
                    if let Some(ver) = &info.target_version {
                        p { span { class: "font-medium", {t!("rollout-detail-version-label")} } "{ver}" }
                    }
                    if let Some(commit) = &info.nixpkgs_commit {
                        {
                            let short: String = commit.chars().take(12).collect();
                            let url = format!("https://git.plan.ai/plan-ai/nixpkgs/-/commit/{commit}");
                            let sha_for_count = commit.clone();
                            let count_res = use_resource(move || {
                                let s = sha_for_count.clone();
                                async move { get_nixpkgs_commit_count_rollout(s).await.ok().flatten() }
                            });
                            let count_label = count_res.read().as_ref()
                                .and_then(|n| n.as_ref())
                                .map(|n| format!(" #{n}"))
                                .unwrap_or_default();
                            rsx! {
                                p {
                                    span { class: "font-medium", {t!("rollout-detail-nixpkgs-label")} }
                                    a { class: "font-mono text-sm hover:text-brand",
                                        href: "{url}",
                                        target: "_blank",
                                        title: "{commit}",
                                        "{short}{count_label}"
                                    }
                                }
                            }
                        }
                    }
                }

                // Baseline — shown once the rollout has started so operators
                // can see exactly what a rollback would restore. Hidden for
                // pending rollouts where nothing's been captured yet.
                if info.baseline_version.is_some() || info.baseline_nixpkgs_commit.is_some() {
                    SectionHeading { class: "mb-2 mt-4", {t!("rollout-detail-rollback-baseline")} }
                    div { class: "bg-surface-2 p-4 rounded text-sm space-y-1",
                        if let Some(ver) = &info.baseline_version {
                            p { span { class: "font-medium", {t!("rollout-detail-version-label")} } "{ver}" }
                        }
                        if let Some(commit) = &info.baseline_nixpkgs_commit {
                            {
                                let short: String = commit.chars().take(12).collect();
                                let url = format!("https://git.plan.ai/plan-ai/nixpkgs/-/commit/{commit}");
                                let sha_for_count = commit.clone();
                                let count_res = use_resource(move || {
                                    let s = sha_for_count.clone();
                                    async move { get_nixpkgs_commit_count_rollout(s).await.ok().flatten() }
                                });
                                let count_label = count_res.read().as_ref()
                                    .and_then(|n| n.as_ref())
                                    .map(|n| format!(" #{n}"))
                                    .unwrap_or_default();
                                rsx! {
                                    p {
                                        span { class: "font-medium", {t!("rollout-detail-nixpkgs-label")} }
                                        a { class: "font-mono text-sm hover:text-brand",
                                            href: "{url}",
                                            target: "_blank",
                                            title: "{commit}",
                                            "{short}{count_label}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}
