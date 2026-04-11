use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrgInfo {
    name: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemberEntry {
    user_id: String,
    email: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClusterEntry {
    cluster_id: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserOption {
    id: String,
    email: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClusterOption {
    id: String,
    name: String,
}

#[server]
async fn get_organization(id: String) -> Result<OrgInfo, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        created_at: DateTime<Utc>,
    }

    let row = sqlx::query_as::<_, Row>("SELECT name, created_at FROM organizations WHERE id = $1")
        .bind(uid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(OrgInfo {
        name: row.name,
        created_at: row.created_at,
    })
}

#[server]
async fn get_org_members(org_id: String) -> Result<Vec<MemberEntry>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        user_id: uuid::Uuid,
        email: String,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT u.id AS user_id, u.email, u.name \
         FROM users u \
         JOIN organization_members om ON om.user_id = u.id \
         WHERE om.organization_id = $1 \
         ORDER BY u.email",
    )
    .bind(oid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| MemberEntry {
            user_id: r.user_id.to_string(),
            email: r.email,
            name: r.name,
        })
        .collect())
}

#[server]
async fn get_org_clusters(org_id: String) -> Result<Vec<ClusterEntry>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        cluster_id: uuid::Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT c.id AS cluster_id, c.name \
         FROM clusters c \
         JOIN organization_clusters oc ON oc.cluster_id = c.id \
         WHERE oc.organization_id = $1 \
         ORDER BY c.name",
    )
    .bind(oid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| ClusterEntry {
            cluster_id: r.cluster_id.to_string(),
            name: r.name,
        })
        .collect())
}

#[server]
async fn get_available_users(org_id: String) -> Result<Vec<UserOption>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        email: String,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, email, name FROM users \
         WHERE id NOT IN (SELECT user_id FROM organization_members WHERE organization_id = $1) \
         ORDER BY email",
    )
    .bind(oid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| UserOption {
            id: r.id.to_string(),
            email: r.email,
            name: r.name,
        })
        .collect())
}

#[server]
async fn get_available_clusters(org_id: String) -> Result<Vec<ClusterOption>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name FROM clusters \
         WHERE id NOT IN (SELECT cluster_id FROM organization_clusters WHERE organization_id = $1) \
         ORDER BY name",
    )
    .bind(oid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| ClusterOption {
            id: r.id.to_string(),
            name: r.name,
        })
        .collect())
}

