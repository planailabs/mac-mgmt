use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
use crate::web::components::table_utils::{Searchable, SortState, SortableTh, TableToolbar};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[server]
async fn get_cluster_nixpkgs_counts(shas: Vec<String>) -> Result<std::collections::HashMap<String, u64>, ServerFnError> {
    let set: std::collections::HashSet<String> = shas.into_iter().collect();
    Ok(super::commit_count::nixpkgs_commit_counts(&set).await)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ClusterRow {
    id: String,
    name: String,
    org_names: Vec<String>,
    pinned_version: Option<String>,
    nixpkgs_commit: Option<String>,
    created_at: DateTime<Utc>,
}

impl Searchable for ClusterRow {
    fn matches_search(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(query)
            || self
                .org_names
                .iter()
                .any(|o| o.to_lowercase().contains(query))
    }
}

#[server]
async fn list_clusters() -> Result<Vec<ClusterRow>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        name: String,
        org_names: Vec<String>,
        pinned_version: Option<String>,
        nixpkgs_commit: Option<String>,
        created_at: DateTime<Utc>,
    }

    let rows = if let Some(ids) = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        sqlx::query_as::<_, Row>(
            "SELECT c.id, c.name, \
             COALESCE(array_agg(DISTINCT o.name) FILTER (WHERE o.name IS NOT NULL), '{}') AS org_names, \
             c.pinned_version, c.nixpkgs_commit, c.created_at \
             FROM clusters c \
             LEFT JOIN organization_clusters oc ON oc.cluster_id = c.id \
             LEFT JOIN organizations o ON o.id = oc.organization_id \
             WHERE c.id = ANY($1) \
             GROUP BY c.id \
             ORDER BY c.name",
        )
        .bind(&ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, Row>(
            "SELECT c.id, c.name, \
             COALESCE(array_agg(DISTINCT o.name) FILTER (WHERE o.name IS NOT NULL), '{}') AS org_names, \
             c.pinned_version, c.nixpkgs_commit, c.created_at \
             FROM clusters c \
             LEFT JOIN organization_clusters oc ON oc.cluster_id = c.id \
             LEFT JOIN organizations o ON o.id = oc.organization_id \
             GROUP BY c.id \
             ORDER BY c.name",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows
        .into_iter()
        .map(|r| ClusterRow {
            id: r.id.to_string(),
            name: r.name,
            org_names: r.org_names,
            pinned_version: r.pinned_version,
            nixpkgs_commit: r.nixpkgs_commit,
            created_at: r.created_at,
        })
        .collect())
}

#[server]
async fn is_current_user_admin() -> Result<bool, ServerFnError> {
    match current_user().await {
        Ok(user) => Ok(user.is_admin),
        Err(_) => Ok(true), // If no OIDC, treat as admin
    }
}

