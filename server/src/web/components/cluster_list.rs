use dioxus::prelude::*;
use dioxus_tabular::*;

use crate::models::Cluster;
use crate::web::app::Route;
use crate::web::components::table_utils::*;

#[server]
async fn list_clusters() -> Result<Vec<Cluster>, ServerFnError> {
    let pool = crate::server_pool()?;
    let clusters = sqlx::query_as::<_, Cluster>("SELECT * FROM clusters ORDER BY name")
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(clusters)
}

#[component]
pub fn ClusterList() -> Element {
    let clusters = use_server_future(list_clusters)?;

    rsx! {
        div { class: "flex items-center justify-between mb-4",
            h2 { class: "text-2xl font-bold", "Clusters" }
            Link {
                to: Route::ClusterForm {},
                class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                "New Cluster"
            }
        }
        {match &*clusters.read() {
            Some(Ok(list)) => {
                let search = use_signal(String::new);
                let limit = use_signal(|| 20usize);

                let list_clone = list.clone();
                let filtered = use_memo(move || {
                    let q = search.read().to_lowercase();
                    if q.is_empty() {
                        list_clone.clone()
                    } else {
                        list_clone.iter().filter(|c| c.matches_search(&q)).cloned().collect()
                    }
                });

                let total = list.len();
                let data = use_tabular(
                    (LinkColumn { header: "Name" }, VersionColumn, NixpkgsCommitColumn, CreatedAtColumn),
                    filtered.into(),
                );
                let all_rows: Vec<_> = data.rows().collect();
                let filtered_count = all_rows.len();
                let limit_val = *limit.read();
                let shown = filtered_count.min(limit_val);

                rsx! {
                    TableToolbar { search, limit, total, filtered: filtered_count, shown }
                    div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 overflow-hidden",
                        table { class: "min-w-full divide-y divide-gray-200 dark:divide-gray-700",
                            thead { class: "bg-gray-50 dark:bg-gray-700",
                                tr { TableHeaders { data } }
                            }
                            tbody { class: "bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700",
                                for row in all_rows.into_iter().take(limit_val) {
                                    tr { key: "{row.key()}", TableCells { row } }
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
