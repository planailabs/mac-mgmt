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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthEvaluation {
    pub passed: bool,
    pub cohort_size: u32,
    pub reasons: Vec<String>,
    pub heartbeat_fresh_pct: u8,
    pub probe_ok_pct: HashMap<String, u8>,
    /// `true` if the stage was within its grace period and skipped gating.
    pub in_grace_period: bool,
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

    // Per-service probe success rate over the last 30 min.
    let mut probe_ok_pct: HashMap<String, u8> = HashMap::new();
    for service in gate.min_probe_ok_pct.keys() {
        #[derive(sqlx::FromRow)]
        struct AggRow { total: i64, ok: i64 }
        let agg: Option<AggRow> = sqlx::query_as(
            "SELECT COUNT(*) AS total, COUNT(*) FILTER (WHERE ok) AS ok \
             FROM assessment_probes \
             WHERE cluster_id = ANY($1) AND service = $2 \
               AND collected_at > now() - interval '30 minutes'",
        )
        .bind(&cohort)
        .bind(service)
        .fetch_optional(pool)
        .await?;
        let pct_v = match agg {
            Some(a) if a.total > 0 => pct(a.ok as u32, a.total as u32),
            _ => 0,
        };
        probe_ok_pct.insert(service.clone(), pct_v);
    }

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
