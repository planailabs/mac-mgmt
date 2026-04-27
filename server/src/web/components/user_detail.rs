use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use dioxus_i18n::t;

use crate::web::app::Route;
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserInfo {
    id: String,
    email: String,
    name: String,
    is_admin: bool,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserOrgEntry {
    organization_id: String,
    name: String,
    role: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct OrgOption {
    id: String,
    name: String,
}

#[server]
async fn get_user(id: String) -> Result<UserInfo, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        email: String,
        name: String,
        is_admin: bool,
        created_at: DateTime<Utc>,
    }

    let row = sqlx::query_as::<_, Row>(
        "SELECT id, email, name, is_admin, created_at FROM users WHERE id = $1",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(UserInfo {
        id: row.id.to_string(),
        email: row.email,
        name: row.name,
        is_admin: row.is_admin,
        created_at: row.created_at,
    })
}

#[server]
async fn get_user_orgs(user_id: String) -> Result<Vec<UserOrgEntry>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        organization_id: uuid::Uuid,
        name: String,
        role: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT om.organization_id, o.name, om.role \
         FROM organization_members om \
         JOIN organizations o ON o.id = om.organization_id \
         WHERE om.user_id = $1 \
         ORDER BY o.name",
    )
    .bind(uid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| UserOrgEntry {
            organization_id: r.organization_id.to_string(),
            name: r.name,
            role: r.role,
        })
        .collect())
}

#[server]
async fn get_available_orgs_for_user(user_id: String) -> Result<Vec<OrgOption>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name FROM organizations \
         WHERE id NOT IN (SELECT organization_id FROM organization_members WHERE user_id = $1) \
         ORDER BY name",
    )
    .bind(uid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| OrgOption {
            id: r.id.to_string(),
            name: r.name,
        })
        .collect())
}

