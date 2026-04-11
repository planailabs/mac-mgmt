use dioxus::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::web::components::table_utils::{Searchable, SortableTh, TableToolbar};
use crate::web::app::Route;
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FleetEntry {
    cluster_id: String,
    cluster_name: String,
    instance_id: String,
    hostname: String,
    environment: String,
    version: String,
    services: serde_json::Value,
    reported_at: DateTime<Utc>,
}

#[server]
async fn get_fleet_status() -> Result<Vec<FleetEntry>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        cluster_id: uuid::Uuid,
        cluster_name: String,
        instance_id: String,
        hostname: String,
        environment: String,
        version: String,
        services: serde_json::Value,
        reported_at: DateTime<Utc>,
    }

    let accessible = user.accessible_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let rows = if let Some(ids) = accessible {
        sqlx::query_as::<_, Row>(
            "SELECT c.id AS cluster_id, c.name AS cluster_name, dh.instance_id, dh.hostname, dh.environment, dh.version, dh.services, dh.reported_at \
             FROM daemon_heartbeats dh \
             JOIN clusters c ON c.id = dh.cluster_id \
             WHERE dh.cluster_id = ANY($1) \
             ORDER BY dh.reported_at DESC",
        )
        .bind(&ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, Row>(
            "SELECT c.id AS cluster_id, c.name AS cluster_name, dh.instance_id, dh.hostname, dh.environment, dh.version, dh.services, dh.reported_at \
             FROM daemon_heartbeats dh \
             JOIN clusters c ON c.id = dh.cluster_id \
             ORDER BY dh.reported_at DESC",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows
        .into_iter()
        .map(|r| FleetEntry {
            cluster_id: r.cluster_id.to_string(),
            cluster_name: r.cluster_name,
            instance_id: r.instance_id,
            hostname: r.hostname,
            environment: r.environment,
            version: r.version,
            services: r.services,
            reported_at: r.reported_at,
        })
        .collect())
}

impl Searchable for FleetEntry {
    fn matches_search(&self, query: &str) -> bool {
        self.cluster_name.to_lowercase().contains(query)
            || self.hostname.to_lowercase().contains(query)
            || self.instance_id.to_lowercase().contains(query)
            || self.version.to_lowercase().contains(query)
            || self.environment.to_lowercase().contains(query)
    }
}

#[component]
pub fn FleetDashboard() -> Element {
    let fleet = use_server_future(move || async move { get_fleet_status().await })?;

    match &*fleet.read() {
        Some(Ok(entries)) => {
            let search = use_signal(String::new);
            let limit = use_signal(|| 20usize);
            let sort = use_signal(|| ("last_seen".to_string(), false));

            let entries_clone = entries.clone();
            let mut filtered: Vec<FleetEntry> = {
                let q = search.read().to_lowercase();
                if q.is_empty() {
                    entries_clone.clone()
                } else {
                    entries_clone.iter().filter(|e| e.matches_search(&q)).cloned().collect()
                }
            };

            {
                let (key, asc) = sort.read().clone();
                filtered.sort_by(|a, b| {
                    let ord = match key.as_str() {
                        "cluster" => a.cluster_name.to_lowercase().cmp(&b.cluster_name.to_lowercase()),
                        "hostname" => a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()),
                        "env" => a.environment.to_lowercase().cmp(&b.environment.to_lowercase()),
                        "version" => a.version.cmp(&b.version),
                        _ => a.reported_at.cmp(&b.reported_at),
                    };
                    if asc { ord } else { ord.reverse() }
                });
            }

            let total = entries.len();
            let filtered_count = filtered.len();
            let limit_val = *limit.read();
            let shown = filtered_count.min(limit_val);

            rsx! {
                h2 { class: "text-2xl font-bold mb-4", "Fleet Dashboard" }
                if entries.is_empty() {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", "No daemons have reported in yet." }
                } else {
                    TableToolbar { search, limit, total, filtered: filtered_count, shown }
                    div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 overflow-hidden",
                        table { class: "min-w-full divide-y divide-gray-200 dark:divide-gray-700",
                            thead { class: "bg-gray-50 dark:bg-gray-700",
                                tr {
                                    SortableTh { label: "Cluster".to_string(), sort_key: "cluster".to_string(), sort }
                                    SortableTh { label: "Hostname".to_string(), sort_key: "hostname".to_string(), sort }
                                    SortableTh { label: "Env".to_string(), sort_key: "env".to_string(), sort }
                                    SortableTh { label: "Version".to_string(), sort_key: "version".to_string(), sort }
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Status" }
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Services" }
                                    SortableTh { label: "Last Seen".to_string(), sort_key: "last_seen".to_string(), sort }
                                }
                            }
                            tbody { class: "bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700",
                                for entry in filtered.into_iter().take(limit_val) {
                                    {
                                        let now = Utc::now();
                                        let age = now.signed_duration_since(entry.reported_at);
                                        let is_online = age.num_seconds() < 300;
                                        let status_class = if is_online { "text-green-600 dark:text-green-400 font-semibold" } else { "text-red-600 dark:text-red-400 font-semibold" };
                                        let status_text = if is_online { "online" } else { "offline" };
                                        let last_seen = if age.num_seconds() < 60 {
                                            "just now".to_string()
                                        } else if age.num_minutes() < 60 {
                                            format!("{}m ago", age.num_minutes())
                                        } else if age.num_hours() < 24 {
                                            format!("{}h ago", age.num_hours())
                                        } else {
                                            entry.reported_at.format("%Y-%m-%d %H:%M").to_string()
                                        };

                                        let services_badges: Vec<(String, String)> = entry.services
                                            .as_array()
                                            .map(|arr| {
                                                arr.iter().map(|s| {
                                                    let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                                    let healthy = s.get("healthy").and_then(|v| v.as_bool()).unwrap_or(false);
                                                    let cls = if healthy {
                                                        "bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200".to_string()
                                                    } else {
                                                        "bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200".to_string()
                                                    };
                                                    (name, cls)
                                                }).collect()
                                            })
                                            .unwrap_or_default();

                                        rsx! {
                                            tr {
                                                td { class: "px-6 py-4 text-sm",
                                                    Link {
                                                        to: Route::ClusterDetail { id: entry.cluster_id.clone() },
                                                        class: "text-blue-600 dark:text-blue-400 hover:underline",
                                                        "{entry.cluster_name}"
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm",
                                                    if entry.hostname.is_empty() {
                                                        span { class: "text-gray-400 dark:text-gray-500 font-mono text-xs", "{entry.instance_id}" }
                                                    } else {
                                                        span { "{entry.hostname}" }
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm",
                                                    if !entry.environment.is_empty() {
                                                        span { class: "px-2 py-0.5 rounded text-xs font-medium bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-200",
                                                            "{entry.environment}"
                                                        }
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm", "{entry.version}" }
                                                td { class: "px-6 py-4 text-sm {status_class}", "{status_text}" }
                                                td { class: "px-6 py-4 text-sm",
                                                    div { class: "flex gap-1 flex-wrap",
                                                        for (name, badge_class) in &services_badges {
                                                            span { class: "inline-block px-2 py-0.5 rounded text-xs font-medium {badge_class}",
                                                                "{name}"
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm text-gray-500 dark:text-gray-400", "{last_seen}" }
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
