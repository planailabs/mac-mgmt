use dioxus::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
use crate::web::components::table_utils::{Searchable, TableToolbar};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct OrgRow {
    id: String,
    name: String,
    member_count: i64,
    cluster_count: i64,
    created_at: DateTime<Utc>,
}

impl Searchable for OrgRow {
    fn matches_search(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(query)
    }
}

#[server]
async fn list_organizations() -> Result<Vec<OrgRow>, ServerFnError> {
    use crate::web::user::current_user;
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        name: String,
        member_count: i64,
        cluster_count: i64,
        created_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT o.id, o.name, \
         (SELECT COUNT(*) FROM organization_members om WHERE om.organization_id = o.id) AS member_count, \
         (SELECT COUNT(*) FROM organization_clusters oc WHERE oc.organization_id = o.id) AS cluster_count, \
         o.created_at \
         FROM organizations o ORDER BY o.name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| OrgRow {
            id: r.id.to_string(),
            name: r.name,
            member_count: r.member_count,
            cluster_count: r.cluster_count,
            created_at: r.created_at,
        })
        .collect())
}

#[component]
pub fn OrganizationList() -> Element {
    let orgs = use_server_future(list_organizations)?;

    rsx! {
        div { class: "flex items-center justify-between mb-4",
            h2 { class: "text-2xl font-bold", "Organizations" }
            Link {
                to: Route::OrganizationForm {},
                class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                "New Organization"
            }
        }
        {match &*orgs.read() {
            Some(Ok(list)) => {
                let search = use_signal(String::new);
                let limit = use_signal(|| 20usize);

                let list_clone = list.clone();
                let filtered = use_memo(move || {
                    let q = search.read().to_lowercase();
                    if q.is_empty() {
                        list_clone.clone()
                    } else {
                        list_clone.iter().filter(|o| o.matches_search(&q)).cloned().collect()
                    }
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
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider", "Name" }
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider", "Members" }
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider", "Clusters" }
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider", "Created" }
                                }
                            }
                            tbody { class: "divide-y divide-gray-200 dark:divide-gray-700",
                                for org in filtered.read().iter().take(limit_val) {
                                    tr { key: "{org.id}",
                                        td { class: "px-6 py-4 text-sm",
                                            Link {
                                                to: Route::OrganizationDetail { id: org.id.clone() },
                                                class: "text-blue-600 dark:text-blue-400 hover:underline font-medium",
                                                "{org.name}"
                                            }
                                        }
                                        td { class: "px-6 py-4 text-sm text-gray-600 dark:text-gray-300", "{org.member_count}" }
                                        td { class: "px-6 py-4 text-sm text-gray-600 dark:text-gray-300", "{org.cluster_count}" }
                                        td { class: "px-6 py-4 text-sm text-gray-500 dark:text-gray-400",
                                            {org.created_at.format("%Y-%m-%d %H:%M").to_string()}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
            None => rsx! { p { "Loading…" } },
        }}
    }
}
