use dioxus::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RolloutInfo {
    id: Uuid,
    config_toml: String,
    status: String,
    created_at: DateTime<Utc>,
    stages: Vec<StageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StageInfo {
    id: Uuid,
    group_name: String,
    stage_order: i32,
    status: String,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
}

#[server]
async fn get_rollout_detail(id: String) -> Result<RolloutInfo, ServerFnError> {
    let pool = crate::server_pool()?;
    let rid: Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct RRow { id: Uuid, config_toml: String, status: String, created_at: DateTime<Utc> }

    let rollout = sqlx::query_as::<_, RRow>("SELECT id, config_toml, status, created_at FROM rollouts WHERE id = $1")
        .bind(rid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct SRow { id: Uuid, group_name: String, stage_order: i32, status: String, started_at: Option<DateTime<Utc>>, completed_at: Option<DateTime<Utc>> }

    let stages = sqlx::query_as::<_, SRow>(
        "SELECT rs.id, rg.name AS group_name, rs.stage_order, rs.status, rs.started_at, rs.completed_at \
         FROM rollout_stages rs JOIN rollout_groups rg ON rg.id = rs.group_id \
         WHERE rs.rollout_id = $1 ORDER BY rs.stage_order"
    )
    .bind(rid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(RolloutInfo {
        id: rollout.id,
        config_toml: rollout.config_toml,
        status: rollout.status,
        created_at: rollout.created_at,
        stages: stages.into_iter().map(|s| StageInfo {
            id: s.id, group_name: s.group_name, stage_order: s.stage_order,
            status: s.status, started_at: s.started_at, completed_at: s.completed_at,
        }).collect(),
    })
}

#[server]
async fn rollout_action(id: String, action: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let rid: Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    match action.as_str() {
        "start" => {
            let mut tx = pool.begin().await.map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollouts SET status = 'rolling', updated_at = now() WHERE id = $1")
                .bind(rid).execute(&mut *tx).await.map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollout_stages SET status = 'rolling', started_at = now() WHERE rollout_id = $1 AND stage_order = 0")
                .bind(rid).execute(&mut *tx).await.map_err(|e| ServerFnError::new(e.to_string()))?;
            tx.commit().await.map_err(|e| ServerFnError::new(e.to_string()))?;
        }
        "advance" => {
            #[derive(sqlx::FromRow)]
            struct SO { stage_order: i32 }
            let current = sqlx::query_as::<_, SO>(
                "SELECT stage_order FROM rollout_stages WHERE rollout_id = $1 AND status = 'rolling' LIMIT 1"
            ).bind(rid).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

            let mut tx = pool.begin().await.map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollout_stages SET status = 'completed', completed_at = now() WHERE rollout_id = $1 AND stage_order = $2")
                .bind(rid).bind(current.stage_order).execute(&mut *tx).await.map_err(|e| ServerFnError::new(e.to_string()))?;
            let next = current.stage_order + 1;
            let updated = sqlx::query("UPDATE rollout_stages SET status = 'rolling', started_at = now() WHERE rollout_id = $1 AND stage_order = $2")
                .bind(rid).bind(next).execute(&mut *tx).await.map_err(|e| ServerFnError::new(e.to_string()))?;
            if updated.rows_affected() == 0 {
                sqlx::query("UPDATE rollouts SET status = 'completed', updated_at = now() WHERE id = $1")
                    .bind(rid).execute(&mut *tx).await.map_err(|e| ServerFnError::new(e.to_string()))?;
            } else {
                sqlx::query("UPDATE rollouts SET updated_at = now() WHERE id = $1")
                    .bind(rid).execute(&mut *tx).await.map_err(|e| ServerFnError::new(e.to_string()))?;
            }
            tx.commit().await.map_err(|e| ServerFnError::new(e.to_string()))?;
        }
        "pause" => {
            let mut tx = pool.begin().await.map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollout_stages SET status = 'paused' WHERE rollout_id = $1 AND status = 'rolling'")
                .bind(rid).execute(&mut *tx).await.map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollouts SET status = 'paused', updated_at = now() WHERE id = $1")
                .bind(rid).execute(&mut *tx).await.map_err(|e| ServerFnError::new(e.to_string()))?;
            tx.commit().await.map_err(|e| ServerFnError::new(e.to_string()))?;
        }
        "complete" => {
            let config_toml: String = sqlx::query_scalar("SELECT config_toml FROM rollouts WHERE id = $1")
                .bind(rid).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
            let customer_ids: Vec<Uuid> = sqlx::query_scalar(
                "SELECT DISTINCT rgm.customer_id FROM rollout_stages rs \
                 JOIN rollout_group_members rgm ON rgm.group_id = rs.group_id WHERE rs.rollout_id = $1"
            ).bind(rid).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

            let mut tx = pool.begin().await.map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollout_stages SET status = 'completed', completed_at = COALESCE(completed_at, now()) WHERE rollout_id = $1")
                .bind(rid).execute(&mut *tx).await.map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query("UPDATE rollouts SET status = 'completed', updated_at = now() WHERE id = $1")
                .bind(rid).execute(&mut *tx).await.map_err(|e| ServerFnError::new(e.to_string()))?;
            for cid in &customer_ids {
                sqlx::query("INSERT INTO customer_configs (customer_id, config_toml) VALUES ($1, $2)")
                    .bind(cid).bind(&config_toml).execute(&mut *tx).await.map_err(|e| ServerFnError::new(e.to_string()))?;
            }
            tx.commit().await.map_err(|e| ServerFnError::new(e.to_string()))?;
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

    match &*detail.read() {
        Some(Ok(info)) => {
            let rid = info.id.to_string();
            let status = info.status.clone();
            let created = info.created_at.format("%Y-%m-%d %H:%M").to_string();
            rsx! {
                h2 { class: "text-2xl font-bold mb-2", "Rollout" }
                p { class: "text-gray-500 text-sm mb-1", "Status: {status}" }
                p { class: "text-gray-500 text-sm mb-4", "Created: {created}" }

                div { class: "flex gap-2 mb-6",
                    if info.status == "pending" {
                        button {
                            class: "bg-green-500 text-white px-3 py-1 rounded text-sm hover:bg-green-600",
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
                            "Start"
                        }
                    }
                    if info.status == "rolling" {
                        button {
                            class: "bg-blue-500 text-white px-3 py-1 rounded text-sm hover:bg-blue-600",
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
                            "Advance"
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
                    if info.status != "completed" {
                        button {
                            class: "bg-gray-500 text-white px-3 py-1 rounded text-sm hover:bg-gray-600",
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

                h3 { class: "text-lg font-semibold mb-2", "Stages" }
                div { class: "space-y-2",
                    for stage in &info.stages {
                        {
                            let badge_class = match stage.status.as_str() {
                                "rolling" => "bg-blue-100 text-blue-800",
                                "completed" => "bg-green-100 text-green-800",
                                "paused" => "bg-yellow-100 text-yellow-800",
                                _ => "bg-gray-100 text-gray-800",
                            };
                            rsx! {
                                div { class: "p-3 bg-white rounded shadow flex justify-between items-center",
                                    div {
                                        span { class: "font-medium", "Stage {stage.stage_order}: " }
                                        span { "{stage.group_name}" }
                                    }
                                    span { class: "px-2 py-0.5 rounded text-xs font-medium {badge_class}", "{stage.status}" }
                                }
                            }
                        }
                    }
                }

                h3 { class: "text-lg font-semibold mt-6 mb-2", "Config" }
                pre { class: "bg-gray-100 p-4 rounded text-xs font-mono overflow-auto max-h-64",
                    "{info.config_toml}"
                }
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
        None => rsx! { p { "Loading..." } },
    }
}
