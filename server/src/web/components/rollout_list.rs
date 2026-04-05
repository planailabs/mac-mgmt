use dioxus::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;

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
    struct Row { id: Uuid, status: String, created_at: DateTime<Utc>, stage_count: i64 }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT r.id, r.status, r.created_at, COUNT(rs.id) AS stage_count \
         FROM rollouts r LEFT JOIN rollout_stages rs ON rs.rollout_id = r.id \
         GROUP BY r.id ORDER BY r.created_at DESC"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|r| RolloutEntry {
        id: r.id, status: r.status, created_at: r.created_at, stage_count: r.stage_count,
    }).collect())
}

#[component]
pub fn RolloutList() -> Element {
    let rollouts = use_server_future(move || async move { get_rollouts().await })?;

    match &*rollouts.read() {
        Some(Ok(list)) => {
            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    h2 { class: "text-2xl font-bold", "Rollouts" }
                    Link { to: Route::RolloutForm {},
                        class: "bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600",
                        "New Rollout"
                    }
                }
                if list.is_empty() {
                    p { class: "text-gray-500", "No rollouts yet." }
                } else {
                    div { class: "bg-white rounded shadow overflow-hidden",
                        table { class: "min-w-full divide-y divide-gray-200",
                            thead { class: "bg-gray-50",
                                tr {
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "ID" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Status" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Stages" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Created" }
                                }
                            }
                            tbody { class: "divide-y divide-gray-200",
                                for r in list {
                                    tr {
                                        td { class: "px-4 py-2 text-sm",
                                            Link { to: Route::RolloutDetail { id: r.id.to_string() },
                                                class: "text-blue-600 hover:underline font-mono text-xs",
                                                "{r.id}"
                                            }
                                        }
                                        td { class: "px-4 py-2 text-sm",
                                            {
                                                let badge_class = match r.status.as_str() {
                                                    "rolling" => "bg-blue-100 text-blue-800",
                                                    "completed" => "bg-green-100 text-green-800",
                                                    "paused" => "bg-yellow-100 text-yellow-800",
                                                    "failed" => "bg-red-100 text-red-800",
                                                    _ => "bg-gray-100 text-gray-800",
                                                };
                                                rsx! {
                                                    span { class: "px-2 py-0.5 rounded text-xs font-medium {badge_class}",
                                                        "{r.status}"
                                                    }
                                                }
                                            }
                                        }
                                        td { class: "px-4 py-2 text-sm", "{r.stage_count}" }
                                        {
                                            let created = r.created_at.format("%Y-%m-%d %H:%M").to_string();
                                            rsx! {
                                                td { class: "px-4 py-2 text-sm text-gray-500", "{created}" }
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
        Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
        None => rsx! { p { "Loading..." } },
    }
}
