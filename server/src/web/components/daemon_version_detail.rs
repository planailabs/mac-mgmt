use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCustomer {
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
pub struct PinnedCustomer {
    pub id: Uuid,
    pub name: String,
}

#[server]
async fn get_customers_pinned_to(
    version: String,
) -> Result<Vec<PinnedCustomer>, ServerFnError> {
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name FROM customers WHERE pinned_version = $1 ORDER BY name",
    )
    .bind(&version)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| PinnedCustomer { id: r.id, name: r.name })
        .collect())
}

#[server]
async fn get_customers_on_version(
    version: String,
) -> Result<Vec<VersionCustomer>, ServerFnError> {
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
         JOIN customers c ON c.id = h.customer_id \
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
        .map(|r| VersionCustomer {
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
    let customers = use_server_future(move || {
        let v = v2.clone();
        async move { get_customers_on_version(v).await }
    })?;
    let v_r = version.clone();
    let rollouts = use_server_future(move || {
        let v = v_r.clone();
        async move { get_rollouts_for_version(v).await }
    })?;
    let v3 = version.clone();
    let pinned = use_server_future(move || {
        let v = v3.clone();
        async move { get_customers_pinned_to(v).await }
    })?;

    rsx! {
        div { class: "flex items-center justify-between mb-6",
            h2 { class: "text-2xl font-bold", "Daemon {version}" }
            Link {
                to: Route::DaemonVersionList {},
                class: "text-blue-600 hover:underline text-sm",
                "← All versions"
            }
        }
        {match &*paths.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! {
                        p { class: "text-gray-500 text-sm",
                            "No store paths found in xzar for this version."
                        }
                    }
                } else {
                    rsx! {
                        table { class: "min-w-full divide-y divide-gray-200",
                            thead { class: "bg-gray-50",
                                tr {
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "System" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Store Path" }
                                }
                            }
                            tbody { class: "bg-white divide-y divide-gray-200",
                                for p in list.iter() {
                                    tr { key: "{p.system}",
                                        td { class: "px-4 py-2 font-mono text-sm", "{p.system}" }
                                        td { class: "px-4 py-2 font-mono text-xs text-gray-600 break-all",
                                            "{p.store_path}"
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
        }}

        h3 { class: "text-lg font-semibold mt-10 mb-3", "Customers on this version" }
        {match &*customers.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! {
                        p { class: "text-gray-500 text-sm",
                            "No daemons reporting this version."
                        }
                    }
                } else {
                    rsx! {
                        table { class: "min-w-full divide-y divide-gray-200",
                            thead { class: "bg-gray-50",
                                tr {
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Customer" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Instances" }
                                }
                            }
                            tbody { class: "bg-white divide-y divide-gray-200",
                                for c in list.iter() {
                                    tr { key: "{c.id}",
                                        td { class: "px-4 py-2 text-sm",
                                            Link {
                                                to: Route::CustomerDetail { id: c.id.to_string() },
                                                class: "text-blue-600 hover:underline",
                                                "{c.name}"
                                            }
                                        }
                                        td { class: "px-4 py-2 text-sm text-gray-600", "{c.instances}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
            None => rsx! { p { "Loading..." } },
        }}

        h3 { class: "text-lg font-semibold mt-10 mb-3", "Rollouts targeting this version" }
        {match &*rollouts.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! {
                        p { class: "text-gray-500 text-sm", "No rollouts target this version." }
                    }
                } else {
                    rsx! {
                        table { class: "min-w-full divide-y divide-gray-200",
                            thead { class: "bg-gray-50",
                                tr {
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Rollout" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Status" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Created" }
                                }
                            }
                            tbody { class: "bg-white divide-y divide-gray-200",
                                for r in list.iter() {
                                    {
                                        let ts = r.created_at.format("%Y-%m-%d %H:%M").to_string();
                                        let short = r.id.to_string()[..8].to_string();
                                        rsx! {
                                            tr { key: "{r.id}",
                                                td { class: "px-4 py-2 text-sm font-mono",
                                                    Link {
                                                        to: Route::RolloutDetail { id: r.id.to_string() },
                                                        class: "text-blue-600 hover:underline",
                                                        "{short}"
                                                    }
                                                }
                                                td { class: "px-4 py-2 text-sm", "{r.status}" }
                                                td { class: "px-4 py-2 text-xs text-gray-500", "{ts}" }
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
        }}

        h3 { class: "text-lg font-semibold mt-10 mb-3", "Customers pinned to this version" }
        {match &*pinned.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! {
                        p { class: "text-gray-500 text-sm",
                            "No customers pinned to this version."
                        }
                    }
                } else {
                    rsx! {
                        ul { class: "list-disc pl-6 text-sm",
                            for c in list.iter() {
                                li { key: "{c.id}",
                                    Link {
                                        to: Route::CustomerDetail { id: c.id.to_string() },
                                        class: "text-blue-600 hover:underline",
                                        "{c.name}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
            None => rsx! { p { "Loading..." } },
        }}
    }
}
