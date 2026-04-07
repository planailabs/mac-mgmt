use dioxus::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;
use crate::web::components::table_utils::{Searchable, SortableTh, TableToolbar};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RolloutEntry {
    id: Uuid,
    status: String,
    created_at: DateTime<Utc>,
    stage_count: i64,
}

#[server]
async fn get_rollouts() -> Result<Vec<RolloutEntry>, ServerFnError> {
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        status: String,
        created_at: DateTime<Utc>,
        stage_count: i64,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT r.id, r.status, r.created_at, COUNT(rs.id) AS stage_count \
         FROM rollouts r LEFT JOIN rollout_stages rs ON rs.rollout_id = r.id \
         GROUP BY r.id ORDER BY r.created_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| RolloutEntry {
            id: r.id,
            status: r.status,
            created_at: r.created_at,
            stage_count: r.stage_count,
        })
        .collect())
}

#[server]
async fn delete_rollout(id: String) -> Result<(), ServerFnError> {
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
            || self.status.to_lowercase().contains(query)
    }
}

fn status_badge(status: &str) -> (&'static str, &'static str) {
    match status {
        "rolling" => ("bg-blue-100 text-blue-800", "rolling"),
        "completed" => ("bg-green-100 text-green-800", "completed"),
        "paused" => ("bg-yellow-100 text-yellow-800", "paused"),
        "failed" => ("bg-red-100 text-red-800", "failed"),
        _ => ("bg-gray-100 text-gray-800", "pending"),
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
                            class: "bg-gray-200 text-gray-700 px-4 py-2 rounded hover:bg-gray-300",
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
                    p { class: "text-gray-500 text-sm", "No rollouts yet." }
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
                                    "id" => a.id.to_string().cmp(&b.id.to_string()),
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
                            div { class: "bg-white rounded shadow overflow-hidden",
                                table { class: "min-w-full divide-y divide-gray-200",
                                    thead { class: "bg-gray-50",
                                        tr {
                                            SortableTh { label: "ID".to_string(), sort_key: "id".to_string(), sort }
                                            SortableTh { label: "Status".to_string(), sort_key: "status".to_string(), sort }
                                            SortableTh { label: "Stages".to_string(), sort_key: "stages".to_string(), sort }
                                            SortableTh { label: "Created".to_string(), sort_key: "created".to_string(), sort }
                                            th { class: "px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase",
                                                ""
                                            }
                                        }
                                    }
                                    tbody { class: "bg-white divide-y divide-gray-200",
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
                                                                class: "text-blue-600 hover:underline font-mono text-xs",
                                                                "{rid}"
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
                                                        td { class: "px-6 py-4 text-sm text-gray-500",
                                                            "{created}"
                                                        }
                                                        td { class: "px-6 py-4 text-right",
                                                            if can_delete {
                                                                button {
                                                                    class: "text-red-600 hover:text-red-700 text-sm",
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
            p { class: "text-red-600 text-sm", "Error: {e}" }
        },
        None => rsx! {
            p { class: "text-gray-500 text-sm", "Loading..." }
        },
    }
}
