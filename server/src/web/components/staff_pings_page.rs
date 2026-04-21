use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use crate::web::user::current_user;

// ── Wire types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffPingRow {
    pub id: String,
    pub session_id: String,
    pub instance_id: String,
    pub cluster_name: String,
    pub category: String,
    pub message: String,
    pub resolved: bool,
    pub resolved_by: Option<String>,
    pub created_at: String,
}

// ── Server functions ───────────────────────────────────────────────────

#[server]
pub async fn list_all_staff_pings() -> Result<Vec<StaffPingRow>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    let accessible = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        session_id: uuid::Uuid,
        instance_id: String,
        cluster_name: Option<String>,
        category: String,
        message: String,
        resolved: bool,
        resolved_by: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let rows = match accessible {
        None => {
            // Admin: all pings
            sqlx::query_as::<_, Row>(
                "SELECT p.id, p.session_id, p.instance_id, c.name AS cluster_name, \
                        p.category, p.message, p.resolved, p.resolved_by, p.created_at \
                 FROM healer_staff_pings p \
                 JOIN clusters c ON c.id = p.cluster_id \
                 ORDER BY p.resolved ASC, p.created_at DESC \
                 LIMIT 200",
            )
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        }
        Some(ids) => {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            sqlx::query_as::<_, Row>(
                "SELECT p.id, p.session_id, p.instance_id, c.name AS cluster_name, \
                        p.category, p.message, p.resolved, p.resolved_by, p.created_at \
                 FROM healer_staff_pings p \
                 JOIN clusters c ON c.id = p.cluster_id \
                 WHERE p.cluster_id = ANY($1) \
                 ORDER BY p.resolved ASC, p.created_at DESC \
                 LIMIT 200",
            )
            .bind(&ids)
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        }
    };

    Ok(rows
        .into_iter()
        .map(|r| StaffPingRow {
            id: r.id.to_string(),
            session_id: r.session_id.to_string(),
            instance_id: r.instance_id,
            cluster_name: r.cluster_name.unwrap_or_default(),
            category: r.category,
            message: r.message,
            resolved: r.resolved,
            resolved_by: r.resolved_by,
            created_at: r.created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect())
}

#[server]
pub async fn resolve_ping(ping_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = ping_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    mac_mgmt_healer::session::store::resolve_staff_ping(&pool, uuid, &user.email)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

// ── Component ──────────────────────────────────────────────────────────

#[component]
pub fn StaffPings() -> Element {
    let mut pings = use_signal::<Vec<StaffPingRow>>(Vec::new);
    let mut loaded = use_signal(|| false);
    let mut error_msg = use_signal::<Option<String>>(|| None);

    // Load on mount
    use_future(move || async move {
        match list_all_staff_pings().await {
            Ok(p) => {
                pings.set(p);
                loaded.set(true);
            }
            Err(e) => {
                error_msg.set(Some(e.to_string()));
                loaded.set(true);
            }
        }
    });

    if !*loaded.read() {
        return rsx! { p { class: "text-gray-500 text-sm", "Loading staff pings..." } };
    }
    if let Some(err) = &*error_msg.read() {
        return rsx! { p { class: "text-red-600 text-sm", "Error: {err}" } };
    }

    let all = pings.read();
    let unresolved: Vec<_> = all.iter().filter(|p| !p.resolved).cloned().collect();
    let resolved: Vec<_> = all.iter().filter(|p| p.resolved).cloned().collect();
    let unresolved_count = unresolved.len();
    let resolved_count = resolved.len();

    rsx! {
        h2 { class: "text-2xl font-bold mb-4", "Staff Pings" }
        p { class: "text-sm text-gray-500 dark:text-gray-400 mb-6",
            "Actionable notifications from the healer agent."
        }

        if !unresolved.is_empty() {
            div { class: "mb-8",
                h3 { class: "text-lg font-semibold mb-3 flex items-center gap-2",
                    span { class: "inline-block w-2.5 h-2.5 rounded-full bg-red-500" }
                    "Open ({unresolved_count})"
                }
                div { class: "space-y-2",
                    for ping in unresolved.iter() {
                        { render_ping_card(ping, pings) }
                    }
                }
            }
        } else {
            div { class: "mb-8 p-6 text-center text-gray-400 dark:text-gray-500 bg-white dark:bg-gray-800 rounded shadow",
                "No open staff pings"
            }
        }

        if !resolved.is_empty() {
            div {
                h3 { class: "text-lg font-semibold mb-3 text-gray-500 dark:text-gray-400",
                    "Resolved ({resolved_count})"
                }
                div { class: "space-y-2 opacity-60",
                    for ping in resolved.iter() {
                        { render_ping_card(ping, pings) }
                    }
                }
            }
        }
    }
}

fn render_ping_card(ping: &StaffPingRow, pings: Signal<Vec<StaffPingRow>>) -> Element {
    let cat_badge = category_badge(&ping.category);
    let ping_id = ping.id.clone();
    let session_url = format!("/fleet/{}/healer/{}", ping.instance_id, ping.session_id);
    let is_resolved = ping.resolved;
    let message = ping.message.clone();
    let category = ping.category.clone();
    let cluster_name = ping.cluster_name.clone();
    let instance_id = ping.instance_id.clone();
    let created_at = ping.created_at.clone();
    let resolved_by = ping.resolved_by.clone();

    rsx! {
        div { class: "p-4 bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30",
            div { class: "flex items-start justify-between gap-4",
                div { class: "flex-1 min-w-0",
                    div { class: "flex items-center gap-2 mb-1 flex-wrap",
                        span { class: "inline-block px-2 py-0.5 text-xs font-medium rounded {cat_badge}", "{category}" }
                        span { class: "text-xs text-gray-500 dark:text-gray-400", "{cluster_name}" }
                        span { class: "text-xs text-gray-400 dark:text-gray-500", "{instance_id}" }
                        span { class: "text-xs text-gray-400", "{created_at}" }
                    }
                    {
                        let html = crate::web::components::healer_page::simple_md_to_html(&message);
                        let class = if is_resolved {
                            "text-sm text-gray-800 dark:text-gray-200 prose prose-sm dark:prose-invert max-w-none line-through"
                        } else {
                            "text-sm text-gray-800 dark:text-gray-200 prose prose-sm dark:prose-invert max-w-none"
                        };
                        rsx! {
                            div { class: "{class}", dangerous_inner_html: "{html}" }
                        }
                    }
                    if let Some(by) = &resolved_by {
                        p { class: "text-xs text-green-600 dark:text-green-400 mt-1", "Resolved by {by}" }
                    }
                }
                div { class: "flex items-center gap-2 shrink-0",
                    Link {
                        to: session_url,
                        class: "px-2 py-1 text-xs bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300 rounded hover:bg-blue-200",
                        "View session"
                    }
                    if !is_resolved {
                        button {
                            class: "px-2 py-1 text-xs bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300 rounded hover:bg-green-200",
                            onclick: {
                                let mut pings = pings;
                                move |_| {
                                    let ping_id = ping_id.clone();
                                    async move {
                                        if resolve_ping(ping_id.clone()).await.is_ok() {
                                            pings.with_mut(|list| {
                                                if let Some(p) = list.iter_mut().find(|p| p.id == ping_id) {
                                                    p.resolved = true;
                                                }
                                            });
                                        }
                                    }
                                }
                            },
                            "Resolve"
                        }
                    }
                }
            }
        }
    }
}

fn category_badge(cat: &str) -> &'static str {
    match cat {
        "hardware" => "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300",
        "network" => "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300",
        "disk_space" => "bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-300",
        "config_error" => "bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300",
        "service_crash" => "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300",
        "model_issue" => "bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-300",
        "permission" => "bg-pink-100 text-pink-800 dark:bg-pink-900 dark:text-pink-300",
        "dependency" => "bg-indigo-100 text-indigo-800 dark:bg-indigo-900 dark:text-indigo-300",
        "security" => "bg-red-200 text-red-900 dark:bg-red-800 dark:text-red-200",
        "performance" => "bg-cyan-100 text-cyan-800 dark:bg-cyan-900 dark:text-cyan-300",
        _ => "bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300",
    }
}
