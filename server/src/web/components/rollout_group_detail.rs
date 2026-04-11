use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::components::table_utils::{SortableTh, TableToolbar};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GroupInfo {
    id: Uuid,
    name: String,
    description: String,
    members: Vec<MemberEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemberEntry {
    member_id: Uuid,
    cluster_id: Uuid,
    cluster_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClusterOption {
    id: Uuid,
    name: String,
}

#[server]
async fn get_group_detail(id: String) -> Result<GroupInfo, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let gid: Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if gid == Uuid::nil() {
        return Err(ServerFnError::new(
            "the all-clusters group is implicit and has no detail page",
        ));
    }

    #[derive(sqlx::FromRow)]
    struct GRow {
        id: Uuid,
        name: String,
        description: String,
    }

    let group = sqlx::query_as::<_, GRow>(
        "SELECT id, name, description FROM rollout_groups WHERE id = $1",
    )
    .bind(gid)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct MRow {
        member_id: Uuid,
        cluster_id: Uuid,
        cluster_name: String,
    }

    let members = sqlx::query_as::<_, MRow>(
        "SELECT rgm.id AS member_id, rgm.cluster_id, c.name AS cluster_name \
         FROM rollout_group_members rgm \
         JOIN clusters c ON c.id = rgm.cluster_id \
         WHERE rgm.group_id = $1 ORDER BY c.name",
    )
    .bind(gid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(GroupInfo {
        id: group.id,
        name: group.name,
        description: group.description,
        members: members
            .into_iter()
            .map(|m| MemberEntry {
                member_id: m.member_id,
                cluster_id: m.cluster_id,
                cluster_name: m.cluster_name,
            })
            .collect(),
    })
}

#[server]
async fn get_available_clusters(group_id: String) -> Result<Vec<ClusterOption>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let gid: Uuid = group_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name FROM clusters \
         WHERE id NOT IN (SELECT cluster_id FROM rollout_group_members WHERE group_id = $1) \
         ORDER BY name",
    )
    .bind(gid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| ClusterOption {
            id: r.id,
            name: r.name,
        })
        .collect())
}

