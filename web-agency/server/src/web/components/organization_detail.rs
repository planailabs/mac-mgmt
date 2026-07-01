use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td,
    TdMuted, Th,
};

// Organization read/mutation endpoints now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::organizations::{
    OrgGetInput, OrgMemberAddInput, OrgMemberRemoveInput, OrgUpdateInput, add_org_member, get_org,
    remove_org_member, update_org,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CdCredOption {
    id: Uuid,
    name: String,
}

#[server]
async fn list_cd_creds_for_org() -> Result<Vec<CdCredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM credentials WHERE credential_type = 'changedetection' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|(id, name)| CdCredOption { id, name })
        .collect())
}

#[component]
pub fn OrganizationDetail(id: String) -> Element {
    let org_id = Uuid::parse_str(&id).ok();
    let mut org = use_server_future(move || {
        let oid = org_id;
        async move {
            match oid {
                Some(id) => get_org(OrgGetInput { id }).await,
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

        Card { class: "mt-4 p-4",
            div { class: "flex items-center justify-between",
                div {
                    span { class: "text-sm font-medium", "Show billing to members" }
                    p { class: "text-xs text-fg-muted", "When enabled, organization members can view billing entries." }
                }
                label { class: "relative inline-flex items-center cursor-pointer",
                    input {
                        r#type: "checkbox",
                        class: "sr-only peer",
                        checked: data.show_billing,
                        onchange: {
                            let oid = data.id;
                            let current = data.show_billing;
                            move |_| {
                                let new_val = !current;
                                spawn(async move {
                                    let _ = update_org(OrgUpdateInput {
                                        id: oid,
                                        show_billing: Some(new_val),
                                        default_changedetection_credential_id: None,
                                        clear_default_changedetection: None,
                                    })
                                    .await;
                                    org.restart();
                                });
                            }
                        },
                    }
                    div { class: "w-9 h-5 bg-surface-3 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-fg-muted after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-brand peer-checked:after:bg-white" }
                }
            }
        }

        // Default CD credential
        DefaultCdCredSection { org_id: data.id, current: data.default_changedetection_credential_id }

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
                                                        let _ = remove_org_member(OrgMemberRemoveInput {
                                                            organization_id: oid,
                                                            user_id: uid,
                                                        })
                                                        .await;
                                                        removing.set(None);
                                                        org.restart();
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
                                    match add_org_member(OrgMemberAddInput {
                                        organization_id: oid,
                                        user_id: None,
                                        email: Some(e),
                                        role: r,
                                    })
                                    .await
                                    {
                                        Ok(()) => {
                                            new_email.set(String::new());
                                            org.restart();
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

#[component]
fn DefaultCdCredSection(org_id: Uuid, current: Option<Uuid>) -> Element {
    let cd_creds = use_server_future(list_cd_creds_for_org)?;
    let cred_list: Vec<CdCredOption> = match &*cd_creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    if cred_list.is_empty() {
        return rsx! {};
    }

    let mut selected = use_signal(move || current.map(|id| id.to_string()).unwrap_or_default());
    let mut saving = use_signal(|| false);

    rsx! {
        Card { class: "mt-4 p-4",
            div { class: "flex items-center justify-between gap-4",
                div {
                    span { class: "text-sm font-medium", "Default ChangeDetection.io credential" }
                    p { class: "text-xs text-fg-muted", "Pre-fills the credential dropdown when creating new webspaces in this org." }
                }
                div { class: "flex items-center gap-2",
                    select {
                        class: "input w-48",
                        value: "{selected}",
                        oninput: move |evt| selected.set(evt.value()),
                        option { value: "", "None" }
                        for c in &cred_list {
                            option { value: "{c.id}", "{c.name}" }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: *saving.read(),
                        onclick: {
                            let oid = org_id;
                            move |_| {
                                let cred_str = selected.read().clone();
                                saving.set(true);
                                spawn(async move {
                                    let cid = Uuid::parse_str(&cred_str).ok();
                                    let _ = update_org(OrgUpdateInput {
                                        id: oid,
                                        show_billing: None,
                                        clear_default_changedetection: if cid.is_none() {
                                            Some(true)
                                        } else {
                                            None
                                        },
                                        default_changedetection_credential_id: cid,
                                    })
                                    .await;
                                    saving.set(false);
                                });
                            }
                        },
                        if *saving.read() { "Saving..." } else { "Save" }
                    }
                }
            }
        }
    }
}
