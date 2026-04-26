use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;
use crate::web::components::table_utils::{Searchable, SortableTh, TableToolbar};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RolloutEntry {
    id: Uuid,
    #[serde(default)]
    name: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    stage_count: i64,
    /// Latest aggregated gate state across the rollout's rolling stages.
    /// `None` when the rollout has no rolling stages or no stage with a
    /// configured health_gate (legacy rollouts).
    #[serde(default)]
    health: Option<RolloutHealthSummary>,
}

/// Compact health rollup for a single rollout — one row, one badge, one
/// tooltip. Rendered in the list view's Health column. Reads the most
/// recent `rollout_stage_health_evaluations` row per rolling stage rather
/// than re-evaluating live (the auto-pause loop ticks every 60s).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RolloutHealthSummary {
    /// "pass" | "grace" | "fail" | "no_data"
    state: String,
    /// Stages with a gate that have at least one evaluation.
    evaluated_stages: u32,
    /// Stages whose last evaluation failed.
    failing_stages: u32,
    /// Top reason text from any failing stage, truncated. Empty when
    /// no stage failed.
    summary: String,
}

#[server]
async fn get_rollouts() -> Result<Vec<RolloutEntry>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

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
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Pull the latest evaluation per rolling stage in one query, then
    // collapse to one summary per rollout. Cheaper than per-rollout
    // queries when the list has many entries.
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
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut health_by_rollout: std::collections::HashMap<Uuid, RolloutHealthSummary> =
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
        let entry =
            health_by_rollout
                .entry(ev.rollout_id)
                .or_insert_with(|| RolloutHealthSummary {
                    state: "pass".into(),
                    evaluated_stages: 0,
                    failing_stages: 0,
                    summary: String::new(),
                });
        entry.evaluated_stages += 1;
        // State precedence: fail > grace (only when grace is actually
        // shielding a real reason) > pass. A clean pass during grace is
        // just a pass.
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
            // Match the detail-page rule: only show health when the
            // rollout is rolling AND has at least one gated stage that's
            // been evaluated. The query above already filters by
            // health_gate IS NOT NULL, so any entry in health_by_rollout
            // is already "has gate, has at least one eval".
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

