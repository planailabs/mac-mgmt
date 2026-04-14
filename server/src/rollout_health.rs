//! Rollout health gates — evaluate per-stage readiness from assessment data.
//!
//! A stage's gate evaluates to pass/fail based on three datasources:
//!   * Heartbeat freshness across the stage's cluster cohort
//!   * Per-service probe success rate over a recent window
//!   * Security-posture regression checks (future)
//!
//! Stages are auto-paused when their gate fails during rollout. Admins can
//! configure each stage's gate at rollout-create time; no gate = always pass
//! (legacy behaviour).
//!
//! The `DEFAULT_GATE` below is applied when a stage has no explicit gate set.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Serialised as the `health_gate` column on `rollout_stages`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthGate {
    /// Minimum percentage of stage-cohort instances that must have heartbeat
    /// within `heartbeat_freshness_secs`. 0 disables.
    #[serde(default = "default_min_heartbeat_fresh_pct")]
    pub min_heartbeat_fresh_pct: u8,
    /// Max age of a heartbeat to count it as fresh. Default 3 min.
    #[serde(default = "default_heartbeat_freshness_secs")]
    pub heartbeat_freshness_secs: u32,
    /// Per-service probe success rate thresholds. A service not listed is
    /// not gated. Service name → required percentage (0..=100).
    #[serde(default)]
    pub min_probe_ok_pct: HashMap<String, u8>,
    /// How long to allow the stage to settle after starting before evaluating.
    /// Prevents an immediate pause when no data has been collected yet.
    #[serde(default = "default_grace_period_secs")]
    pub grace_period_secs: u32,
}

fn default_min_heartbeat_fresh_pct() -> u8 { 95 }
fn default_heartbeat_freshness_secs() -> u32 { 180 }
fn default_grace_period_secs() -> u32 { 600 }

impl Default for HealthGate {
    fn default() -> Self {
        let mut min_probe = HashMap::new();
        min_probe.insert("openclaw".to_string(), 90);
        min_probe.insert("ollama".to_string(), 90);
        Self {
            min_heartbeat_fresh_pct: default_min_heartbeat_fresh_pct(),
            heartbeat_freshness_secs: default_heartbeat_freshness_secs(),
            min_probe_ok_pct: min_probe,
            grace_period_secs: default_grace_period_secs(),
        }
    }
}

/// Full evaluation report — stored in `rollout_stage_health_evaluations.report`.
///
/// Beyond the raw gate inputs (`heartbeat_fresh_pct`, `probe_ok_pct`) this
/// struct also carries aggregate probe statistics and a cohort sample
/// summary so the UI can answer "why is the gate failing?" without a
/// second round-trip per service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthEvaluation {
    pub passed: bool,
    pub cohort_size: u32,
    pub reasons: Vec<String>,
    pub heartbeat_fresh_pct: u8,
    pub probe_ok_pct: HashMap<String, u8>,
    /// `true` if the stage was within its grace period and skipped gating.
    pub in_grace_period: bool,
    /// Per-service aggregates over the same 30-minute window as
    /// `probe_ok_pct`. Keyed by service name. Empty when no probes ran.
    #[serde(default)]
    pub probe_stats: HashMap<String, ProbeStats>,
    /// Cohort-wide averages computed from the latest heartbeat sample of
    /// every reporting instance. `None` when no instance reported a sample.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_summary: Option<SampleSummary>,
}

/// Aggregate of probe rows for a single service across the cohort. Used
/// both to explain a gate failure and to render a per-stage health table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProbeStats {
    pub total_runs: u32,
    pub ok_count: u32,
    /// Mean `duration_ms` across all runs in the window. `None` when
    /// `total_runs == 0`.
    pub avg_duration_ms: Option<u32>,
    /// Mean `tokens_out` across runs that reported token counts (LLM
    /// backends only — missing on liveness probes).
    pub avg_tokens_out: Option<u32>,
    pub avg_first_token_ms: Option<u32>,
    /// Timestamp of the most recent failed probe row. `None` when no
    /// failure in the window.
    pub last_failure_at: Option<chrono::DateTime<chrono::Utc>>,
    /// `error_class` from the most recent failed row (e.g. "timeout",
    /// "bad_response"). Always `None` when `last_failure_at` is `None`.
    pub last_error_class: Option<String>,
}

