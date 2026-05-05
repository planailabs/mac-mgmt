use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrgData {
    id: Uuid,
    name: String,
    members: Vec<MemberRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MemberRow {
    user_id: Uuid,
    email: String,
    name: String,
    role: String,
}

#[server]
async fn get_org(org_id: Uuid) -> Result<OrgData, ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id).fetch_optional(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("organization not found"))?;

    let members = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "SELECT u.id, u.email, u.name, om.role \
         FROM organization_members om JOIN users u ON u.id = om.user_id \
         WHERE om.organization_id = $1 ORDER BY u.email",
    )
    .bind(org_id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    .into_iter()
    .map(|(user_id, email, name, role)| MemberRow { user_id, email, name, role })
    .collect();

    Ok(OrgData { id: org_id, name: org_name, members })
}

#[server]
async fn add_member(org_id: Uuid, email: String, role: String) -> Result<(), ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let user_id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE email = $1")
        .bind(&email).fetch_optional(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new(format!("user {email} not found")))?;

    sqlx::query(
        "INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, $2, $3) \
         ON CONFLICT (organization_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(org_id).bind(user_id).bind(&role)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn remove_member(org_id: Uuid, user_id: Uuid) -> Result<(), ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    sqlx::query("DELETE FROM organization_members WHERE organization_id = $1 AND user_id = $2")
        .bind(org_id).bind(user_id).execute(&pool).await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[component]
pub fn OrganizationDetail(id: String) -> Element {
    let org_id = Uuid::parse_str(&id).ok();
    let org = use_server_future(move || {
        let oid = org_id;
        async move {
            match oid {
                Some(id) => get_org(id).await,
                None => Err(ServerFnError::new("invalid ID")),
            }
        }
    })?;

    let data = match &*org.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let mut new_email = use_signal(String::new);
    let mut new_role = use_signal(|| "read".to_string());
    let mut adding = use_signal(|| false);
    let mut removing: Signal<Option<Uuid>> = use_signal(|| None);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        PageHeader { "{data.name}" }

        SectionHeading { class: "mt-4", "Members" }
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead { tr { Th { "Email" } Th { "Name" } Th { "Role" } Th { "" } } }
                    tbody {
                        if data.members.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "4", "No members" } }
                        }
                        for m in &data.members {
                            {
                                let uid = m.user_id;
                                let oid = data.id;
                                let is_removing = *removing.read() == Some(uid);
                                rsx! {
                                    tr {
                                        Td { "{m.email}" }
                                        TdMuted { "{m.name}" }
                                        Td {
                                            match m.role.as_str() {
                                                "admin" => rsx! { Badge { variant: BadgeVariant::Accent, "admin" } },
                                                "write" => rsx! { Badge { variant: BadgeVariant::Info, "write" } },
                                                _ => rsx! { Badge { "{m.role}" } },
                                            }
                                        }
                                        Td {
                                            Button {
                                                variant: ButtonVariant::Danger,
                                                disabled: is_removing,
                                                onclick: move |_| {
                                                    removing.set(Some(uid));
                                                    spawn(async move {
                                                        let _ = remove_member(oid, uid).await;
                                                        removing.set(None);
                                                        navigator().replace(crate::web::app::Route::OrganizationDetail { id: oid.to_string() });
                                                    });
                                                },
                                                if is_removing { "..." } else { "Remove" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "p-4 border-t border-line-soft",
                div { class: "flex items-end gap-2",
                    FormField { label: "Email",
                        input { class: "input w-56", r#type: "email", placeholder: "user@example.com",
                            value: "{new_email}", oninput: move |evt| new_email.set(evt.value()) }
                    }
                    FormField { label: "Role",
                        select { class: "input w-28", value: "{new_role}",
                            oninput: move |evt| new_role.set(evt.value()),
                            option { value: "read", "read" }
                            option { value: "write", "write" }
                            option { value: "admin", "admin" }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: new_email.read().is_empty() || *adding.read(),
                        onclick: {
                            let oid = data.id;
                            move |_| {
                                let e = new_email.read().clone();
                                let r = new_role.read().clone();
                                adding.set(true);
                                error.set(None);
                                spawn(async move {
                                    match add_member(oid, e, r).await {
                                        Ok(()) => {
                                            new_email.set(String::new());
                                            navigator().replace(crate::web::app::Route::OrganizationDetail { id: oid.to_string() });
                                        }
                                        Err(e) => error.set(Some(format!("{e}"))),
                                    }
                                    adding.set(false);
                                });
                            }
                        },
                        if *adding.read() { "Adding..." } else { "Add Member" }
                    }
                }
                if let Some(err) = &*error.read() {
                    div { class: "mt-2 text-danger text-sm", "{err}" }
                }
            }
        }
    }
}
