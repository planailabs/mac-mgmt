use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonKind, ButtonVariant, Card, FormField, PageHeader,
};

// The credential read/mutation endpoints now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::credentials::{
    CredentialDeleteInput, CredentialInfo, CredentialTestInput, CredentialUpdateInput,
    delete_credential, test_credential_conn, update_credential,
};

/// Composite edit-form payload: the credential itself plus the org dropdown
/// options and the caller's admin flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredEditData {
    cred: CredentialInfo,
    orgs: Vec<crate::web::user::OrgOption>,
    is_admin: bool,
}

#[server]
async fn load_credential_form(credential_id: Uuid) -> Result<CredEditData, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let principal = crate::web::user::principal_from(&user);

    let cred = crate::api_mcp::endpoints::credentials::credential_get(
        &pool,
        &principal,
        crate::api_mcp::endpoints::credentials::CredentialGetInput { id: credential_id },
    )
    .await
    .map_err(crate::web::user::to_serverfn)?;

    let orgs = crate::web::user::list_user_orgs(&user, &pool).await?;

    Ok(CredEditData {
        cred,
        orgs,
        is_admin: user.is_admin,
    })
}

#[component]
pub fn CredentialEdit(id: String) -> Element {
    let cred_id = Uuid::parse_str(&id).ok();
    let data = use_server_future(move || {
        let cid = cred_id;
        async move {
            match cid {
                Some(id) => load_credential_form(id).await,
                None => Err(ServerFnError::new("invalid ID")),
            }
        }
    })?;

    let (cred, org_list, is_admin) = match &*data.read() {
        Some(Ok(d)) => (d.cred.clone(), d.orgs.clone(), d.is_admin),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let mut name = use_signal(move || cred.name.clone());
    let mut org_id = use_signal(move || {
        cred.organization_id
            .map(|id| id.to_string())
            .unwrap_or_default()
    });
    let mut new_data = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let mut test_result = use_signal(|| None::<String>);
    let mut testing = use_signal(|| false);
    let mut deleting = use_signal(|| false);
    let mut confirm_delete = use_signal(|| false);
    let nav = use_navigator();

    rsx! {
        PageHeader { "Edit Credential" }

        Card { div { class: "p-6 space-y-4",
            div { class: "flex items-center gap-3 mb-2",
                Badge { variant: BadgeVariant::Info, "{cred.credential_type}" }
                span { class: "text-sm text-fg-muted", "Created {cred.created_at}" }
            }

            form {
                class: "space-y-4",
                onsubmit: move |evt| {
                    evt.prevent_default();
                    let n = name.read().clone();
                    let oid_str = org_id.read().clone();
                    let nd = new_data.read().clone();
                    let cid = cred.id;
                    let nav = nav.clone();
                    saving.set(true); error.set(None);
                    spawn(async move {
                        let oid = uuid::Uuid::parse_str(&oid_str).ok();
                        let data_opt = if nd.trim().is_empty() { None } else { Some(nd) };
                        match update_credential(CredentialUpdateInput {
                            id: cid,
                            name: n,
                            organization_id: oid,
                            new_data_json: data_opt,
                        }).await {
                            Ok(()) => { nav.push(crate::web::app::Route::CredentialList {}); }
                            Err(e) => error.set(Some(format!("{e}"))),
                        }
                        saving.set(false);
                    });
                },

                FormField { label: "Name",
                    input { class: "input", r#type: "text", required: true, value: "{name}",
                        oninput: move |evt| name.set(evt.value()) }
                }

                FormField { label: if is_admin { "Organization (optional)" } else { "Organization" },
                    select { class: "input", value: "{org_id}", oninput: move |evt| org_id.set(evt.value()),
                        if is_admin {
                            option { value: "", "Global (no organization)" }
                        }
                        for org in &org_list {
                            option { value: "{org.id}", "{org.name}" }
                        }
                    }
                }

                FormField { label: "Update Secret Data (leave empty to keep current)",
                    help: "Only fill this in if you want to replace the credential data.",
                    textarea { class: "input font-mono text-sm", rows: "4",
                        placeholder: match cred.credential_type.as_str() {
                            "cloudflare" => r#"{"api_token": "...", "account_id": "..."}"#,
                            "spaceship" => r#"{"api_key": "...", "api_secret": "..."}"#,
                            "mac-mgmt" => r#"{"server_url": "...", "token": "..."}"#,
                            "changedetection" => r#"{"api_url": "...", "api_key": "...", "group": "..."}"#,
                            _ => "{}",
                        },
                        value: "{new_data}", oninput: move |evt| new_data.set(evt.value()) }
                }

                if let Some(err) = &*error.read() {
                    div { class: "text-danger text-sm", "{err}" }
                }

                div { class: "flex gap-3",
                    Button { variant: ButtonVariant::Primary, kind: ButtonKind::Submit, disabled: *saving.read(),
                        if *saving.read() { "Saving..." } else { "Save Changes" } }
                    Link { to: crate::web::app::Route::CredentialList {}, class: "btn btn-secondary", "Cancel" }
                }
            }
        }}

        // Test connection
        Card { div { class: "p-6 mt-4",
            div { class: "flex items-center gap-3",
                Button { variant: ButtonVariant::Secondary, disabled: *testing.read(),
                    onclick: {
                        let cid = cred.id;
                        move |_| {
                            testing.set(true); test_result.set(None);
                            spawn(async move {
                                match test_credential_conn(CredentialTestInput { id: cid }).await {
                                    Ok(msg) => test_result.set(Some(msg)),
                                    Err(e) => test_result.set(Some(format!("Error: {e}"))),
                                }
                                testing.set(false);
                            });
                        }
                    },
                    if *testing.read() { "Testing..." } else { "Test Connection" }
                }
                if let Some(msg) = &*test_result.read() {
                    span { class: "text-sm text-fg-muted", "{msg}" }
                }
            }
        }}

        // Delete
        Card { div { class: "p-6 mt-4 flex items-center justify-between",
            div {
                div { class: "font-medium text-danger", "Delete this credential" }
                div { class: "text-sm text-fg-muted", "Domains and webspaces using this credential will lose their link." }
            }
            if !*confirm_delete.read() {
                Button { variant: ButtonVariant::Danger, onclick: move |_| confirm_delete.set(true), "Delete" }
            } else {
                div { class: "flex items-center gap-2",
                    Button { variant: ButtonVariant::Danger, disabled: *deleting.read(),
                        onclick: {
                            let cid = cred.id;
                            let nav = nav.clone();
                            move |_| {
                                let nav = nav.clone();
                                deleting.set(true);
                                spawn(async move {
                                    let _ = delete_credential(CredentialDeleteInput { id: cid }).await;
                                    nav.push(crate::web::app::Route::CredentialList {});
                                });
                            }
                        },
                        if *deleting.read() { "Deleting..." } else { "Confirm Delete" }
                    }
                    Button { variant: ButtonVariant::Secondary, onclick: move |_| confirm_delete.set(false), "Cancel" }
                }
            }
        }}
    }
}