#[component]
pub fn ClusterList() -> Element {
    let clusters = use_server_future(list_clusters)?;
    let admin_check = use_server_future(is_current_user_admin)?;
    let is_admin = matches!(&*admin_check.read(), Some(Ok(true)));

    rsx! {
        div { class: "flex items-center justify-between mb-4",
            h2 { class: "text-2xl font-bold", "Clusters" }
            if is_admin {
                Link {
                    to: Route::ClusterForm {},
                    class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                    "New Cluster"
                }
            }
        }
        {match &*clusters.read() {
            Some(Ok(list)) => {
                let search = use_signal(String::new);
                let limit = use_signal(|| 20usize);
                let sort = use_signal::<SortState>(|| ("name".to_string(), true));

                let list_for_counts = list.clone();
                let nix_counts = use_resource(move || {
                    let list = list_for_counts.clone();
                    async move {
                        let shas: Vec<String> = list
                            .iter()
                            .filter_map(|c| c.nixpkgs_commit.clone())
                            .collect::<std::collections::HashSet<_>>()
                            .into_iter()
                            .collect();
                        if shas.is_empty() {
                            return std::collections::HashMap::new();
                        }
                        get_cluster_nixpkgs_counts(shas).await.unwrap_or_default()
                    }
                });
                let counts = nix_counts.read();

                let list_clone = list.clone();
                let filtered = use_memo(move || {
                    let q = search.read().to_lowercase();
                    let mut items: Vec<ClusterRow> = if q.is_empty() {
                        list_clone.clone()
                    } else {
                        list_clone.iter().filter(|c| c.matches_search(&q)).cloned().collect()
                    };
                    let (key, asc) = sort.read().clone();
                    items.sort_by(|a, b| {
                        let ord = match key.as_str() {
                            "organization" => a.org_names.join(", ").to_lowercase().cmp(&b.org_names.join(", ").to_lowercase()),
                            "version" => a.pinned_version.cmp(&b.pinned_version),
                            "nixpkgs" => a.nixpkgs_commit.cmp(&b.nixpkgs_commit),
                            "created" => a.created_at.cmp(&b.created_at),
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
                    TableToolbar { search, limit, total, filtered: filtered_count, shown }
                    div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 overflow-hidden",
                        table { class: "min-w-full divide-y divide-gray-200 dark:divide-gray-700",
                            thead { class: "bg-gray-50 dark:bg-gray-700",
                                tr {
                                    SortableTh { label: "Organization".to_string(), sort_key: "organization".to_string(), sort }
                                    SortableTh { label: "Name".to_string(), sort_key: "name".to_string(), sort }
                                    SortableTh { label: "Version".to_string(), sort_key: "version".to_string(), sort }
                                    SortableTh { label: "Nixpkgs".to_string(), sort_key: "nixpkgs".to_string(), sort }
                                    SortableTh { label: "Created".to_string(), sort_key: "created".to_string(), sort }
                                }
                            }
                            tbody { class: "bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700",
                                for cluster in filtered.read().iter().take(limit_val) {
                                    tr { key: "{cluster.id}",
                                        td { class: "px-6 py-4 text-sm text-gray-600 dark:text-gray-300",
                                            if cluster.org_names.is_empty() {
                                                span { class: "text-gray-400 dark:text-gray-500 italic", "-" }
                                            } else {
                                                {cluster.org_names.join(", ")}
                                            }
                                        }
                                        td { class: "px-6 py-4 dark:text-gray-200",
                                            Link {
                                                to: Route::ClusterDetail { id: cluster.id.clone() },
                                                class: "text-blue-600 dark:text-blue-400 hover:underline",
                                                "{cluster.name}"
                                            }
                                        }
                                        td { class: "px-6 py-4",
                                            if let Some(ver) = &cluster.pinned_version {
                                                span { class: "font-mono text-sm text-gray-700 dark:text-gray-200", "v{ver}" }
                                            } else {
                                                span { class: "text-gray-400 dark:text-gray-500 text-sm", "-" }
                                            }
                                        }
                                        td { class: "px-6 py-4",
                                            if let Some(commit) = &cluster.nixpkgs_commit {
                                                {
                                                    let short: String = commit.chars().take(12).collect();
                                                    let url = format!("https://git.plan.ai/plan-ai/nixpkgs/-/commit/{commit}");
                                                    let count_label = counts.as_ref()
                                                        .and_then(|m| m.get(commit))
                                                        .map(|n| format!(" #{n}"))
                                                        .unwrap_or_default();
                                                    rsx! {
                                                        a {
                                                            class: "text-xs font-mono text-gray-500 dark:text-gray-400 hover:text-blue-600 dark:hover:text-blue-400",
                                                            href: "{url}",
                                                            target: "_blank",
                                                            title: "{commit}",
                                                            "{short}{count_label}"
                                                        }
                                                    }
                                                }
                                            } else {
                                                span { class: "text-gray-400 dark:text-gray-500 text-sm", "-" }
                                            }
                                        }
                                        td { class: "px-6 py-4 text-gray-500 dark:text-gray-400",
                                            {cluster.created_at.format("%Y-%m-%d %H:%M").to_string()}
                                        }
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
