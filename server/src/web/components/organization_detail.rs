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
    role: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrgTokenRow {
    id: String,
    label: String,
    kind: String,
    revoked: bool,
    created_at: DateTime<Utc>,
}

/// What the current user can do on this org.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrgPermissions {
    /// mac-mgmt global admin
    is_global_admin: bool,
    /// org-level admin (can manage members, roles, tokens)
    is_org_admin: bool,
}

#[server]
async fn get_organization(id: String) -> Result<OrgInfo, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let oid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    // Global admins or org members can view
    if !user.is_admin && !user.org_ids().contains(&oid) {
        return Err(ServerFnError::new("access denied"));
    }

    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        created_at: DateTime<Utc>,
    }

    let row = sqlx::query_as::<_, Row>("SELECT name, created_at FROM organizations WHERE id = $1")
        .bind(oid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(OrgInfo {
        name: row.name,
        created_at: row.created_at,
    })
}

#[server]
async fn get_org_permissions(org_id: String) -> Result<OrgPermissions, ServerFnError> {
    let user = current_user().await?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    Ok(OrgPermissions {
        is_global_admin: user.is_admin,
        is_org_admin: user.is_org_admin(&oid),
    })
}

#[server]
async fn get_org_members(org_id: String) -> Result<Vec<MemberEntry>, ServerFnError> {
    let user = current_user().await?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    // Global admins or org members can view members
    if !user.is_admin && !user.org_ids().contains(&oid) {
        return Err(ServerFnError::new("access denied"));
    }

    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        user_id: uuid::Uuid,
        email: String,
        name: String,
        role: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT u.id AS user_id, u.email, u.name, om.role \
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
            role: r.role,
        })
        .collect())
}