#[server]
async fn add_user_to_org(
    user_id: String,
    org_id: String,
    role: String,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    if !["admin", "write", "read"].contains(&role.as_str()) {
        return Err(ServerFnError::new("invalid role"));
    }
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    sqlx::query("INSERT INTO organization_members (user_id, organization_id, role) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
        .bind(uid)
        .bind(oid)
        .bind(&role)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn remove_user_from_org(user_id: String, org_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let oid: uuid::Uuid = org_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    sqlx::query("DELETE FROM organization_members WHERE user_id = $1 AND organization_id = $2")
        .bind(uid)
        .bind(oid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn toggle_user_admin(user_id: String, is_admin: bool) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;

    // Prevent de-admining yourself
    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if uid == user.id && !is_admin {
        return Err(ServerFnError::new("cannot remove your own admin status"));
    }

    let pool = crate::server_pool()?;
    sqlx::query("UPDATE users SET is_admin = $2 WHERE id = $1")
        .bind(uid)
        .bind(is_admin)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn delete_user(id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    // Prevent deleting yourself
    if uid == user.id {
        return Err(ServerFnError::new("cannot delete yourself"));
    }

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(uid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn UserDetail(id: String) -> Element {
    let id_for_user = id.clone();
    let mut user_future = use_server_future(move || {
        let id = id_for_user.clone();
        async move { get_user(id).await }
    })?;

    let id_for_orgs = id.clone();
    let mut orgs_future = use_server_future(move || {
        let id = id_for_orgs.clone();
        async move { get_user_orgs(id).await }
    })?;

    let id_for_avail = id.clone();
    let mut avail_future = use_server_future(move || {
        let id = id_for_avail.clone();
        async move { get_available_orgs_for_user(id).await }
    })?;

    let mut selected_org = use_signal(|| Option::<String>::None);
    let mut selected_org_role = use_signal(|| "read".to_string());
    let mut confirm_delete = use_signal(|| false);
    let nav = navigator();

    match &*user_future.read() {
        Some(Ok(info)) => {
            let orgs = match &*orgs_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };
            let avail_orgs = match &*avail_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };

            let is_admin = info.is_admin;
            let user_id = info.id.clone();

            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    div {
                        h2 { class: "text-2xl font-bold", "{info.email}" }
                        p { class: "text-gray-500 dark:text-gray-400 text-sm",
                            if !info.name.is_empty() {
                                span { "{info.name} · " }
                            }
                            {t!("user-detail-created", date: info.created_at.format("%Y-%m-%d %H:%M").to_string())}
                        }
                    }
                    div { class: "flex gap-2 items-center",
                        // Admin toggle
                        div { class: "flex items-center gap-2 mr-4",
                            input {
                                r#type: "checkbox",
                                checked: is_admin,
                                class: "h-4 w-4 rounded border-gray-300 text-blue-600 focus:ring-blue-500",
                                onchange: {
                                    let uid = user_id.clone();
                                    move |e: Event<FormData>| {
                                        let uid = uid.clone();
                                        let new_val = e.checked();
                                        async move {
                                            let _ = toggle_user_admin(uid, new_val).await;
                                            user_future.restart();
                                        }
                                    }
                                },
                            }
                            label { class: "text-sm font-medium text-gray-700 dark:text-gray-200", {t!("admin")} }
                        }
                        // Impersonate
                        button {
                            class: "bg-yellow-500 text-yellow-900 px-3 py-1 rounded text-sm hover:bg-yellow-600",
                            onclick: {
                                let uid = user_id.clone();
                                move |_| {
                                    let js = format!(
                                        "document.cookie = 'impersonate_user_id={uid}; Path=/; SameSite=Lax'; window.location.href = '/';"
                                    );
                                    document::eval(&js);
                                }
                            },
                            {t!("user-detail-impersonate")}
                        }
                        // Delete
                        if *confirm_delete.read() {
                            span { class: "text-sm text-red-600 dark:text-red-400 mr-2", {t!("user-detail-confirm")} }
                            button {
                                class: "bg-red-600 text-white px-3 py-1 rounded text-sm hover:bg-red-700",
                                onclick: {
                                    let uid = id.clone();
                                    move |_| {
                                        let uid = uid.clone();
                                        let nav = nav.clone();
                                        async move {
                                            let _ = delete_user(uid).await;
                                            nav.push(Route::UserList {});
                                        }
                                    }
                                },
                                {t!("user-detail-yes-delete")}
                            }
                            button {
                                class: "bg-gray-300 dark:bg-gray-600 text-gray-700 dark:text-gray-200 px-3 py-1 rounded text-sm",
                                onclick: move |_| confirm_delete.set(false),
                                {t!("cancel")}
                            }
                        } else {
                            button {
                                class: "bg-red-600 text-white px-3 py-1 rounded text-sm hover:bg-red-700",
                                onclick: move |_| confirm_delete.set(true),
                                {t!("user-detail-delete")}
                            }
                        }
                    }
                }

                // Organizations section
                div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 p-4",
                    h3 { class: "text-lg font-semibold mb-3", {t!("user-detail-orgs")} }

                    div { class: "flex gap-2 mb-4",
                        select {
                            class: "border border-gray-300 dark:border-gray-600 rounded px-2 py-1 flex-1 dark:bg-gray-700 dark:text-white",
                            onchange: move |e| {
                                let val = e.value();
                                if val.is_empty() {
                                    selected_org.set(None);
                                } else {
                                    selected_org.set(Some(val));
                                }
                            },
                            option { value: "", {t!("user-detail-select-org")} }
                            for o in &avail_orgs {
                                {
                                    let oid = o.id.clone();
                                    let oname = o.name.clone();
                                    rsx! { option { value: "{oid}", "{oname}" } }
                                }
                            }
                        }
                        select {
                            class: "border border-gray-300 dark:border-gray-600 rounded px-2 py-1 w-24 dark:bg-gray-700 dark:text-white",
                            value: "{selected_org_role}",
                            onchange: move |e| selected_org_role.set(e.value()),
                            option { value: "read", {t!("org-detail-role-read")} }
                            option { value: "write", {t!("org-detail-role-write")} }
                            option { value: "admin", {t!("org-detail-role-admin")} }
                        }
                        button {
                            class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700 disabled:opacity-50",
                            disabled: selected_org.read().is_none(),
                            onclick: {
                                let uid = id.clone();
                                move |_| {
                                    let uid = uid.clone();
                                    let oid = selected_org.read().clone();
                                    let role = selected_org_role.read().clone();
                                    async move {
                                        if let Some(oid) = oid {
                                            let _ = add_user_to_org(uid, oid, role).await;
                                            selected_org.set(None);
                                            orgs_future.restart();
                                            avail_future.restart();
                                        }
                                    }
                                }
                            },
                            {t!("add")}
                        }
                    }

                    if orgs.is_empty() {
                        p { class: "text-gray-500 dark:text-gray-400 text-sm", {t!("user-detail-no-orgs")} }
                    } else {
                        div { class: "divide-y divide-gray-200 dark:divide-gray-700",
                            for o in &orgs {
                                {
                                    let oid = o.organization_id.clone();
                                    let uid = id.clone();
                                    let oname = o.name.clone();
                                    let role = o.role.clone();
                                    let badge_class = match o.role.as_str() {
                                        "admin" => "bg-purple-100 dark:bg-purple-900 text-purple-700 dark:text-purple-300",
                                        "write" => "bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300",
                                        _ => "bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300",
                                    };
                                    rsx! {
                                        div { class: "flex justify-between items-center py-2",
                                            div { class: "flex items-center gap-2",
                                                Link {
                                                    to: Route::OrganizationDetail { id: oid.clone() },
                                                    class: "text-blue-600 dark:text-blue-400 hover:underline text-sm font-medium",
                                                    "{oname}"
                                                }
                                                span { class: "text-xs px-1.5 py-0.5 rounded {badge_class}", "{role}" }
                                            }
                                            button {
                                                class: "text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 text-sm",
                                                onclick: {
                                                    let oid = oid.clone();
                                                    let uid = uid.clone();
                                                    move |_| {
                                                        let oid = oid.clone();
                                                        let uid = uid.clone();
                                                        async move {
                                                            let _ = remove_user_from_org(uid, oid).await;
                                                            orgs_future.restart();
                                                            avail_future.restart();
                                                        }
                                                    }
                                                },
                                                {t!("remove")}
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
        Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", {t!("error-message", message: e.to_string())} } },
        None => rsx! { p { {t!("loading")} } },
    }
}
