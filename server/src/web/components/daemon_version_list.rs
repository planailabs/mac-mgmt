use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonVersionRow {
    pub version: String,
    pub created_at: DateTime<Utc>,
}

#[server]
async fn list_daemon_versions() -> Result<Vec<DaemonVersionRow>, ServerFnError> {
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        version: String,
        created_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT version, created_at FROM daemon_versions ORDER BY version DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| DaemonVersionRow {
            version: r.version,
            created_at: r.created_at,
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSyncResult {
    pub created: u32,
    pub removed: u32,
}

/// Fetch xzar pins, parse `daemon/{version}/{system}` and upsert the
/// distinct versions into `daemon_versions`. Store paths are resolved
/// live from xzar on each /api/update call, so they are not stored.
#[server]
async fn sync_daemon_versions_from_xzar() -> Result<DaemonSyncResult, ServerFnError> {
    let pool = crate::server_pool()?;
    let cfg = crate::config::config();

    let xzar = cfg
        .xzar
        .as_ref()
        .ok_or_else(|| ServerFnError::new("xzar not configured".to_string()))?;
    let pins = crate::xzar::fetch_pins(&xzar.url, &xzar.token)
        .await
        .map_err(|e| ServerFnError::new(format!("xzar error: {e}")))?;

    let mut created: u32 = 0;
    let mut valid: std::collections::HashSet<String> = std::collections::HashSet::new();

    for pin in &pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }
        let parts: Vec<&str> = pin.name.splitn(3, '/').collect();
        if parts.len() != 3 || parts[0] != "daemon" {
            continue;
        }
        let version = parts[1].to_string();
        if !valid.insert(version.clone()) {
            continue;
        }

        let inserted = sqlx::query_scalar::<_, bool>(
            "INSERT INTO daemon_versions (version) VALUES ($1) \
             ON CONFLICT (version) DO NOTHING \
             RETURNING true",
        )
        .bind(&version)
        .fetch_optional(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        if inserted.is_some() {
            created += 1;
        }
    }

    let valid_vec: Vec<String> = valid.into_iter().collect();
    let removed = sqlx::query_scalar::<_, i64>(
        "WITH deleted AS ( \
             DELETE FROM daemon_versions dv \
             WHERE NOT (dv.version = ANY($1::text[])) \
             RETURNING dv.id \
         ) SELECT count(*) FROM deleted",
    )
    .bind(&valid_vec)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(DaemonSyncResult {
        created,
        removed: removed as u32,
    })
}

#[component]
pub fn DaemonVersionList() -> Element {
    let mut versions = use_server_future(list_daemon_versions)?;
    let mut syncing = use_signal(|| false);
    let mut sync_msg = use_signal(|| None::<String>);
    let mut sync_err = use_signal(|| None::<String>);

    rsx! {
        div { class: "flex items-center justify-between mb-4",
            h2 { class: "text-2xl font-bold", "Daemon Versions" }
            button {
                class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700 disabled:opacity-50",
                disabled: *syncing.read(),
                onclick: move |_| {
                    syncing.set(true);
                    sync_msg.set(None);
                    sync_err.set(None);
                    spawn(async move {
                        match sync_daemon_versions_from_xzar().await {
                            Ok(result) => {
                                sync_msg.set(Some(format!(
                                    "Synced: +{} new, -{} removed",
                                    result.created, result.removed,
                                )));
                                versions.restart();
                            }
                            Err(e) => {
                                sync_err.set(Some(e.to_string()));
                            }
                        }
                        syncing.set(false);
                    });
                },
                if *syncing.read() { "Syncing..." } else { "Sync from xzar" }
            }
        }
        if let Some(msg) = &*sync_msg.read() {
            p { class: "text-green-600 text-sm mb-4", "{msg}" }
        }
        if let Some(err) = &*sync_err.read() {
            p { class: "text-red-600 text-sm mb-4", "{err}" }
        }
        {match &*versions.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! {
                        p { class: "text-gray-500 text-sm",
                            "No daemon versions yet. Run xzar.sh to upload binaries, then click Sync."
                        }
                    }
                } else {
                    rsx! {
                        table { class: "min-w-full divide-y divide-gray-200",
                            thead { class: "bg-gray-50",
                                tr {
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Version" }
                                    th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase", "Added" }
                                }
                            }
                            tbody { class: "bg-white divide-y divide-gray-200",
                                for v in list.iter() {
                                    {
                                        let ts = v.created_at.format("%Y-%m-%d %H:%M").to_string();
                                        rsx! {
                                            tr { key: "{v.version}",
                                                td { class: "px-4 py-2 font-mono text-sm",
                                                    Link {
                                                        to: Route::DaemonVersionDetail { version: v.version.clone() },
                                                        class: "text-blue-600 hover:underline",
                                                        "{v.version}"
                                                    }
                                                }
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
    }
}
