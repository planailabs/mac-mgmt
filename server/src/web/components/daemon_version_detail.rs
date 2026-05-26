use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    DataTable, ErrorText, HelpText, PageHeader, SectionHeading, SortState, SortableTh, Td, TdMono,
    TdMuted, Th,
};
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

/// Return the configured external API base URL.
#[server]
async fn get_api_base_url() -> Result<String, ServerFnError> {
    let cfg = crate::config::config();
    Ok(cfg.api.external_url.clone())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionCluster {
    pub id: Uuid,
    pub name: String,
    pub instances: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionRollout {
    pub id: Uuid,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[server]
async fn get_rollouts_for_version(version: String) -> Result<Vec<VersionRollout>, ServerFnError> {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PinnedCluster {
    pub id: Uuid,
    pub name: String,
}

#[server]
async fn get_clusters_pinned_to(version: String) -> Result<Vec<PinnedCluster>, ServerFnError> {
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
        .map(|r| PinnedCluster {
            id: r.id,
            name: r.name,
        })
        .collect())
}

#[server]
async fn get_clusters_on_version(version: String) -> Result<Vec<VersionCluster>, ServerFnError> {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonStorePath {
    pub system: String,
    pub store_path: String,
}

/// Fetch all `daemon/{version}/{system}` pins from xzar live and return
/// the (system, store_path) pairs for the requested version.
#[server]
async fn get_daemon_store_paths(version: String) -> Result<Vec<DaemonStorePath>, ServerFnError> {
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
    use_topbar(t!("nav-daemon-versions"), None);
    let api_base = use_server_future(get_api_base_url)?;
    let api_base_url: String = match &*api_base.read() {
        Some(Ok(url)) => url.trim_end_matches('/').to_string(),
        _ => String::new(),
    };

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
            PageHeader { class: "mb-0", {t!("daemon-version-detail-title", version: version.clone())} }
            Link { to: Route::DaemonVersionList {}, class: "link text-sm",
                {t!("daemon-version-detail-all")}
            }
        }
        {match &*paths.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! { HelpText { {t!("daemon-version-detail-no-paths")} } }
                } else {
                    rsx! { PathsTable { list: list.clone(), api_base_url: api_base_url.clone(), version: version.clone() } }
                }
            }
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}

        SectionHeading { class: "mt-10", {t!("daemon-version-detail-clusters")} }
        {match &*clusters.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! { HelpText { {t!("daemon-version-detail-no-daemons")} } }
                } else {
                    rsx! { ClustersTable { list: list.clone() } }
                }
            }
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}

        SectionHeading { class: "mt-10", {t!("daemon-version-detail-rollouts")} }
        {match &*rollouts.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! { HelpText { {t!("daemon-version-detail-no-rollouts")} } }
                } else {
                    rsx! { RolloutsTable { list: list.clone() } }
                }
            }
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}

        SectionHeading { class: "mt-10", {t!("daemon-version-detail-pinned")} }
        {match &*pinned.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! { HelpText { {t!("daemon-version-detail-no-pinned")} } }
                } else {
                    rsx! {
                        ul { class: "list-disc pl-6 text-sm",
                            for c in list.iter() {
                                li { key: "{c.id}",
                                    Link { to: Route::ClusterDetail { id: c.id.to_string() }, class: "link",
                                        "{c.name}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}

#[component]
fn PathsTable(list: Vec<DaemonStorePath>, api_base_url: String, version: String) -> Element {
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);
    let sort = use_signal::<SortState>(|| ("system".to_string(), true));

    let list_clone = list.clone();
    let filtered = use_memo(move || {
        let q = search.read().to_lowercase();
        let mut items: Vec<DaemonStorePath> = if q.is_empty() {
            list_clone.clone()
        } else {
            list_clone
                .iter()
                .filter(|p| {
                    p.system.to_lowercase().contains(&q) || p.store_path.to_lowercase().contains(&q)
                })
                .cloned()
                .collect()
        };
        let (key, asc) = sort.read().clone();
        items.sort_by(|a, b| {
            let ord = match key.as_str() {
                "store_path" => a.store_path.cmp(&b.store_path),
                _ => a.system.cmp(&b.system),
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
                SortableTh { label: t!("daemon-version-detail-col-system"), sort_key: "system".to_string(), sort }
                SortableTh { label: t!("daemon-version-detail-col-store-path"), sort_key: "store_path".to_string(), sort }
                Th { "" }
            },
            body: rsx! {
                for p in filtered.read().iter().take(limit_val) {
                    {
                        let dl_url = format!("{}/api/daemon-download/{}/{}", api_base_url, version, p.system);
                        rsx! {
                            tr { key: "{p.system}",
                                TdMono { "{p.system}" }
                                td { class: "td-muted font-mono text-xs break-all", "{p.store_path}" }
                                Td {
                                    a { href: "{dl_url}", class: "btn btn-xs btn-primary",
                                        {t!("download")}
                                    }
                                }
                            }
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn ClustersTable(list: Vec<VersionCluster>) -> Element {
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);
    let sort = use_signal::<SortState>(|| ("cluster".to_string(), true));

    let list_clone = list.clone();
    let filtered = use_memo(move || {
        let q = search.read().to_lowercase();
        let mut items: Vec<VersionCluster> = if q.is_empty() {
            list_clone.clone()
        } else {
            list_clone
                .iter()
                .filter(|c| c.name.to_lowercase().contains(&q))
                .cloned()
                .collect()
        };
        let (key, asc) = sort.read().clone();
        items.sort_by(|a, b| {
            let ord = match key.as_str() {
                "instances" => a.instances.cmp(&b.instances),
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
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
                SortableTh { label: t!("daemon-version-detail-col-cluster"), sort_key: "cluster".to_string(), sort }
                SortableTh { label: t!("daemon-version-detail-col-instances"), sort_key: "instances".to_string(), sort }
            },
            body: rsx! {
                for c in filtered.read().iter().take(limit_val) {
                    tr { key: "{c.id}",
                        Td { class: "text-sm",
                            Link { to: Route::ClusterDetail { id: c.id.to_string() }, class: "link",
                                "{c.name}"
                            }
                        }
                        TdMuted { class: "text-sm", "{c.instances}" }
                    }
                }
            },
        }
    }
}

#[component]
fn RolloutsTable(list: Vec<VersionRollout>) -> Element {
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);
    let sort = use_signal::<SortState>(|| ("created".to_string(), false));

    let list_clone = list.clone();
    let filtered = use_memo(move || {
        let q = search.read().to_lowercase();
        let mut items: Vec<VersionRollout> = if q.is_empty() {
            list_clone.clone()
        } else {
            list_clone
                .iter()
                .filter(|r| {
                    r.id.to_string().to_lowercase().contains(&q)
                        || r.status.to_lowercase().contains(&q)
                })
                .cloned()
                .collect()
        };
        let (key, asc) = sort.read().clone();
        items.sort_by(|a, b| {
            let ord = match key.as_str() {
                "rollout" => a.id.to_string().cmp(&b.id.to_string()),
                "status" => a.status.cmp(&b.status),
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
                SortableTh { label: t!("daemon-version-detail-col-rollout"), sort_key: "rollout".to_string(), sort }
                SortableTh { label: t!("status"), sort_key: "status".to_string(), sort }
                SortableTh { label: t!("created"), sort_key: "created".to_string(), sort }
            },
            body: rsx! {
                for r in filtered.read().iter().take(limit_val) {
                    {
                        let ts = r.created_at.format("%Y-%m-%d %H:%M").to_string();
                        let short = r.id.to_string()[..8].to_string();
                        rsx! {
                            tr { key: "{r.id}",
                                TdMono {
                                    Link { to: Route::RolloutDetail { id: r.id.to_string() }, class: "link",
                                        "{short}"
                                    }
                                }
                                Td { class: "text-sm", "{r.status}" }
                                td { class: "td-muted text-xs", "{ts}" }
                            }
                        }
                    }
                }
            },
        }
    }
}