#[server]
async fn delete_rollout(id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let rid: Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM rollouts WHERE id = $1")
        .bind(rid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

impl Searchable for RolloutEntry {
    fn matches_search(&self, query: &str) -> bool {
        self.id.to_string().to_lowercase().contains(query)
            || self.name.as_deref().unwrap_or("").to_lowercase().contains(query)
            || self.status.to_lowercase().contains(query)
    }
}

/// Render the Health column cell for one rollout. Non-rolling rollouts
/// get an em dash; rolling rollouts get a coloured pill plus an
/// `evaluated/total` count and a tooltip carrying the top failure reason.
fn render_health_cell(health: Option<&RolloutHealthSummary>) -> Element {
    let Some(h) = health else {
        return rsx! { span { class: "text-gray-400 dark:text-gray-500", "—" } };
    };
    let (cls, label) = match h.state.as_str() {
        "pass" => (
            "bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200",
            "pass",
        ),
        "fail" => (
            "bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200",
            "fail",
        ),
        "grace" => (
            "bg-yellow-100 dark:bg-yellow-900 text-yellow-800 dark:text-yellow-200",
            "grace",
        ),
        _ => (
            "bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300",
            "no data",
        ),
    };
    let title = if h.summary.is_empty() {
        format!(
            "{} stage(s) evaluated, {} failing",
            h.evaluated_stages, h.failing_stages
        )
    } else {
        format!(
            "{} stage(s) evaluated, {} failing — {}",
            h.evaluated_stages, h.failing_stages, h.summary
        )
    };
    rsx! {
        span { class: "inline-flex items-center gap-2",
            span {
                class: "px-2 py-0.5 rounded text-xs font-medium {cls}",
                title: "{title}",
                "{label}"
            }
            if h.failing_stages > 0 {
                span { class: "text-xs font-mono text-red-700 dark:text-red-300",
                    "{h.failing_stages}/{h.evaluated_stages}"
                }
            } else if h.evaluated_stages > 0 {
                span { class: "text-xs font-mono text-gray-500 dark:text-gray-400",
                    "{h.evaluated_stages}/{h.evaluated_stages}"
                }
            }
        }
    }
}

fn status_badge(status: &str) -> (&'static str, &'static str) {
    match status {
        "rolling" => (
            "bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200",
            "rolling",
        ),
        "completed" => (
            "bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200",
            "completed",
        ),
        "paused" => (
            "bg-yellow-100 dark:bg-yellow-900 text-yellow-800 dark:text-yellow-200",
            "paused",
        ),
        "failed" => (
            "bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200",
            "failed",
        ),
        _ => (
            "bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-200",
            "pending",
        ),
    }
}

#[component]
pub fn RolloutList() -> Element {
    let mut rollouts = use_server_future(move || async move { get_rollouts().await })?;

    match &*rollouts.read() {
        Some(Ok(list)) => {
            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    h2 { class: "text-2xl font-bold", "Rollouts" }
                    div { class: "flex gap-2",
                        Link {
                            to: Route::RolloutGroupList {},
                            class: "bg-gray-200 dark:bg-gray-600 text-gray-700 dark:text-gray-200 px-4 py-2 rounded hover:bg-gray-300 dark:hover:bg-gray-500",
                            "Manage Groups"
                        }
                        Link {
                            to: Route::RolloutForm {},
                            class: "bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700",
                            "New Rollout"
                        }
                    }
                }
                if list.is_empty() {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", "No rollouts yet." }
                } else {
                    {
                        let search = use_signal(String::new);
                        let limit = use_signal(|| 20usize);
                        let sort = use_signal(|| ("created".to_string(), false));

                        let list_clone = list.clone();
                        let mut filtered: Vec<RolloutEntry> = {
                            let q = search.read().to_lowercase();
                            if q.is_empty() {
                                list_clone.clone()
                            } else {
                                list_clone.iter().filter(|e| e.matches_search(&q)).cloned().collect()
                            }
                        };

                        {
                            let (key, asc) = sort.read().clone();
                            filtered.sort_by(|a, b| {
                                let ord = match key.as_str() {
                                    "name" => a.name.as_deref().unwrap_or("").cmp(b.name.as_deref().unwrap_or("")),
                                    "status" => a.status.cmp(&b.status),
                                    "stages" => a.stage_count.cmp(&b.stage_count),
                                    _ => a.created_at.cmp(&b.created_at),
                                };
                                if asc { ord } else { ord.reverse() }
                            });
                        }

                        let total = list.len();
                        let filtered_count = filtered.len();
                        let limit_val = *limit.read();
                        let shown = filtered_count.min(limit_val);

                        rsx! {
                            TableToolbar { search, limit, total, filtered: filtered_count, shown }
                            div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 overflow-hidden",
                                table { class: "min-w-full divide-y divide-gray-200 dark:divide-gray-700",
                                    thead { class: "bg-gray-50 dark:bg-gray-700",
                                        tr {
                                            SortableTh { label: "Name".to_string(), sort_key: "name".to_string(), sort }
                                            SortableTh { label: "Status".to_string(), sort_key: "status".to_string(), sort }
                                            SortableTh { label: "Stages".to_string(), sort_key: "stages".to_string(), sort }
                                            th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Health" }
                                            SortableTh { label: "Created".to_string(), sort_key: "created".to_string(), sort }
                                            th { class: "px-6 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase",
                                                ""
                                            }
                                        }
                                    }
                                    tbody { class: "bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700",
                                        for r in filtered.into_iter().take(limit_val) {
                                            {
                                                let rid = r.id.to_string();
                                                let created =
                                                    r.created_at.format("%Y-%m-%d %H:%M").to_string();
                                                let (badge_class, badge_text) =
                                                    status_badge(&r.status);
                                                let can_delete = r.status == "pending"
                                                    || r.status == "completed"
                                                    || r.status == "failed";
                                                rsx! {
                                                    tr {
                                                        td { class: "px-6 py-4 text-sm",
                                                            Link {
                                                                to: Route::RolloutDetail {
                                                                    id: rid.clone(),
                                                                },
                                                                class: "text-blue-600 dark:text-blue-400 hover:underline",
                                                                if let Some(ref name) = r.name {
                                                                    span { "{name}" }
                                                                } else {
                                                                    span { class: "font-mono text-xs", "{rid}" }
                                                                }
                                                            }
                                                        }
                                                        td { class: "px-6 py-4 text-sm",
                                                            span { class: "px-2 py-0.5 rounded text-xs font-medium {badge_class}",
                                                                "{badge_text}"
                                                            }
                                                        }
                                                        td { class: "px-6 py-4 text-sm",
                                                            "{r.stage_count}"
                                                        }
                                                        td { class: "px-6 py-4 text-sm",
                                                            {render_health_cell(r.health.as_ref())}
                                                        }
                                                        td { class: "px-6 py-4 text-sm text-gray-500 dark:text-gray-400",
                                                            "{created}"
                                                        }
                                                        td { class: "px-6 py-4 text-right",
                                                            if can_delete {
                                                                button {
                                                                    class: "text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 text-sm",
                                                                    onclick: {
                                                                        let rid = rid.clone();
                                                                        move |_| {
                                                                            let rid = rid.clone();
                                                                            async move {
                                                                                let _ =
                                                                                    delete_rollout(rid)
                                                                                        .await;
                                                                                rollouts.restart();
                                                                            }
                                                                        }
                                                                    },
                                                                    "Delete"
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
