use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonKind, ButtonVariant, Card, FormField, PageHeader,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialData {
    id: Uuid,
    name: String,
    credential_type: String,
    organization_id: Option<Uuid>,
    created_at: String,
}

#[server]
async fn get_credential(
    credential_id: Uuid,
) -> Result<(CredentialData, Vec<crate::web::user::OrgOption>, bool), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (Uuid, String, String, Option<Uuid>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, name, credential_type, organization_id, created_at FROM credentials WHERE id = $1",
    )
    .bind(credential_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("credential not found"))?;

    let (id, name, credential_type, organization_id, created_at) = row;

    crate::web::user::require_credential_read(&user, &pool, credential_id).await?;

    let orgs = crate::web::user::list_user_orgs(&user, &pool).await?;

    Ok((
        CredentialData {
            id,
            name,
            credential_type,
            organization_id,
            created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
        },
        orgs,
        user.is_admin,
    ))
}

#[server]
async fn update_credential(
    credential_id: Uuid,
    name: String,
    org_id: Option<Uuid>,
    new_data_json: Option<String>,
) -> Result<(), ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    crate::web::user::require_credential_write(&user, &pool, credential_id).await?;

    // Validate target org assignment.
    match org_id {
        None => user.require_admin()?,
        Some(oid) => user.require_org_write(&oid)?,
    }

    sqlx::query(
        "UPDATE credentials SET name = $1, organization_id = $2, updated_at = now() WHERE id = $3",
    )
    .bind(&name)
    .bind(org_id)
    .bind(credential_id)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if let Some(json) = new_data_json {
        if !json.trim().is_empty() {
            let _: serde_json::Value = serde_json::from_str(&json)
                .map_err(|e| ServerFnError::new(format!("invalid JSON: {e}")))?;
            let encrypted = crate::crypto::encrypt(json.as_bytes())
                .map_err(|e| ServerFnError::new(format!("encryption failed: {e}")))?;
            sqlx::query(
                "UPDATE credentials SET encrypted_data = $1, updated_at = now() WHERE id = $2",
            )
            .bind(&encrypted)
            .bind(credential_id)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        }
    }

    Ok(())
}

#[server]
async fn delete_credential(credential_id: Uuid) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    crate::web::user::require_credential_write(&user, &pool, credential_id).await?;

    sqlx::query("DELETE FROM credentials WHERE id = $1")
        .bind(credential_id)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn test_credential_conn(credential_id: Uuid) -> Result<String, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    crate::web::user::require_credential_read(&user, &pool, credential_id).await?;

    let cred_type =
        sqlx::query_scalar::<_, String>("SELECT credential_type FROM credentials WHERE id = $1")
            .bind(credential_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .ok_or_else(|| ServerFnError::new("credential not found"))?;

    match cred_type.as_str() {
        "cloudflare" => {
            let client = crate::credentials::cf_client(&pool, credential_id)
                .await
                .map_err(|e| ServerFnError::new(format!("{e}")))?;
            let zones = client
                .list_zones(None)
                .await
                .map_err(|e| ServerFnError::new(format!("CF: {e}")))?;
            Ok(format!("OK — {} zone(s) accessible", zones.len()))
        }
        "spaceship" => {
            let client = crate::credentials::spaceship_client(&pool, credential_id)
                .await
                .map_err(|e| ServerFnError::new(format!("{e}")))?;
            let resp = client
                .list_domains(0, 1)
                .await
                .map_err(|e| ServerFnError::new(format!("SS: {e}")))?;
            Ok(format!("OK — {} domain(s)", resp.total_count.unwrap_or(0)))
        }
        "changedetection" => {
            let (client, _group) = crate::credentials::changedetection_client(&pool, credential_id)
                .await
                .map_err(|e| ServerFnError::new(format!("{e}")))?;
            let info = client
                .get_system_info()
                .await
                .map_err(|e| ServerFnError::new(format!("CD: {e}")))?;
            let info = info.into_inner();
            Ok(format!(
                "OK — v{}, {} watch(es)",
                info.version.as_deref().unwrap_or("?"),
                info.watch_count.unwrap_or(0),
            ))
        }
        _ => Err(ServerFnError::new("unknown type")),
    }
}

#[component]
pub fn CredentialEdit(id: String) -> Element {
    let cred_id = Uuid::parse_str(&id).ok();
    let data = use_server_future(move || {
        let cid = cred_id;
        async move {
            match cid {
                Some(id) => get_credential(id).await,
                None => Err(ServerFnError::new("invalid ID")),
            }
        }
    })?;

    let (cred, org_list, is_admin) = match &*data.read() {
        Some(Ok((c, o, a))) => (c.clone(), o.clone(), *a),
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
                        match update_credential(cid, n, oid, data_opt).await {
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
                                match test_credential_conn(cid).await {
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
                                    let _ = delete_credential(cid).await;
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
