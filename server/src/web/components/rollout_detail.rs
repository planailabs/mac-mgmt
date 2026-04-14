use dioxus::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RolloutInfo {
    id: Uuid,
    target_version: Option<String>,
    nixpkgs_commit: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    stages: Vec<StageInfo>,
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
        target_version: Option<String>,
        nixpkgs_commit: Option<String>,
        status: String,
        created_at: DateTime<Utc>,
    }

    let rollout = sqlx::query_as::<_, RRow>(
        "SELECT id, target_version, nixpkgs_commit, status, created_at FROM rollouts WHERE id = $1",
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
        .map(|h| (h.stage_order, (h.healthy, h.total, h.upgraded, h.nixpkgs_upgraded)))
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

    Ok(RolloutInfo {
        id: rollout.id,
        target_version: rollout.target_version,
        nixpkgs_commit: rollout.nixpkgs_commit,
        status: rollout.status,
        created_at: rollout.created_at,
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
    Some(StageHealthInfo {
        passed: v.get("passed").and_then(|x| x.as_bool()).unwrap_or(false),
        in_grace_period: v.get("in_grace_period").and_then(|x| x.as_bool()).unwrap_or(false),
        cohort_size: v.get("cohort_size").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        heartbeat_fresh_pct: v.get("heartbeat_fresh_pct").and_then(|x| x.as_u64()).unwrap_or(0) as u8,
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
    })
}

#[server]
async fn request_stage_assessment(
    _rollout_id: String,
    stage_id: String,
) -> Result<(), ServerFnError> {
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

    let channels = crate::push_channels()?;
    let map = channels.read().await;
    for cid in cohort {
        if let Some(tx) = map.get(&cid) {
            let _ = tx.send(crate::api::push::PushMessage::RequestAssessment);
        }
    }
    Ok(())
}

#[server]
async fn reevaluate_stage(stage_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let sid: Uuid = stage_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let eval = crate::rollout_health::evaluate_stage(&pool, sid)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if let Some(e) = eval {
        sqlx::query(
            "INSERT INTO rollout_stage_health_evaluations (stage_id, passed, report) \
             VALUES ($1, $2, $3)",
        )
        .bind(sid)
        .bind(e.passed)
        .bind(serde_json::to_value(&e).unwrap_or_default())
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
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
            let mut tx = pool
                .begin()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query(
                "UPDATE rollouts SET status = 'rolling', updated_at = now() WHERE id = $1",
            )
            .bind(rid)
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
            crate::api::push::notify_rollout_global(rid, crate::api::push::PushMessage::SelfUpdate).await;
            crate::api::push::notify_rollout_global(rid, crate::api::push::PushMessage::SyncNixpkgs).await;
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
            crate::api::push::notify_rollout_global(rid, crate::api::push::PushMessage::SelfUpdate).await;
            crate::api::push::notify_rollout_global(rid, crate::api::push::PushMessage::SyncNixpkgs).await;
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
            sqlx::query(
                "UPDATE rollouts SET status = 'paused', updated_at = now() WHERE id = $1",
            )
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
            sqlx::query(
                "UPDATE rollouts SET status = 'rolling', updated_at = now() WHERE id = $1",
            )
            .bind(rid)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            crate::api::push::notify_rollout_global(rid, crate::api::push::PushMessage::SelfUpdate).await;
            crate::api::push::notify_rollout_global(rid, crate::api::push::PushMessage::SyncNixpkgs).await;
        }
        "complete" => {
            let (target_version, nixpkgs_commit): (Option<String>, Option<String>) = sqlx::query_as(
                "SELECT target_version, nixpkgs_commit FROM rollouts WHERE id = $1",
            )
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
            crate::api::push::notify_all_rollout_global(rid, crate::api::push::PushMessage::SelfUpdate).await;
            if nixpkgs_commit.is_some() {
                crate::api::push::notify_all_rollout_global(rid, crate::api::push::PushMessage::SyncNixpkgs).await;
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

#[component]
pub fn RolloutDetail(id: String) -> Element {
    let id_clone = id.clone();
    let mut detail = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_rollout_detail(id).await }
    })?;
    let nav = navigator();

    match &*detail.read() {
        Some(Ok(info)) => {
            let rid = info.id.to_string();
            let status = info.status.clone();
            let created = info.created_at.format("%Y-%m-%d %H:%M").to_string();

            let status_badge = match info.status.as_str() {
                "rolling" => "bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200",
                "completed" => "bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200",
                "paused" => "bg-yellow-100 dark:bg-yellow-900 text-yellow-800 dark:text-yellow-200",
                "failed" => "bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200",
                _ => "bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-200",
            };

            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    div {
                        h2 { class: "text-2xl font-bold mb-1", "Rollout" }
                        div { class: "flex items-center gap-2",
                            span { class: "px-2 py-0.5 rounded text-xs font-medium {status_badge}",
                                "{status}"
                            }
                            span { class: "text-gray-500 dark:text-gray-400 text-sm", "Created: {created}" }
                        }
                    }
                    if info.status == "pending" || info.status == "completed" || info.status == "failed" {
                        button {
                            class: "bg-red-600 text-white px-3 py-1 rounded text-sm hover:bg-red-700",
                            onclick: {
                                let rid = rid.clone();
                                move |_| {
                                    let rid = rid.clone();
                                    async move {
                                        let _ = rollout_action(rid, "delete".into()).await;
                                        nav.push(Route::RolloutList {});
                                    }
                                }
                            },
                            "Delete"
                        }
                    }
                }

                // Action buttons
                div { class: "flex gap-2 mb-6",
                    if info.status == "pending" {
                        button {
                            class: "bg-green-600 text-white px-3 py-1 rounded text-sm hover:bg-green-700",
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
                            "Start Rollout"
                        }
                    }
                    if info.status == "rolling" {
                        button {
                            class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700",
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
                            "Advance to Next Stage"
                        }
                        button {
                            class: "bg-yellow-500 text-white px-3 py-1 rounded text-sm hover:bg-yellow-600",
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
                            "Pause"
                        }
                    }
                    if info.status == "paused" {
                        button {
                            class: "bg-green-600 text-white px-3 py-1 rounded text-sm hover:bg-green-700",
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
                            "Resume"
                        }
                    }
                    if info.status != "completed" {
                        button {
                            class: "bg-gray-600 text-white px-3 py-1 rounded text-sm hover:bg-gray-700",
                            onclick: {
                                let rid = rid.clone();
                                move |_| {
                                    let rid = rid.clone();
                                    async move {
                                        let _ = rollout_action(rid, "complete".into()).await;
                                        detail.restart();
                                    }
                                }
                            },
                            "Complete All"
                        }
                    }
                }

                // Stages
                h3 { class: "text-lg font-semibold mb-3", "Stages" }
                div { class: "space-y-3 mb-6",
                    for stage in &info.stages {
                        {
                            let badge_class = match stage.status.as_str() {
                                "rolling" => "bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200",
                                "completed" => "bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200",
                                "paused" => "bg-yellow-100 dark:bg-yellow-900 text-yellow-800 dark:text-yellow-200",
                                _ => "bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-200",
                            };
                            let started = stage
                                .started_at
                                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let completed = stage
                                .completed_at
                                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let health_text = if stage.total_count > 0 {
                                format!("{}/{} online", stage.healthy_count, stage.total_count)
                            } else {
                                "no heartbeats".to_string()
                            };
                            let health_color = if stage.total_count == 0 {
                                "text-gray-400 dark:text-gray-500"
                            } else if stage.healthy_count == stage.total_count {
                                "text-green-600 dark:text-green-400"
                            } else {
                                "text-red-600 dark:text-red-400"
                            };
                            let version_upgrade_text = if stage.total_count > 0 {
                                format!("{}/{} version", stage.upgraded_count, stage.total_count)
                            } else {
                                String::new()
                            };
                            let version_upgrade_color = if stage.total_count == 0 {
                                "text-gray-400 dark:text-gray-500"
                            } else if stage.upgraded_count == stage.total_count {
                                "text-green-600 dark:text-green-400"
                            } else if stage.upgraded_count > 0 {
                                "text-blue-600 dark:text-blue-400"
                            } else {
                                "text-gray-400 dark:text-gray-500"
                            };
                            let nixpkgs_upgrade_text = if stage.total_count > 0 {
                                format!("{}/{} nixpkgs", stage.nixpkgs_upgraded_count, stage.total_count)
                            } else {
                                String::new()
                            };
                            let nixpkgs_upgrade_color = if stage.total_count == 0 {
                                "text-gray-400 dark:text-gray-500"
                            } else if stage.nixpkgs_upgraded_count == stage.total_count {
                                "text-green-600 dark:text-green-400"
                            } else if stage.nixpkgs_upgraded_count > 0 {
                                "text-blue-600 dark:text-blue-400"
                            } else {
                                "text-gray-400 dark:text-gray-500"
                            };

                            let stage_id_str = stage.id.to_string();
                            let rid_clone = rid.clone();

                            rsx! {
                                div { class: "p-4 bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30",
                                    div { class: "flex justify-between items-center mb-2",
                                        div { class: "flex items-center gap-2",
                                            span { class: "font-medium text-sm",
                                                "Stage {stage.stage_order}"
                                            }
                                            span { class: "text-gray-600 dark:text-gray-300", "{stage.group_name}" }
                                            span { class: "px-2 py-0.5 rounded text-xs font-medium {badge_class}",
                                                "{stage.status}"
                                            }
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
                                    div { class: "text-xs text-gray-400 dark:text-gray-500 flex gap-4",
                                        span { "Started: {started}" }
                                        span { "Completed: {completed}" }
                                    }

                                    // ── System-assessment health gate ──
                                    if stage.has_gate {
                                        {
                                            let evaluated_at_text = stage.health.as_ref().map(|h| h.evaluated_at.format("%Y-%m-%d %H:%M:%S").to_string());
                                            let (gate_badge_class, gate_text) = match stage.health.as_ref() {
                                                Some(h) if h.passed && h.in_grace_period => (
                                                    "bg-yellow-100 dark:bg-yellow-900 text-yellow-800 dark:text-yellow-200",
                                                    "gate: grace period".to_string(),
                                                ),
                                                Some(h) if h.passed => (
                                                    "bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200",
                                                    "gate: pass".to_string(),
                                                ),
                                                Some(_) => (
                                                    "bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200",
                                                    "gate: fail".to_string(),
                                                ),
                                                None => (
                                                    "bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300",
                                                    "gate: no data".to_string(),
                                                ),
                                            };
                                            rsx! {
                                                div { class: "mt-3 pt-3 border-t border-gray-200 dark:border-gray-700",
                                                    div { class: "flex justify-between items-center mb-2",
                                                        div { class: "flex items-center gap-2",
                                                            span { class: "text-sm font-semibold text-gray-700 dark:text-gray-200",
                                                                "Assessment gate"
                                                            }
                                                            span { class: "px-2 py-0.5 rounded text-xs font-medium {gate_badge_class}",
                                                                "{gate_text}"
                                                            }
                                                            if let Some(ts) = evaluated_at_text.as_ref() {
                                                                span { class: "text-xs text-gray-500 dark:text-gray-400",
                                                                    "evaluated {ts}"
                                                                }
                                                            }
                                                        }
                                                        div { class: "flex gap-2",
                                                            button {
                                                                class: "px-2 py-1 text-xs rounded bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 text-gray-700 dark:text-gray-200",
                                                                onclick: {
                                                                    let sid = stage_id_str.clone();
                                                                    move |_| {
                                                                        let sid = sid.clone();
                                                                        async move {
                                                                            let _ = reevaluate_stage(sid).await;
                                                                            detail.restart();
                                                                        }
                                                                    }
                                                                },
                                                                "Reevaluate now"
                                                            }
                                                            button {
                                                                class: "px-2 py-1 text-xs rounded bg-blue-100 dark:bg-blue-900 hover:bg-blue-200 dark:hover:bg-blue-800 text-blue-800 dark:text-blue-200",
                                                                onclick: {
                                                                    let rid = rid_clone.clone();
                                                                    let sid = stage_id_str.clone();
                                                                    move |_| {
                                                                        let rid = rid.clone();
                                                                        let sid = sid.clone();
                                                                        async move {
                                                                            let _ = request_stage_assessment(rid, sid).await;
                                                                        }
                                                                    }
                                                                },
                                                                "Request fresh assessment"
                                                            }
                                                        }
                                                    }
                                                    if let Some(h) = stage.health.as_ref() {
                                                        div { class: "flex flex-wrap gap-4 text-xs text-gray-600 dark:text-gray-300",
                                                            span {
                                                                span { class: "font-medium", "cohort: " }
                                                                "{h.cohort_size}"
                                                            }
                                                            span {
                                                                span { class: "font-medium", "heartbeats fresh: " }
                                                                "{h.heartbeat_fresh_pct}%"
                                                            }
                                                            for (svc, pct) in h.probe_ok_pct.iter() {
                                                                span { class: "font-mono",
                                                                    span { class: "font-medium font-sans", "{svc}: " }
                                                                    "{pct}%"
                                                                }
                                                            }
                                                        }
                                                        if !h.reasons.is_empty() {
                                                            ul { class: "mt-2 text-xs text-red-700 dark:text-red-300 list-disc list-inside",
                                                                for reason in h.reasons.iter() {
                                                                    li { "{reason}" }
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
                h3 { class: "text-lg font-semibold mb-2", "Target" }
                div { class: "bg-gray-100 dark:bg-gray-700 p-4 rounded text-sm space-y-1",
                    if let Some(ver) = &info.target_version {
                        p { span { class: "font-medium", "Version: " } "{ver}" }
                    }
                    if let Some(commit) = &info.nixpkgs_commit {
                        p { span { class: "font-medium", "Nixpkgs commit: " } code { class: "font-mono", "{commit}" } }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! {
            p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" }
        },
        None => rsx! {
            p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." }
        },
    }
}