#[server]
async fn add_org_member(org_id: String, user_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO organization_members (organization_id, user_id) VALUES ($1, $2)")
        .bind(oid)
        .bind(uid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_org_member(org_id: String, user_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query(
        "DELETE FROM organization_members WHERE organization_id = $1 AND user_id = $2",
    )
    .bind(oid)
    .bind(uid)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn add_org_cluster(org_id: String, cluster_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let cid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO organization_clusters (organization_id, cluster_id) VALUES ($1, $2)")
        .bind(oid)
        .bind(cid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_org_cluster(org_id: String, cluster_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let cid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query(
        "DELETE FROM organization_clusters WHERE organization_id = $1 AND cluster_id = $2",
    )
    .bind(oid)
    .bind(cid)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn delete_organization(id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM organizations WHERE id = $1")
        .bind(uid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn OrganizationDetail(id: String) -> Element {
    let id_for_org = id.clone();
    let org_future = use_server_future(move || {
        let id = id_for_org.clone();
        async move { get_organization(id).await }
    })?;

    let id_for_members = id.clone();
    let mut members_future = use_server_future(move || {
        let id = id_for_members.clone();
        async move { get_org_members(id).await }
    })?;

    let id_for_clusters = id.clone();
    let mut clusters_future = use_server_future(move || {
        let id = id_for_clusters.clone();
        async move { get_org_clusters(id).await }
    })?;

    let id_for_avail_users = id.clone();
    let mut avail_users_future = use_server_future(move || {
        let id = id_for_avail_users.clone();
        async move { get_available_users(id).await }
    })?;

    let id_for_avail_clusters = id.clone();
    let mut avail_clusters_future = use_server_future(move || {
        let id = id_for_avail_clusters.clone();
        async move { get_available_clusters(id).await }
    })?;

    let mut selected_user = use_signal(|| Option::<String>::None);
    let mut selected_cluster = use_signal(|| Option::<String>::None);
    let mut confirm_delete = use_signal(|| false);
    let nav = navigator();

    match &*org_future.read() {
        Some(Ok(info)) => {
            let members = match &*members_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };
            let clusters = match &*clusters_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };
            let avail_users = match &*avail_users_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };
            let avail_clusters = match &*avail_clusters_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };

            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    div {
                        h2 { class: "text-2xl font-bold", "{info.name}" }
                        p { class: "text-gray-500 dark:text-gray-400 text-sm",
                            "Created "
                            {info.created_at.format("%Y-%m-%d %H:%M").to_string()}
                        }
                    }
                    div { class: "flex gap-2",
                        if *confirm_delete.read() {
                            span { class: "text-sm text-red-600 dark:text-red-400 self-center mr-2", "Are you sure?" }
                            button {
                                class: "bg-red-600 text-white px-3 py-1 rounded text-sm hover:bg-red-700",
                                onclick: {
                                    let oid = id.clone();
                                    move |_| {
                                        let oid = oid.clone();
                                        async move {
                                            let _ = delete_organization(oid).await;
                                            nav.push(Route::OrganizationList {});
                                        }
                                    }
                                },
                                "Confirm Delete"
                            }
                            button {
                                class: "bg-gray-500 text-white px-3 py-1 rounded text-sm hover:bg-gray-600",
                                onclick: move |_| confirm_delete.set(false),
                                "Cancel"
                            }
                        } else {
                            button {
                                class: "bg-red-600 text-white px-3 py-1 rounded text-sm hover:bg-red-700",
                                onclick: move |_| confirm_delete.set(true),
                                "Delete Organization"
                            }
                        }
                    }
                }

                div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                    // Members section
                    div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 p-4",
                        h3 { class: "text-lg font-semibold mb-3", "Members" }

                        div { class: "flex gap-2 mb-4",
                            select {
                                class: "border border-gray-300 dark:border-gray-600 rounded px-2 py-1 flex-1 dark:bg-gray-700 dark:text-white",
                                onchange: move |e| {
                                    let val = e.value();
                                    if val.is_empty() {
                                        selected_user.set(None);
                                    } else {
                                        selected_user.set(Some(val));
                                    }
                                },
                                option { value: "", "Select user to add..." }
                                for u in &avail_users {
                                    {
                                        let uid = u.id.clone();
                                        let label = format!("{} ({})", u.email, u.name);
                                        rsx! { option { value: "{uid}", "{label}" } }
                                    }
                                }
                            }
                            button {
                                class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700 disabled:opacity-50",
                                disabled: selected_user.read().is_none(),
                                onclick: {
                                    let oid = id.clone();
                                    move |_| {
                                        let oid = oid.clone();
                                        let uid = selected_user.read().clone();
                                        async move {
                                            if let Some(uid) = uid {
                                                let _ = add_org_member(oid, uid).await;
                                                selected_user.set(None);
                                                members_future.restart();
                                                avail_users_future.restart();
                                            }
                                        }
                                    }
                                },
                                "Add"
                            }
                        }

                        if members.is_empty() {
                            p { class: "text-gray-500 dark:text-gray-400 text-sm", "No members yet." }
                        } else {
                            div { class: "divide-y divide-gray-200 dark:divide-gray-700",
                                for m in &members {
                                    {
                                        let uid = m.user_id.clone();
                                        let oid = id.clone();
                                        rsx! {
                                            div { class: "flex justify-between items-center py-2",
                                                div {
                                                    span { class: "text-sm font-medium", "{m.email}" }
                                                    span { class: "text-sm text-gray-500 dark:text-gray-400 ml-2", "({m.name})" }
                                                }
                                                button {
                                                    class: "text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 text-sm",
                                                    onclick: {
                                                        let uid = uid.clone();
                                                        let oid = oid.clone();
                                                        move |_| {
                                                            let uid = uid.clone();
                                                            let oid = oid.clone();
                                                            async move {
                                                                let _ = remove_org_member(oid, uid).await;
                                                                members_future.restart();
                                                                avail_users_future.restart();
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

                    // Clusters section
                    div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 p-4",
                        h3 { class: "text-lg font-semibold mb-3", "Clusters" }

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
                                for c in &avail_clusters {
                                    {
                                        let cid = c.id.clone();
                                        let cname = c.name.clone();
                                        rsx! { option { value: "{cid}", "{cname}" } }
                                    }
                                }
                            }
                            button {
                                class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700 disabled:opacity-50",
                                disabled: selected_cluster.read().is_none(),
                                onclick: {
                                    let oid = id.clone();
                                    move |_| {
                                        let oid = oid.clone();
                                        let cid = selected_cluster.read().clone();
                                        async move {
                                            if let Some(cid) = cid {
                                                let _ = add_org_cluster(oid, cid).await;
                                                selected_cluster.set(None);
                                                clusters_future.restart();
                                                avail_clusters_future.restart();
                                            }
                                        }
                                    }
                                },
                                "Add"
                            }
                        }

                        if clusters.is_empty() {
                            p { class: "text-gray-500 dark:text-gray-400 text-sm", "No clusters yet." }
                        } else {
                            div { class: "divide-y divide-gray-200 dark:divide-gray-700",
                                for c in &clusters {
                                    {
                                        let cid = c.cluster_id.clone();
                                        let oid = id.clone();
                                        rsx! {
                                            div { class: "flex justify-between items-center py-2",
                                                span { class: "text-sm font-medium", "{c.name}" }
                                                button {
                                                    class: "text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 text-sm",
                                                    onclick: {
                                                        let cid = cid.clone();
                                                        let oid = oid.clone();
                                                        move |_| {
                                                            let cid = cid.clone();
                                                            let oid = oid.clone();
                                                            async move {
                                                                let _ = remove_org_cluster(oid, cid).await;
                                                                clusters_future.restart();
                                                                avail_clusters_future.restart();
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
        Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" } },
        None => rsx! { p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." } },
    }
}