#[server]
async fn get_org_clusters(org_id: String) -> Result<Vec<ClusterEntry>, ServerFnError> {
    let user = current_user().await?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    // Global admins or org members can view clusters
    if !user.is_admin && !user.org_ids().contains(&oid) {
        return Err(ServerFnError::new("access denied"));
    }

    let pool = crate::server_pool()?;

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
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    // Org admins can add members
    user.require_org_admin(&oid)?;
    let pool = crate::server_pool()?;

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
    user.require_admin()?; // Only global admins can assign clusters to orgs
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
async fn add_org_member(
    org_id: String,
    user_id: String,
    role: String,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    user.require_org_admin(&oid)?;

    if !["admin", "write", "read"].contains(&role.as_str()) {
        return Err(ServerFnError::new("invalid role"));
    }

    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query(
        "INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, $2, $3)",
    )
    .bind(oid)
    .bind(uid)
    .bind(&role)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_org_member(org_id: String, user_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    user.require_org_admin(&oid)?;
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM organization_members WHERE organization_id = $1 AND user_id = $2")
        .bind(oid)
        .bind(uid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn change_member_role(
    org_id: String,
    user_id: String,
    role: String,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    user.require_org_admin(&oid)?;

    if !["admin", "write", "read"].contains(&role.as_str()) {
        return Err(ServerFnError::new("invalid role"));
    }

    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query(
        "UPDATE organization_members SET role = $3 WHERE organization_id = $1 AND user_id = $2",
    )
    .bind(oid)
    .bind(uid)
    .bind(&role)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn add_org_cluster(org_id: String, cluster_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?; // Only global admins
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
    user.require_admin()?; // Only global admins
    let pool = crate::server_pool()?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let cid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM organization_clusters WHERE organization_id = $1 AND cluster_id = $2")
        .bind(oid)
        .bind(cid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn list_org_tokens(org_id: String) -> Result<Vec<OrgTokenRow>, ServerFnError> {
    let user = current_user().await?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    user.require_org_admin(&oid)?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        label: String,
        kind: String,
        revoked: bool,
        created_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, label, kind, revoked, created_at FROM tokens \
         WHERE organization_id = $1 ORDER BY created_at DESC",
    )
    .bind(oid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| OrgTokenRow {
            id: r.id.to_string(),
            label: r.label,
            kind: r.kind,
            revoked: r.revoked,
            created_at: r.created_at,
        })
        .collect())
}

#[server]
async fn create_org_token(org_id: String, label: String) -> Result<String, ServerFnError> {
    use rand::Rng;
    use sha2::Digest;

    let user = current_user().await?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    user.require_org_admin(&oid)?;
    let pool = crate::server_pool()?;

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(sha2::Sha256::digest(raw_token.as_bytes()));

    sqlx::query(
        "INSERT INTO tokens (organization_id, token_hash, label, kind) VALUES ($1, $2, $3, 'setting')",
    )
    .bind(oid)
    .bind(hash)
    .bind(label)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(raw_token)
}

#[server]
async fn revoke_org_token(org_id: String, token_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    user.require_org_admin(&oid)?;
    let pool = crate::server_pool()?;
    let tid: uuid::Uuid = token_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    sqlx::query("UPDATE tokens SET revoked = true WHERE id = $1 AND organization_id = $2")
        .bind(tid)
        .bind(oid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn delete_organization(id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?; // Only global admins
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

    let id_for_perms = id.clone();
    let perms_future = use_server_future(move || {
        let id = id_for_perms.clone();
        async move { get_org_permissions(id).await }
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

    let id_for_tokens = id.clone();
    let mut tokens_future = use_server_future(move || {
        let id = id_for_tokens.clone();
        async move { list_org_tokens(id).await }
    })?;

    let mut selected_user = use_signal(|| Option::<String>::None);
    let mut selected_role = use_signal(|| "read".to_string());
    let mut selected_cluster = use_signal(|| Option::<String>::None);
    let mut confirm_delete = use_signal(|| false);
    let mut token_label = use_signal(|| String::new());
    let mut created_token = use_signal(|| Option::<String>::None);
    let nav = navigator();

    let perms = match &*perms_future.read() {
        Some(Ok(p)) => p.clone(),
        _ => OrgPermissions {
            is_global_admin: false,
            is_org_admin: false,
        },
    };

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
            let tokens = match &*tokens_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };

            let can_manage_members = perms.is_org_admin;
            let can_manage_clusters = perms.is_global_admin;
            let can_manage_tokens = perms.is_org_admin;
            let can_delete = perms.is_global_admin;

            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    div {
                        h2 { class: "text-2xl font-bold", "{info.name}" }
                        p { class: "text-gray-500 dark:text-gray-400 text-sm",
                            "Created "
                            {info.created_at.format("%Y-%m-%d %H:%M").to_string()}
                        }
                    }
                    if can_delete {
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
                }

                div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                    // Members section
                    div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 p-4",
                        h3 { class: "text-lg font-semibold mb-3", "Members" }

                        if can_manage_members {
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
                                select {
                                    class: "border border-gray-300 dark:border-gray-600 rounded px-2 py-1 w-24 dark:bg-gray-700 dark:text-white",
                                    value: "{selected_role}",
                                    onchange: move |e| selected_role.set(e.value()),
                                    option { value: "read", "Read" }
                                    option { value: "write", "Write" }
                                    option { value: "admin", "Admin" }
                                }
                                button {
                                    class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700 disabled:opacity-50",
                                    disabled: selected_user.read().is_none(),
                                    onclick: {
                                        let oid = id.clone();
                                        move |_| {
                                            let oid = oid.clone();
                                            let uid = selected_user.read().clone();
                                            let role = selected_role.read().clone();
                                            async move {
                                                if let Some(uid) = uid {
                                                    let _ = add_org_member(oid, uid, role).await;
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
                        }

                        if members.is_empty() {
                            p { class: "text-gray-500 dark:text-gray-400 text-sm", "No members yet." }
                        } else {
                            div { class: "divide-y divide-gray-200 dark:divide-gray-700",
                                for m in &members {
                                    {
                                        let uid = m.user_id.clone();
                                        let oid = id.clone();
                                        let current_role = m.role.clone();
                                        let badge_class = match m.role.as_str() {
                                            "admin" => "bg-purple-100 dark:bg-purple-900 text-purple-700 dark:text-purple-300",
                                            "write" => "bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300",
                                            _ => "bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300",
                                        };
                                        rsx! {
                                            div { class: "flex justify-between items-center py-2",
                                                div { class: "flex items-center gap-2",
                                                    span { class: "text-sm font-medium", "{m.email}" }
                                                    if !m.name.is_empty() {
                                                        span { class: "text-sm text-gray-500 dark:text-gray-400", "({m.name})" }
                                                    }
                                                    if can_manage_members {
                                                        select {
                                                            class: "text-xs border border-gray-300 dark:border-gray-600 rounded px-1 py-0.5 dark:bg-gray-700 dark:text-white",
                                                            value: "{current_role}",
                                                            onchange: {
                                                                let uid = uid.clone();
                                                                let oid = oid.clone();
                                                                move |e: Event<FormData>| {
                                                                    let uid = uid.clone();
                                                                    let oid = oid.clone();
                                                                    let new_role = e.value();
                                                                    async move {
                                                                        let _ = change_member_role(oid, uid, new_role).await;
                                                                        members_future.restart();
                                                                    }
                                                                }
                                                            },
                                                            option { value: "read", "Read" }
                                                            option { value: "write", "Write" }
                                                            option { value: "admin", "Admin" }
                                                        }
                                                    } else {
                                                        span { class: "text-xs px-1.5 py-0.5 rounded {badge_class}", "{current_role}" }
                                                    }
                                                }
                                                if can_manage_members {
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
                    }

                    // Clusters section
                    div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 p-4",
                        h3 { class: "text-lg font-semibold mb-3", "Clusters" }

                        if can_manage_clusters {
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
                                                Link {
                                                    to: Route::ClusterDetail { id: cid.clone() },
                                                    class: "text-blue-600 dark:text-blue-400 hover:underline text-sm font-medium",
                                                    "{c.name}"
                                                }
                                                if can_manage_clusters {
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

                    // Tokens section (org admins only)
                    if can_manage_tokens {
                        div { class: "lg:col-span-2 bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 p-4",
                            h3 { class: "text-lg font-semibold mb-3", "Tokens" }

                            div { class: "flex gap-2 mb-4",
                                input {
                                    class: "border border-gray-300 dark:border-gray-600 rounded px-2 py-1 flex-1 dark:bg-gray-700 dark:text-white",
                                    r#type: "text",
                                    placeholder: "Token label...",
                                    value: "{token_label}",
                                    oninput: move |e| token_label.set(e.value()),
                                }
                                button {
                                    class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700 disabled:opacity-50",
                                    disabled: token_label.read().trim().is_empty(),
                                    onclick: {
                                        let oid = id.clone();
                                        move |_| {
                                            let oid = oid.clone();
                                            let label = token_label.read().clone();
                                            async move {
                                                match create_org_token(oid, label).await {
                                                    Ok(raw) => {
                                                        created_token.set(Some(raw));
                                                        token_label.set(String::new());
                                                        tokens_future.restart();
                                                    }
                                                    Err(_) => {}
                                                }
                                            }
                                        }
                                    },
                                    "Create Token"
                                }
                            }

                            if let Some(raw) = &*created_token.read() {
                                div { class: "bg-green-100 dark:bg-green-900 border border-green-400 dark:border-green-600 text-green-800 dark:text-green-200 rounded p-3 mb-4 text-sm",
                                    p { class: "font-semibold mb-1", "Token created! Copy it now — it won't be shown again." }
                                    code { class: "block break-all", "{raw}" }
                                }
                            }

                            if tokens.is_empty() {
                                p { class: "text-gray-500 dark:text-gray-400 text-sm", "No tokens yet." }
                            } else {
                                div { class: "divide-y divide-gray-200 dark:divide-gray-700",
                                    for t in &tokens {
                                        {
                                            let tid = t.id.clone();
                                            let oid_for_revoke = id.clone();
                                            let is_revoked = t.revoked;
                                            let label = t.label.clone();
                                            let created = t.created_at.format("%Y-%m-%d %H:%M").to_string();
                                            rsx! {
                                                div { class: "flex justify-between items-center py-2",
                                                    div {
                                                        span { class: "text-sm font-medium", "{label}" }
                                                        if is_revoked {
                                                            span { class: "ml-2 text-xs bg-red-100 dark:bg-red-900 text-red-700 dark:text-red-300 px-1.5 py-0.5 rounded", "revoked" }
                                                        } else {
                                                            span { class: "ml-2 text-xs bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300 px-1.5 py-0.5 rounded", "active" }
                                                        }
                                                        span { class: "text-sm text-gray-500 dark:text-gray-400 ml-2", "{created}" }
                                                    }
                                                    if !is_revoked {
                                                        button {
                                                            class: "text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 text-sm",
                                                            onclick: {
                                                                let tid = tid.clone();
                                                                let oid = oid_for_revoke.clone();
                                                                move |_| {
                                                                    let tid = tid.clone();
                                                                    let oid = oid.clone();
                                                                    async move {
                                                                        let _ = revoke_org_token(oid, tid).await;
                                                                        created_token.set(None);
                                                                        tokens_future.restart();
                                                                    }
                                                                }
                                                            },
                                                            "Revoke"
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
        }
        Some(Err(e)) => {
            rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" } }
        }
        None => rsx! { p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." } },
    }
}