/// Cohort-wide averages from the latest heartbeat sample per instance.
/// All percentages are 0-100. `thermal_alerts` is a count, not a
/// percentage, because it's rare by design.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SampleSummary {
    /// Instances contributing to this summary — equal to the number of
    /// rows in `daemon_heartbeats` for the cohort with a non-null sample.
    pub reporting_instances: u32,
    pub avg_cpu_load_1m: f32,
    pub avg_mem_used_pct: u8,
    /// Max used-% across the root + /nix/store mounts reported by any
    /// instance in the cohort. `None` when no sample reported disks.
    pub max_disk_used_pct: Option<u8>,
    /// Mean utilization across every GPU reported by every reporting
    /// instance. `None` when no GPU samples exist.
    pub gpu_avg_util_pct: Option<u8>,
    /// Number of instances whose thermal_state is anything other than
    /// "nominal" in their last sample.
    pub thermal_alerts: u32,
}

/// Evaluate a stage against its gate config.
///
/// Returns `Ok(None)` when the stage has no gate configured — callers should
/// treat that as a pass. An error return is reserved for database issues, not
/// gate failures (which come back as `passed: false` in the report).
pub async fn evaluate_stage(
    pool: &PgPool,
    stage_id: Uuid,
) -> Result<Option<HealthEvaluation>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct StageRow {
        group_id: Uuid,
        started_at: Option<chrono::DateTime<chrono::Utc>>,
        status: String,
        health_gate: Option<serde_json::Value>,
    }

    let stage: StageRow = sqlx::query_as(
        "SELECT group_id, started_at, status, health_gate \
         FROM rollout_stages WHERE id = $1",
    )
    .bind(stage_id)
    .fetch_one(pool)
    .await?;

    let gate: HealthGate = match stage.health_gate {
        Some(v) => serde_json::from_value(v).unwrap_or_default(),
        None => return Ok(None),
    };

    // Stage must be active to be gated.
    if stage.status != "rolling" {
        return Ok(None);
    }

    let cohort: Vec<Uuid> = sqlx::query_scalar(
        "SELECT cluster_id FROM rollout_group_members WHERE group_id = $1 \
         UNION ALL \
         SELECT id FROM clusters WHERE $1 = '00000000-0000-0000-0000-000000000000'::uuid",
    )
    .bind(stage.group_id)
    .fetch_all(pool)
    .await?;
    let cohort_size = cohort.len() as u32;

    let in_grace_period = stage
        .started_at
        .map(|t| {
            let age = (chrono::Utc::now() - t).num_seconds();
            age >= 0 && age < gate.grace_period_secs as i64
        })
        .unwrap_or(true);

    if cohort_size == 0 {
        return Ok(Some(HealthEvaluation {
            passed: true,
            cohort_size: 0,
            reasons: vec!["empty cohort".into()],
            heartbeat_fresh_pct: 100,
            probe_ok_pct: HashMap::new(),
            in_grace_period,
            probe_stats: HashMap::new(),
            sample_summary: None,
        }));
    }

    // Heartbeat freshness.
    let fresh_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM daemon_heartbeats \
         WHERE cluster_id = ANY($1) \
           AND reported_at > now() - ($2::int || ' seconds')::interval",
    )
    .bind(&cohort)
    .bind(gate.heartbeat_freshness_secs as i32)
    .fetch_one(pool)
    .await?;
    let heartbeat_fresh_pct = pct(fresh_count as u32, cohort_size);

    // Per-service probe stats over the last 30 min. Aggregates feed both
    // the existing gate logic (`probe_ok_pct`) and the new `probe_stats`
    // table the UI renders.
    let probe_stats = collect_probe_stats(pool, &cohort, &gate.min_probe_ok_pct).await?;
    let probe_ok_pct: HashMap<String, u8> = probe_stats
        .iter()
        .map(|(svc, s)| {
            let pct_v = if s.total_runs > 0 {
                pct(s.ok_count, s.total_runs)
            } else {
                0
            };
            (svc.clone(), pct_v)
        })
        .collect();

    let sample_summary = collect_sample_summary(pool, &cohort).await?;

    // Accumulate failure reasons.
    let mut reasons: Vec<String> = Vec::new();
    if gate.min_heartbeat_fresh_pct > 0 && heartbeat_fresh_pct < gate.min_heartbeat_fresh_pct {
        reasons.push(format!(
            "heartbeat freshness {heartbeat_fresh_pct}% < required {}%",
            gate.min_heartbeat_fresh_pct
        ));
    }
    for (service, required) in &gate.min_probe_ok_pct {
        let got = probe_ok_pct.get(service).copied().unwrap_or(0);
        if got < *required {
            reasons.push(format!(
                "{service} probe ok {got}% < required {required}%"
            ));
        }
    }

    let passed = reasons.is_empty() || in_grace_period;
    Ok(Some(HealthEvaluation {
        passed,
        cohort_size,
        reasons,
        heartbeat_fresh_pct,
        probe_ok_pct,
        in_grace_period,
        probe_stats,
        sample_summary,
    }))
}