#[server]
async fn add_member(group_id: String, cluster_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let gid: Uuid = group_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let cid: Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO rollout_group_members (group_id, cluster_id) VALUES ($1, $2)")
        .bind(gid)
        .bind(cid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn add_all_clusters(group_id: String) -> Result<u64, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let gid: Uuid = group_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let result = sqlx::query(
        "INSERT INTO rollout_group_members (group_id, cluster_id) \
         SELECT $1, id FROM clusters \
         WHERE id NOT IN (SELECT cluster_id FROM rollout_group_members WHERE group_id = $1) \
         ON CONFLICT DO NOTHING",
    )
    .bind(gid)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(result.rows_affected())
}

#[server]
async fn remove_member(member_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let mid: Uuid = member_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM rollout_group_members WHERE id = $1")
        .bind(mid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn delete_group(id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let gid: Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if gid.is_nil() {
        return Err(ServerFnError::new("the All Clusters group cannot be deleted"));
    }
    sqlx::query("DELETE FROM rollout_groups WHERE id = $1")
        .bind(gid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn RolloutGroupDetail(id: String) -> Element {
    let id_clone = id.clone();
    let mut detail = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_group_detail(id).await }
    })?;

    let id_for_clusters = id.clone();
    let mut available = use_server_future(move || {
        let id = id_for_clusters.clone();
        async move { get_available_clusters(id).await }
    })?;

    let mut selected_cluster = use_signal(|| Option::<String>::None);
    let nav = navigator();

    match &*detail.read() {
        Some(Ok(info)) => {
            let gid = info.id.to_string();

            let clusters = match &*available.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };

            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    div {
                        h2 { class: "text-2xl font-bold", "{info.name}" }
                        p { class: "text-gray-500 dark:text-gray-400 text-sm", "{info.description}" }
                    }
                    button {
                        class: "bg-red-600 text-white px-3 py-1 rounded text-sm hover:bg-red-700",
                        onclick: {
                            let gid = gid.clone();
                            move |_| {
                                let gid = gid.clone();
                                async move {
                                    let _ = delete_group(gid).await;
                                    nav.push(crate::web::app::Route::RolloutGroupList {});
                                }
                            }
                        },
                        "Delete Group"
                    }
                }

                h3 { class: "text-lg font-semibold mb-3", "Members" }

                div { class: "flex gap-2 mb-4",
                    select {
                        class: "border border-gray-300 dark:border-gray-600 rounded px-2 py-1 flex-1 dark:bg-gray-700 dark:text-white",
                        onchange: move |e| {
                            let val = e.value();
                            if val.is_empty() {
                                selected_cluster.set(None);
                            } else {
                                selected_cluster.set(Some(val));
                            }
                        },
                        option { value: "", "Select cluster to add..." }
                        for c in &clusters {
                            {
                                let cid = c.id.to_string();
                                let cname = c.name.clone();
                                rsx! { option { value: "{cid}", "{cname}" } }
                            }
                        }
                    }
                    button {
                        class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700 disabled:opacity-50",
                        disabled: selected_cluster.read().is_none(),
                        onclick: {
                            let gid = gid.clone();
                            move |_| {
                                let gid = gid.clone();
                                let cid = selected_cluster.read().clone();
                                async move {
                                    if let Some(cid) = cid {
                                        let _ = add_member(gid, cid).await;
                                        selected_cluster.set(None);
                                        detail.restart();
                                        available.restart();
                                    }
                                }
                            }
                        },
                        "Add"
                    }
                    if !clusters.is_empty() {
                        button {
                            class: "bg-gray-600 text-white px-3 py-1 rounded text-sm hover:bg-gray-700",
                            onclick: {
                                let gid = gid.clone();
                                move |_| {
                                    let gid = gid.clone();
                                    async move {
                                        let _ = add_all_clusters(gid).await;
                                        selected_cluster.set(None);
                                        detail.restart();
                                        available.restart();
                                    }
                                }
                            },
                            "Add All Clusters"
                        }
                    }
                }

                if info.members.is_empty() {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", "No members yet." }
                } else {{
                    let search = use_signal(String::new);
                    let limit = use_signal(|| 20usize);
                    let sort = use_signal(|| ("cluster".to_string(), true));

                    let mut filtered: Vec<MemberEntry> = {
                        let q = search.read().to_lowercase();
                        if q.is_empty() {
                            info.members.clone()
                        } else {
                            info.members.iter()
                                .filter(|m| m.cluster_name.to_lowercase().contains(&q))
                                .cloned().collect()
                        }
                    };
                    {
                        let (_key, asc) = sort.read().clone();
                        filtered.sort_by(|a, b| {
                            let ord = a.cluster_name.to_lowercase().cmp(&b.cluster_name.to_lowercase());
                            if asc { ord } else { ord.reverse() }
                        });
                    }
                    let total = info.members.len();
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
                                        th { class: "px-6 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "" }
                                    }
                                }
                                tbody { class: "bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700",
                                    for m in filtered.into_iter().take(limit_val) {
                                    {
                                        let mid = m.member_id.to_string();
                                        rsx! {
                                            tr {
                                                td { class: "px-6 py-4 text-sm", "{m.cluster_name}" }
                                                td { class: "px-6 py-4 text-right",
                                                    button {
                                                        class: "text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 text-sm",
                                                        onclick: {
                                                            let mid = mid.clone();
                                                            move |_| {
                                                                let mid = mid.clone();
                                                                async move {
                                                                    let _ = remove_member(mid).await;
                                                                    detail.restart();
                                                                    available.restart();
                                                                }
                                                            }
                                                        },
                                                        "Remove"
                                                    }
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
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" } },
        None => rsx! { p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." } },
    }
}
