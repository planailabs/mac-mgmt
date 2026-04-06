use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;
use crate::web::components::table_utils::{Searchable, TableToolbar};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GroupEntry {
    id: Uuid,
    name: String,
    description: String,
    member_count: i64,
}

impl Searchable for GroupEntry {
    fn matches_search(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(query)
            || self.description.to_lowercase().contains(query)
    }
}

#[server]
async fn get_rollout_groups() -> Result<Vec<GroupEntry>, ServerFnError> {
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row { id: Uuid, name: String, description: String, member_count: i64 }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT rg.id, rg.name, rg.description, COUNT(rgm.id) AS member_count \
         FROM rollout_groups rg LEFT JOIN rollout_group_members rgm ON rgm.group_id = rg.id \
         GROUP BY rg.id ORDER BY rg.name"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|r| GroupEntry {
        id: r.id, name: r.name, description: r.description, member_count: r.member_count,
    }).collect())
}

#[server]
async fn create_group(name: String, description: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    sqlx::query("INSERT INTO rollout_groups (name, description) VALUES ($1, $2)")
        .bind(&name)
        .bind(&description)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn RolloutGroupList() -> Element {
    let mut groups = use_server_future(move || async move { get_rollout_groups().await })?;
    let mut name = use_signal(String::new);
    let mut desc = use_signal(String::new);

    match &*groups.read() {
        Some(Ok(list)) => {
            rsx! {
                h2 { class: "text-2xl font-bold mb-4", "Rollout Groups" }

                div { class: "mb-6 p-4 bg-white rounded shadow",
                    h3 { class: "text-lg font-semibold mb-2", "Create Group" }
                    div { class: "flex gap-2",
                        input {
                            class: "border rounded px-2 py-1 flex-1",
                            placeholder: "Group name",
                            value: "{name}",
                            oninput: move |e| name.set(e.value()),
                        }
                        input {
                            class: "border rounded px-2 py-1 flex-1",
                            placeholder: "Description",
                            value: "{desc}",
                            oninput: move |e| desc.set(e.value()),
                        }
                        button {
                            class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700",
                            onclick: move |_| {
                                let n = name.read().clone();
                                let d = desc.read().clone();
                                async move {
                                    if !n.trim().is_empty() {
                                        let _ = create_group(n, d).await;
                                        name.set(String::new());
                                        desc.set(String::new());
                                        groups.restart();
                                    }
                                }
                            },
                            "Create"
                        }
                    }
                }

                {
                    let search = use_signal(String::new);
                    let limit = use_signal(|| 20usize);

                    let list_clone = list.clone();
                    let filtered: Vec<&GroupEntry> = {
                        let q = search.read().to_lowercase();
                        if q.is_empty() {
                            list_clone.iter().collect()
                        } else {
                            list_clone.iter().filter(|e| e.matches_search(&q)).collect()
                        }
                    };

                    let total = list.len();
                    let filtered_count = filtered.len();
                    let limit_val = *limit.read();
                    let shown = filtered_count.min(limit_val);

                    rsx! {
                        TableToolbar { search, limit, total, filtered: filtered_count, shown }
                        div { class: "bg-white rounded shadow overflow-hidden",
                            table { class: "min-w-full divide-y divide-gray-200",
                                thead { class: "bg-gray-50",
                                    tr {
                                        th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase", "Name" }
                                        th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase", "Description" }
                                        th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase", "Members" }
                                    }
                                }
                                tbody { class: "bg-white divide-y divide-gray-200",
                                    for g in filtered.into_iter().take(limit_val) {
                                        {
                                            let gid = g.id.to_string();
                                            rsx! {
                                                tr {
                                                    td { class: "px-6 py-4 text-sm font-medium",
                                                        Link { to: Route::RolloutGroupDetail { id: gid },
                                                            class: "text-blue-600 hover:underline",
                                                            "{g.name}"
                                                        }
                                                    }
                                                    td { class: "px-6 py-4 text-sm text-gray-500", "{g.description}" }
                                                    td { class: "px-6 py-4 text-sm", "{g.member_count}" }
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
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600 text-sm", "Error: {e}" } },
        None => rsx! { p { class: "text-gray-500 text-sm", "Loading..." } },
    }
}