/// Pull aggregate probe stats per service over the last 30 min. We always
/// include every service in `gated` (so the UI can show 0/0 for services
/// that failed to run at all), and any other service that has rows in the
/// window so operators see liveness probes alongside gated ones.
async fn collect_probe_stats(
    pool: &PgPool,
    cohort: &[Uuid],
    gated: &HashMap<String, u8>,
) -> Result<HashMap<String, ProbeStats>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        service: String,
        total: i64,
        ok: i64,
        avg_duration_ms: Option<f64>,
        avg_tokens_out: Option<f64>,
        avg_first_token_ms: Option<f64>,
        last_failure_at: Option<chrono::DateTime<chrono::Utc>>,
        last_error_class: Option<String>,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT service, \
                COUNT(*) AS total, \
                COUNT(*) FILTER (WHERE ok) AS ok, \
                AVG(duration_ms)::float8 AS avg_duration_ms, \
                AVG(tokens_out)::float8 AS avg_tokens_out, \
                AVG(first_token_ms)::float8 AS avg_first_token_ms, \
                MAX(collected_at) FILTER (WHERE NOT ok) AS last_failure_at, \
                ( \
                  SELECT error_class FROM assessment_probes p2 \
                  WHERE p2.cluster_id = ANY($1) AND p2.service = p.service AND NOT p2.ok \
                    AND p2.collected_at > now() - interval '30 minutes' \
                  ORDER BY p2.collected_at DESC LIMIT 1 \
                ) AS last_error_class \
         FROM assessment_probes p \
         WHERE p.cluster_id = ANY($1) \
           AND p.collected_at > now() - interval '30 minutes' \
         GROUP BY p.service",
    )
    .bind(cohort)
    .fetch_all(pool)
    .await?;

    let mut out: HashMap<String, ProbeStats> = HashMap::new();
    for r in rows {
        out.insert(
            r.service,
            ProbeStats {
                total_runs: r.total as u32,
                ok_count: r.ok as u32,
                avg_duration_ms: r.avg_duration_ms.map(|v| v as u32),
                avg_tokens_out: r.avg_tokens_out.map(|v| v as u32),
                avg_first_token_ms: r.avg_first_token_ms.map(|v| v as u32),
                last_failure_at: r.last_failure_at,
                last_error_class: r.last_error_class,
            },
        );
    }
    // Backfill gated services that produced no rows so the UI sees 0/0
    // rather than silently omitting a required probe.
    for service in gated.keys() {
        out.entry(service.clone()).or_insert_with(ProbeStats::default);
    }
    Ok(out)
}

