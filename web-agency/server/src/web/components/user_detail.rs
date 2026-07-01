use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use super::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, ErrorText, HelpText,
    SectionHeading,
};
use crate::web::app::Route;

// User read/mutation endpoints now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::organizations::{
    OrgMemberAddInput, OrgMemberRemoveInput, add_org_member, remove_org_member,
};
use crate::api_mcp::endpoints::users::{UserGetInput, UserUpdateInput, get_user, set_user_admin};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct OrgOption {
    id: String,
    name: String,
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

// delete_user now lives in the shared api_mcp layer.
use crate::api_mcp::endpoints::users::{UserDeleteInput, delete_user};

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
        async move { get_user(UserGetInput { id }).await }
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
            let orgs = info.organizations.clone();
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
                                            let _ = set_user_admin(UserUpdateInput {
                                                id: uid,
                                                is_admin: new_val,
                                            })
                                            .await;
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
                                            let _ = delete_user(UserDeleteInput { id: uid }).await;
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
                                            if let (Ok(uid), Ok(oid)) =
                                                (uid.parse::<uuid::Uuid>(), oid.parse::<uuid::Uuid>())
                                            {
                                                let _ = add_org_member(OrgMemberAddInput {
                                                    organization_id: oid,
                                                    user_id: Some(uid),
                                                    email: None,
                                                    role,
                                                })
                                                .await;
                                            }
                                            selected_org.set(None);
                                            user_future.restart();
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
                                                            if let (Ok(uid), Ok(oid)) = (
                                                                uid.parse::<uuid::Uuid>(),
                                                                oid.parse::<uuid::Uuid>(),
                                                            ) {
                                                                let _ = remove_org_member(OrgMemberRemoveInput {
                                                                    organization_id: oid,
                                                                    user_id: uid,
                                                                })
                                                                .await;
                                                            }
                                                            user_future.restart();
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
