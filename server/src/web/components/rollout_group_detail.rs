use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    customer_id: Uuid,
    customer_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CustomerOption {
    id: Uuid,
    name: String,
}

#[server]
async fn get_group_detail(id: String) -> Result<GroupInfo, ServerFnError> {
    let pool = crate::server_pool()?;
    let gid: Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

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
        customer_id: Uuid,
        customer_name: String,
    }

    let members = sqlx::query_as::<_, MRow>(
        "SELECT rgm.id AS member_id, rgm.customer_id, c.name AS customer_name \
         FROM rollout_group_members rgm \
         JOIN customers c ON c.id = rgm.customer_id \
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
                customer_id: m.customer_id,
                customer_name: m.customer_name,
            })
            .collect(),
    })
}

#[server]
async fn get_available_customers(group_id: String) -> Result<Vec<CustomerOption>, ServerFnError> {
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
        "SELECT id, name FROM customers \
         WHERE id NOT IN (SELECT customer_id FROM rollout_group_members WHERE group_id = $1) \
         ORDER BY name",
    )
    .bind(gid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| CustomerOption {
            id: r.id,
            name: r.name,
        })
        .collect())
}

#[server]
async fn add_member(group_id: String, customer_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let gid: Uuid = group_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let cid: Uuid = customer_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO rollout_group_members (group_id, customer_id) VALUES ($1, $2)")
        .bind(gid)
        .bind(cid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn add_all_customers(group_id: String) -> Result<u64, ServerFnError> {
    let pool = crate::server_pool()?;
    let gid: Uuid = group_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let result = sqlx::query(
        "INSERT INTO rollout_group_members (group_id, customer_id) \
         SELECT $1, id FROM customers \
         WHERE id NOT IN (SELECT customer_id FROM rollout_group_members WHERE group_id = $1) \
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
    let pool = crate::server_pool()?;
    let gid: Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
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

    let id_for_customers = id.clone();
    let mut available = use_server_future(move || {
        let id = id_for_customers.clone();
        async move { get_available_customers(id).await }
    })?;

    let mut selected_customer = use_signal(|| Option::<String>::None);
    let nav = navigator();

    match &*detail.read() {
        Some(Ok(info)) => {
            let gid = info.id.to_string();

            let customers = match &*available.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };

            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    div {
                        h2 { class: "text-2xl font-bold", "{info.name}" }
                        p { class: "text-gray-500 text-sm", "{info.description}" }
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
                        class: "border rounded px-2 py-1 flex-1",
                        onchange: move |e| {
                            let val = e.value();
                            if val.is_empty() {
                                selected_customer.set(None);
                            } else {
                                selected_customer.set(Some(val));
                            }
                        },
                        option { value: "", "Select customer to add..." }
                        for c in &customers {
                            {
                                let cid = c.id.to_string();
                                let cname = c.name.clone();
                                rsx! { option { value: "{cid}", "{cname}" } }
                            }
                        }
                    }
                    button {
                        class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700 disabled:opacity-50",
                        disabled: selected_customer.read().is_none(),
                        onclick: {
                            let gid = gid.clone();
                            move |_| {
                                let gid = gid.clone();
                                let cid = selected_customer.read().clone();
                                async move {
                                    if let Some(cid) = cid {
                                        let _ = add_member(gid, cid).await;
                                        selected_customer.set(None);
                                        detail.restart();
                                        available.restart();
                                    }
                                }
                            }
                        },
                        "Add"
                    }
                    if !customers.is_empty() {
                        button {
                            class: "bg-gray-600 text-white px-3 py-1 rounded text-sm hover:bg-gray-700",
                            onclick: {
                                let gid = gid.clone();
                                move |_| {
                                    let gid = gid.clone();
                                    async move {
                                        let _ = add_all_customers(gid).await;
                                        selected_customer.set(None);
                                        detail.restart();
                                        available.restart();
                                    }
                                }
                            },
                            "Add All Customers"
                        }
                    }
                }

                if info.members.is_empty() {
                    p { class: "text-gray-500 text-sm", "No members yet." }
                } else {
                    div { class: "bg-white rounded shadow overflow-hidden",
                        table { class: "min-w-full divide-y divide-gray-200",
                            thead { class: "bg-gray-50",
                                tr {
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase", "Customer" }
                                    th { class: "px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase", "" }
                                }
                            }
                            tbody { class: "bg-white divide-y divide-gray-200",
                                for m in &info.members {
                                    {
                                        let mid = m.member_id.to_string();
                                        rsx! {
                                            tr {
                                                td { class: "px-6 py-4 text-sm", "{m.customer_name}" }
                                                td { class: "px-6 py-4 text-right",
                                                    button {
                                                        class: "text-red-600 hover:text-red-700 text-sm",
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
                }
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600 text-sm", "Error: {e}" } },
        None => rsx! { p { class: "text-gray-500 text-sm", "Loading..." } },
    }
}