/// Aggregate the latest heartbeat sample per instance across the cohort.
/// Returns `None` when no instance has reported a sample (typical for
/// brand-new rollouts where daemons haven't ticked yet).
async fn collect_sample_summary(
    pool: &PgPool,
    cohort: &[Uuid],
) -> Result<Option<SampleSummary>, sqlx::Error> {
    let samples: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT sample FROM daemon_heartbeats \
         WHERE cluster_id = ANY($1) AND sample IS NOT NULL",
    )
    .bind(cohort)
    .fetch_all(pool)
    .await?;
    if samples.is_empty() {
        return Ok(None);
    }

    let mut cpu_acc = 0.0f64;
    let mut mem_acc = 0u64;
    let mut mem_count = 0u32;
    let mut max_disk_used_pct: Option<u8> = None;
    let mut gpu_util_acc = 0u64;
    let mut gpu_util_count = 0u32;
    let mut thermal_alerts = 0u32;

    for s in &samples {
        if let Some(v) = s.get("cpu_load_1m").and_then(|v| v.as_f64()) {
            cpu_acc += v;
        }
        let used = s.get("mem_used_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
        let total = s.get("mem_total_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
        if total > 0 {
            mem_acc += used.saturating_mul(100) / total;
            mem_count += 1;
        }
        if let Some(disks) = s.get("disk_free").and_then(|v| v.as_array()) {
            for d in disks {
                let free = d.get("free_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                let dtotal = d.get("total_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                if dtotal > 0 {
                    let used_pct = ((dtotal - free).saturating_mul(100) / dtotal).min(100) as u8;
                    max_disk_used_pct = Some(max_disk_used_pct.map_or(used_pct, |m| m.max(used_pct)));
                }
            }
        }
        if let Some(gpus) = s.get("gpus").and_then(|v| v.as_array()) {
            for g in gpus {
                if let Some(util) = g.get("utilization_pct").and_then(|v| v.as_u64()) {
                    gpu_util_acc += util;
                    gpu_util_count += 1;
                }
            }
        }
        match s.get("thermal_state").and_then(|v| v.as_str()) {
            Some("nominal") | None => {}
            Some(_) => thermal_alerts += 1,
        }
    }

    let n = samples.len() as u32;
    Ok(Some(SampleSummary {
        reporting_instances: n,
        avg_cpu_load_1m: (cpu_acc / n as f64) as f32,
        avg_mem_used_pct: if mem_count == 0 {
            0
        } else {
            (mem_acc / mem_count as u64).min(100) as u8
        },
        max_disk_used_pct,
        gpu_avg_util_pct: if gpu_util_count == 0 {
            None
        } else {
            Some((gpu_util_acc / gpu_util_count as u64).min(100) as u8)
        },
        thermal_alerts,
    }))
}

fn pct(num: u32, denom: u32) -> u8 {
    if denom == 0 { return 100; }
    ((num as u64 * 100 / denom as u64).min(100)) as u8
}

/// Background loop: evaluate every active stage on `interval`; on failure,
/// auto-pause the stage and record the evaluation. Spawn once at server start.
pub async fn run_auto_pause_loop(pool: PgPool, interval: std::time::Duration) {
    loop {
        if let Err(e) = tick(&pool).await {
            tracing::warn!("rollout-health tick failed: {e}");
        }
        tokio::time::sleep(interval).await;
    }
}

async fn tick(pool: &PgPool) -> Result<(), sqlx::Error> {
    let active_stages: Vec<Uuid> = sqlx::query_scalar(
        "SELECT rs.id FROM rollout_stages rs \
         JOIN rollouts r ON r.id = rs.rollout_id \
         WHERE rs.status = 'rolling' AND r.status = 'rolling'",
    )
    .fetch_all(pool)
    .await?;

    for stage_id in active_stages {
        let eval = match evaluate_stage(pool, stage_id).await? {
            Some(e) => e,
            None => continue,
        };
        sqlx::query(
            "INSERT INTO rollout_stage_health_evaluations (stage_id, passed, report) \
             VALUES ($1, $2, $3)",
        )
        .bind(stage_id)
        .bind(eval.passed)
        .bind(serde_json::to_value(&eval).unwrap_or_default())
        .execute(pool)
        .await?;

        if !eval.passed {
            tracing::warn!(
                "rollout stage {stage_id} health gate failed: {:?}",
                eval.reasons
            );
            sqlx::query(
                "UPDATE rollout_stages SET status = 'paused' WHERE id = $1 AND status = 'rolling'",
            )
            .bind(stage_id)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}
