use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use super::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, ErrorText, HelpText,
    SectionHeading,
};
use crate::web::app::Route;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserInfo {
    id: String,
    email: String,
    name: String,
    is_admin: bool,
    created_at: String,
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
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let row = sqlx::query_as::<
        _,
        (
            uuid::Uuid,
            String,
            String,
            bool,
            chrono::DateTime<chrono::Utc>,
        ),
    >("SELECT id, email, name, is_admin, created_at FROM users WHERE id = $1")
    .bind(uid)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(UserInfo {
        id: row.0.to_string(),
        email: row.1,
        name: row.2,
        is_admin: row.3,
        created_at: row.4.format("%Y-%m-%d %H:%M").to_string(),
    })
}

#[server]
async fn get_user_orgs(user_id: String) -> Result<Vec<UserOrgEntry>, ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let rows = sqlx::query_as::<_, (uuid::Uuid, String, String)>(
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
        .map(|(org_id, name, role)| UserOrgEntry {
            organization_id: org_id.to_string(),
            name,
            role,
        })
        .collect())
}

#[server]
async fn get_available_orgs_for_user(user_id: String) -> Result<Vec<OrgOption>, ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let rows = sqlx::query_as::<_, (uuid::Uuid, String)>(
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
        .map(|(id, name)| OrgOption {
            id: id.to_string(),
            name,
        })
        .collect())
}

#[server]
async fn add_user_to_org(
    user_id: String,
    org_id: String,
    role: String,
) -> Result<(), ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
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
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
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
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;

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
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

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

fn role_variant(role: &str) -> BadgeVariant {
    match role {
        "admin" => BadgeVariant::Accent,
        "write" => BadgeVariant::Info,
        _ => BadgeVariant::Neutral,
    }
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
                div { class: "flex flex-col sm:flex-row sm:justify-between sm:items-end gap-3 mb-4",
                    div { class: "min-w-0",
                        p { class: "text-xs font-semibold uppercase tracking-wider text-fg-muted mb-2", "Users" }
                        h1 { class: "text-2xl font-bold text-fg mb-0 break-all", "{info.email}" }
                        p { class: "text-sm text-fg-muted mt-2",
                            if !info.name.is_empty() {
                                span { "{info.name} · " }
                            }
                            "Created {info.created_at}"
                        }
                    }
                    div { class: "flex gap-2 items-center",
                        // Admin toggle
                        div { class: "flex items-center gap-2 mr-4",
                            input {
                                r#type: "checkbox",
                                checked: is_admin,
                                class: "h-4 w-4 rounded border-line text-brand focus:ring-brand",
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
                            label { class: "text-sm font-medium text-fg-strong", "Admin" }
                        }
                        // Impersonate
                        Button { variant: ButtonVariant::Warn, size: ButtonSize::Sm,
                            onclick: {
                                let uid = user_id.clone();
                                move |_| {
                                    let js = format!(
                                        "document.cookie = 'impersonate_user_id={uid}; Path=/; SameSite=Lax'; window.location.href = '/';"
                                    );
                                    document::eval(&js);
                                }
                            },
                            "Impersonate"
                        }
                        // Delete (with confirm flow)
                        if *confirm_delete.read() {
                            span { class: "text-sm text-danger mr-2", "Are you sure?" }
                            Button { variant: ButtonVariant::Danger, size: ButtonSize::Sm,
                                onclick: {
                                    let uid = id.clone();
                                    move |_| {
                                        let uid = uid.clone();
                                        async move {
                                            let _ = delete_user(uid).await;
                                            nav.push(Route::UserList {});
                                        }
                                    }
                                },
                                "Yes, delete"
                            }
                            Button { variant: ButtonVariant::Secondary, size: ButtonSize::Sm,
                                onclick: move |_| confirm_delete.set(false),
                                "Cancel"
                            }
                        } else {
                            Button { variant: ButtonVariant::Danger, size: ButtonSize::Sm,
                                onclick: move |_| confirm_delete.set(true),
                                "Delete"
                            }
                        }
                    }
                }

                // Organizations section
                Card { class: "p-4",
                    SectionHeading { "Organizations" }

                    div { class: "flex gap-2 mb-4",
                        select {
                            class: "input flex-1 w-auto py-1 text-sm",
                            onchange: move |e| {
                                let val = e.value();
                                if val.is_empty() {
                                    selected_org.set(None);
                                } else {
                                    selected_org.set(Some(val));
                                }
                            },
                            option { value: "", "Select organization..." }
                            for o in &avail_orgs {
                                {
                                    let oid = o.id.clone();
                                    let oname = o.name.clone();
                                    rsx! { option { value: "{oid}", "{oname}" } }
                                }
                            }
                        }
                        select {
                            class: "input w-24 py-1 text-sm",
                            value: "{selected_org_role}",
                            onchange: move |e| selected_org_role.set(e.value()),
                            option { value: "read", "Read" }
                            option { value: "write", "Write" }
                            option { value: "admin", "Admin" }
                        }
                        Button { size: ButtonSize::Sm,
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
                            "Add"
                        }
                    }

                    if orgs.is_empty() {
                        HelpText { "No organization memberships." }
                    } else {
                        div { class: "divide-y divide-line-soft",
                            for o in &orgs {
                                {
                                    let oid = o.organization_id.clone();
                                    let uid = id.clone();
                                    let oname = o.name.clone();
                                    let role = o.role.clone();
                                    let variant = role_variant(&o.role);
                                    rsx! {
                                        div { class: "flex justify-between items-center py-2",
                                            div { class: "flex items-center gap-2",
                                                Link { to: Route::OrganizationDetail { id: oid.clone() },
                                                    class: "link text-sm font-medium",
                                                    "{oname}"
                                                }
                                                Badge { variant, "{role}" }
                                            }
                                            button { class: "link-danger text-sm",
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
        Some(Err(e)) => rsx! { ErrorText { "Error: {e}" } },
        None => rsx! { HelpText { "Loading..." } },
    }
}
