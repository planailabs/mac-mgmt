use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;
use crate::web::components::table_utils::{SortableTh, TableToolbar};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCluster {
    pub id: Uuid,
    pub name: String,
    pub instances: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRollout {
    pub id: Uuid,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[server]
async fn get_rollouts_for_version(
    version: String,
) -> Result<Vec<VersionRollout>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        status: String,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, status, created_at FROM rollouts \
         WHERE target_version = $1 \
         ORDER BY created_at DESC",
    )
    .bind(&version)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| VersionRollout {
            id: r.id,
            status: r.status,
            created_at: r.created_at,
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedCluster {
    pub id: Uuid,
    pub name: String,
}

#[server]
async fn get_clusters_pinned_to(
    version: String,
) -> Result<Vec<PinnedCluster>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name FROM clusters WHERE pinned_version = $1 ORDER BY name",
    )
    .bind(&version)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| PinnedCluster { id: r.id, name: r.name })
        .collect())
}

#[server]
async fn get_clusters_on_version(
    version: String,
) -> Result<Vec<VersionCluster>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
        instances: i64,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT c.id, c.name, COUNT(*)::bigint AS instances \
         FROM daemon_heartbeats h \
         JOIN clusters c ON c.id = h.cluster_id \
         WHERE h.version = $1 \
         GROUP BY c.id, c.name \
         ORDER BY c.name",
    )
    .bind(&version)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| VersionCluster {
            id: r.id,
            name: r.name,
            instances: r.instances,
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStorePath {
    pub system: String,
    pub store_path: String,
}

/// Fetch all `daemon/{version}/{system}` pins from xzar live and return
/// the (system, store_path) pairs for the requested version.
#[server]
async fn get_daemon_store_paths(
    version: String,
) -> Result<Vec<DaemonStorePath>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let cfg = crate::config::config();
    let xzar = cfg
        .xzar
        .as_ref()
        .ok_or_else(|| ServerFnError::new("xzar not configured".to_string()))?;
    let pins = crate::xzar::fetch_pins(&xzar.url, &xzar.token)
        .await
        .map_err(|e| ServerFnError::new(format!("xzar error: {e}")))?;

    let prefix = format!("daemon/{version}/");
    let mut out: Vec<DaemonStorePath> = Vec::new();
    for pin in &pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }
        let Some(system) = pin.name.strip_prefix(&prefix) else {
            continue;
        };
        if system.contains('/') {
            continue;
        }
        let raw = &pin.roots[0].drv_full;
        let store_path = if raw.starts_with("/nix/store/") {
            raw.clone()
        } else {
            format!("/nix/store/{raw}")
        };
        out.push(DaemonStorePath {
            system: system.to_string(),
            store_path,
        });
    }
    out.sort_by(|a, b| a.system.cmp(&b.system));
    Ok(out)
}

