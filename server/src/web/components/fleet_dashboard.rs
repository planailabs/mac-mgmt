use dioxus::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FleetEntry {
    customer_name: String,
    instance_id: String,
    hostname: String,
    version: String,
    services: serde_json::Value,
    reported_at: DateTime<Utc>,
}

#[server]
async fn get_fleet_status() -> Result<Vec<FleetEntry>, ServerFnError> {
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        customer_name: String,
        instance_id: String,
        hostname: String,
        version: String,
        services: serde_json::Value,
        reported_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT c.name AS customer_name, dh.instance_id, dh.hostname, dh.version, dh.services, dh.reported_at \
         FROM daemon_heartbeats dh \
         JOIN customers c ON c.id = dh.customer_id \
         ORDER BY dh.reported_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| FleetEntry {
            customer_name: r.customer_name,
            instance_id: r.instance_id,
            hostname: r.hostname,
            version: r.version,
            services: r.services,
            reported_at: r.reported_at,
        })
        .collect())
}

#[component]
pub fn FleetDashboard() -> Element {
    let fleet = use_server_future(move || async move { get_fleet_status().await })?;

    match &*fleet.read() {
        Some(Ok(entries)) => {
            rsx! {
                h2 { class: "text-2xl font-bold mb-4", "Fleet Dashboard" }
                if entries.is_empty() {
                    p { class: "text-gray-500 text-sm", "No daemons have reported in yet." }
                } else {
                    div { class: "bg-white rounded shadow overflow-hidden",
                        table { class: "min-w-full divide-y divide-gray-200",
                            thead { class: "bg-gray-50",
                                tr {
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Customer" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Hostname" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Version" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Status" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Services" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Last Seen" }
                                }
                            }
                            tbody { class: "bg-white divide-y divide-gray-200",
                                for entry in entries {
                                    {
                                        let now = Utc::now();
                                        let age = now.signed_duration_since(entry.reported_at);
                                        let is_online = age.num_seconds() < 300;
                                        let status_class = if is_online { "text-green-600 font-semibold" } else { "text-red-600 font-semibold" };
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
                                                        "bg-green-100 text-green-800".to_string()
                                                    } else {
                                                        "bg-red-100 text-red-800".to_string()
                                                    };
                                                    (name, cls)
                                                }).collect()
                                            })
                                            .unwrap_or_default();

                                        rsx! {
                                            tr {
                                                td { class: "px-4 py-2 text-sm", "{entry.customer_name}" }
                                                td { class: "px-4 py-2 text-sm",
                                                    if entry.hostname.is_empty() {
                                                        span { class: "text-gray-400 font-mono text-xs", "{entry.instance_id}" }
                                                    } else {
                                                        span { "{entry.hostname}" }
                                                    }
                                                }
                                                td { class: "px-4 py-2 text-sm", "{entry.version}" }
                                                td { class: "px-4 py-2 text-sm {status_class}", "{status_text}" }
                                                td { class: "px-4 py-2 text-sm",
                                                    div { class: "flex gap-1 flex-wrap",
                                                        for (name, badge_class) in &services_badges {
                                                            span { class: "inline-block px-2 py-0.5 rounded text-xs font-medium {badge_class}",
                                                                "{name}"
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "px-4 py-2 text-sm text-gray-500", "{last_seen}" }
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
