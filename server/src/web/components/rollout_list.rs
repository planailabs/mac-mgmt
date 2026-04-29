use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;
use crate::web::components::table_utils::Searchable;
use crate::web::components::ui::{
    Badge, BadgeVariant, DataTable, ErrorText, HelpText, PageHeader, SortState, SortableTh, Td,
    TdMuted, Th,
};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

fn status_variant(status: &str) -> (BadgeVariant, String) {
    match status {
        "rolling"   => (BadgeVariant::Info,    t!("rollout-status-rolling")),
        "completed" => (BadgeVariant::Success, t!("rollout-status-completed")),
        "paused"    => (BadgeVariant::Warn,    t!("rollout-status-paused")),
        "failed"    => (BadgeVariant::Danger,  t!("rollout-status-failed")),
        _           => (BadgeVariant::Neutral, t!("rollout-status-pending")),
    }
}

fn health_variant(state: &str) -> (BadgeVariant, String) {
    match state {
        "pass"  => (BadgeVariant::Success, t!("rollout-health-pass")),
        "fail"  => (BadgeVariant::Danger,  t!("rollout-health-fail")),
        "grace" => (BadgeVariant::Warn,    t!("rollout-health-grace")),
        _       => (BadgeVariant::Neutral, t!("rollout-health-no-data")),
    }
}

#[component]
pub fn RolloutList() -> Element {
    let rollouts = use_server_future(move || async move { get_rollouts().await })?;

    rsx! {
        div { class: "flex justify-between items-center mb-4",
            PageHeader { class: "mb-0", {t!("rollout-list-title")} }
            div { class: "flex gap-2",
                Link { to: Route::RolloutGroupList {}, class: "btn btn-lg btn-secondary",
                    {t!("rollout-list-manage-groups")}
                }
                Link { to: Route::RolloutForm {}, class: "btn btn-lg btn-primary",
                    {t!("rollout-list-new")}
                }
            }
        }
        {match &*rollouts.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! { HelpText { {t!("rollout-list-no-rollouts")} } }
                } else {
                    rsx! { RolloutsTable { list: list.clone(), rollouts } }
                }
            }
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}

#[component]
fn RolloutsTable(
    list: Vec<RolloutEntry>,
    rollouts: Resource<Result<Vec<RolloutEntry>, ServerFnError>>,
) -> Element {
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);
    let sort = use_signal::<SortState>(|| ("created".to_string(), false));

    let list_clone = list.clone();
    let filtered = use_memo(move || {
        let q = search.read().to_lowercase();
        let mut items: Vec<RolloutEntry> = if q.is_empty() {
            list_clone.clone()
        } else {
            list_clone.iter().filter(|e| e.matches_search(&q)).cloned().collect()
        };
        let (key, asc) = sort.read().clone();
        items.sort_by(|a, b| {
            let ord = match key.as_str() {
                "name" => a.name.as_deref().unwrap_or("").cmp(b.name.as_deref().unwrap_or("")),
                "status" => a.status.cmp(&b.status),
                "stages" => a.stage_count.cmp(&b.stage_count),
                _ => a.created_at.cmp(&b.created_at),
            };
            if asc { ord } else { ord.reverse() }
        });
        items
    });

    let total = list.len();
    let filtered_count = filtered.read().len();
    let limit_val = *limit.read();
    let shown = filtered_count.min(limit_val);

    rsx! {
        DataTable {
            search, limit, total, filtered: filtered_count, shown,
            headers: rsx! {
                SortableTh { label: t!("name"), sort_key: "name".to_string(), sort }
                SortableTh { label: t!("status"), sort_key: "status".to_string(), sort }
                SortableTh { label: t!("rollout-list-col-stages"), sort_key: "stages".to_string(), sort }
                Th { {t!("rollout-list-col-health")} }
                SortableTh { label: t!("created"), sort_key: "created".to_string(), sort }
                Th { "" }
            },
            body: rsx! {
                for r in filtered.read().iter().take(limit_val) {
                    RolloutRow { key: "{r.id}", entry: r.clone(), rollouts }
                }
            },
        }
    }
}

#[component]
fn RolloutRow(
    entry: RolloutEntry,
    rollouts: Resource<Result<Vec<RolloutEntry>, ServerFnError>>,
) -> Element {
    let mut rollouts = rollouts;
    let rid = entry.id.to_string();
    let created = entry.created_at.format("%Y-%m-%d %H:%M").to_string();
    let (status_var, status_text) = status_variant(&entry.status);
    let can_delete =
        entry.status == "pending" || entry.status == "completed" || entry.status == "failed";

    rsx! {
        tr {
            Td { class: "text-sm",
                Link { to: Route::RolloutDetail { id: rid.clone() }, class: "link",
                    if let Some(ref name) = entry.name {
                        span { "{name}" }
                    } else {
                        span { class: "font-mono text-xs", "{rid}" }
                    }
                }
            }
            Td { class: "text-sm",
                Badge { variant: status_var, "{status_text}" }
            }
            Td { class: "text-sm", "{entry.stage_count}" }
            Td { class: "text-sm", HealthCell { health: entry.health.clone() } }
            TdMuted { class: "text-sm", {created} }
            td { class: "td text-right",
                if can_delete {
                    button {
                        class: "link-danger text-sm",
                        onclick: {
                            let rid = rid.clone();
                            move |_| {
                                let rid = rid.clone();
                                async move {
                                    let _ = delete_rollout(rid).await;
                                    rollouts.restart();
                                }
                            }
                        },
                        {t!("delete")}
                    }
                }
            }
        }
    }
}

#[component]
fn HealthCell(health: Option<RolloutHealthSummary>) -> Element {
    let Some(h) = health else {
        return rsx! { span { class: "text-fg-faint", {t!("em-dash")} } };
    };
    let (variant, label) = health_variant(&h.state);
    let title = if h.summary.is_empty() {
        t!("rollout-health-tooltip", evaluated: h.evaluated_stages, failing: h.failing_stages)
    } else {
        t!(
            "rollout-health-tooltip-summary",
            evaluated: h.evaluated_stages,
            failing: h.failing_stages,
            summary: h.summary.clone()
        )
    };
    rsx! {
        span { class: "inline-flex items-center gap-2",
            Badge { variant, title, "{label}" }
            if h.failing_stages > 0 {
                span { class: "text-xs font-mono text-danger-strong",
                    "{h.failing_stages}/{h.evaluated_stages}"
                }
            } else if h.evaluated_stages > 0 {
                span { class: "text-xs font-mono text-fg-muted",
                    "{h.evaluated_stages}/{h.evaluated_stages}"
                }
            }
        }
    }
}