#[component]
pub fn DaemonVersionDetail(version: String) -> Element {
    let v = version.clone();
    let paths = use_server_future(move || {
        let v = v.clone();
        async move { get_daemon_store_paths(v).await }
    })?;
    let v2 = version.clone();
    let clusters = use_server_future(move || {
        let v = v2.clone();
        async move { get_clusters_on_version(v).await }
    })?;
    let v_r = version.clone();
    let rollouts = use_server_future(move || {
        let v = v_r.clone();
        async move { get_rollouts_for_version(v).await }
    })?;
    let v3 = version.clone();
    let pinned = use_server_future(move || {
        let v = v3.clone();
        async move { get_clusters_pinned_to(v).await }
    })?;

    rsx! {
        div { class: "flex items-center justify-between mb-6",
            h2 { class: "text-2xl font-bold", "Daemon {version}" }
            Link {
                to: Route::DaemonVersionList {},
                class: "text-blue-600 dark:text-blue-400 hover:underline text-sm",
                "← All versions"
            }
        }
        {match &*paths.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! {
                        p { class: "text-gray-500 dark:text-gray-400 text-sm",
                            "No store paths found in xzar for this version."
                        }
                    }
                } else {{
                    let search = use_signal(String::new);
                    let limit = use_signal(|| 20usize);
                    let sort = use_signal(|| ("system".to_string(), true));
                    let list_clone = list.clone();
                    let mut filtered: Vec<DaemonStorePath> = {
                        let q = search.read().to_lowercase();
                        if q.is_empty() {
                            list_clone.clone()
                        } else {
                            list_clone.iter()
                                .filter(|p| p.system.to_lowercase().contains(&q) || p.store_path.to_lowercase().contains(&q))
                                .cloned().collect()
                        }
                    };
                    {
                        let (key, asc) = sort.read().clone();
                        filtered.sort_by(|a, b| {
                            let ord = match key.as_str() {
                                "store_path" => a.store_path.cmp(&b.store_path),
                                _ => a.system.cmp(&b.system),
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
                                        SortableTh { label: "System".to_string(), sort_key: "system".to_string(), sort }
                                        SortableTh { label: "Store Path".to_string(), sort_key: "store_path".to_string(), sort }
                                    }
                                }
                                tbody { class: "bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700",
                                    for p in filtered.into_iter().take(limit_val) {
                                        tr { key: "{p.system}",
                                            td { class: "px-6 py-4 font-mono text-sm", "{p.system}" }
                                            td { class: "px-6 py-4 font-mono text-xs text-gray-600 dark:text-gray-300 break-all",
                                                "{p.store_path}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }}
            }
            Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
            None => rsx! { p { "Loading..." } },
        }}

        h3 { class: "text-lg font-semibold mt-10 mb-3", "Clusters on this version" }
        {match &*clusters.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! {
                        p { class: "text-gray-500 dark:text-gray-400 text-sm",
                            "No daemons reporting this version."
                        }
                    }
                } else {{
                    let search = use_signal(String::new);
                    let limit = use_signal(|| 20usize);
                    let sort = use_signal(|| ("cluster".to_string(), true));
                    let list_clone = list.clone();
                    let mut filtered: Vec<VersionCluster> = {
                        let q = search.read().to_lowercase();
                        if q.is_empty() {
                            list_clone.clone()
                        } else {
                            list_clone.iter()
                                .filter(|c| c.name.to_lowercase().contains(&q))
                                .cloned().collect()
                        }
                    };
                    {
                        let (key, asc) = sort.read().clone();
                        filtered.sort_by(|a, b| {
                            let ord = match key.as_str() {
                                "instances" => a.instances.cmp(&b.instances),
                                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
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
                                        SortableTh { label: "Cluster".to_string(), sort_key: "cluster".to_string(), sort }
                                        SortableTh { label: "Instances".to_string(), sort_key: "instances".to_string(), sort }
                                    }
                                }
                                tbody { class: "bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700",
                                    for c in filtered.into_iter().take(limit_val) {
                                        tr { key: "{c.id}",
                                            td { class: "px-6 py-4 text-sm",
                                                Link {
                                                    to: Route::ClusterDetail { id: c.id.to_string() },
                                                    class: "text-blue-600 dark:text-blue-400 hover:underline",
                                                    "{c.name}"
                                                }
                                            }
                                            td { class: "px-6 py-4 text-sm text-gray-600 dark:text-gray-300", "{c.instances}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }}
            }
            Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
            None => rsx! { p { "Loading..." } },
        }}

        h3 { class: "text-lg font-semibold mt-10 mb-3", "Rollouts targeting this version" }
        {match &*rollouts.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! {
                        p { class: "text-gray-500 dark:text-gray-400 text-sm", "No rollouts target this version." }
                    }
                } else {{
                    let search = use_signal(String::new);
                    let limit = use_signal(|| 20usize);
                    let sort = use_signal(|| ("created".to_string(), false));
                    let list_clone = list.clone();
                    let mut filtered: Vec<VersionRollout> = {
                        let q = search.read().to_lowercase();
                        if q.is_empty() {
                            list_clone.clone()
                        } else {
                            list_clone.iter()
                                .filter(|r| r.id.to_string().to_lowercase().contains(&q) || r.status.to_lowercase().contains(&q))
                                .cloned().collect()
                        }
                    };
                    {
                        let (key, asc) = sort.read().clone();
                        filtered.sort_by(|a, b| {
                            let ord = match key.as_str() {
                                "rollout" => a.id.to_string().cmp(&b.id.to_string()),
                                "status" => a.status.cmp(&b.status),
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
                                        SortableTh { label: "Rollout".to_string(), sort_key: "rollout".to_string(), sort }
                                        SortableTh { label: "Status".to_string(), sort_key: "status".to_string(), sort }
                                        SortableTh { label: "Created".to_string(), sort_key: "created".to_string(), sort }
                                    }
                                }
                                tbody { class: "bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700",
                                    for r in filtered.into_iter().take(limit_val) {
                                        {
                                            let ts = r.created_at.format("%Y-%m-%d %H:%M").to_string();
                                            let short = r.id.to_string()[..8].to_string();
                                            rsx! {
                                                tr { key: "{r.id}",
                                                    td { class: "px-6 py-4 text-sm font-mono",
                                                        Link {
                                                            to: Route::RolloutDetail { id: r.id.to_string() },
                                                            class: "text-blue-600 dark:text-blue-400 hover:underline",
                                                            "{short}"
                                                        }
                                                    }
                                                    td { class: "px-6 py-4 text-sm", "{r.status}" }
                                                    td { class: "px-6 py-4 text-xs text-gray-500 dark:text-gray-400", "{ts}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }}
            }
            Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
            None => rsx! { p { "Loading..." } },
        }}

        h3 { class: "text-lg font-semibold mt-10 mb-3", "Clusters pinned to this version" }
        {match &*pinned.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! {
                        p { class: "text-gray-500 dark:text-gray-400 text-sm",
                            "No clusters pinned to this version."
                        }
                    }
                } else {
                    rsx! {
                        ul { class: "list-disc pl-6 text-sm",
                            for c in list.iter() {
                                li { key: "{c.id}",
                                    Link {
                                        to: Route::ClusterDetail { id: c.id.to_string() },
                                        class: "text-blue-600 dark:text-blue-400 hover:underline",
                                        "{c.name}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
            None => rsx! { p { "Loading..." } },
        }}
    }
}
