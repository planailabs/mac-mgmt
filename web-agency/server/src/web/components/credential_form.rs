use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Button, ButtonKind, ButtonVariant, FormField, PageHeader};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredFormData {
    orgs: Vec<crate::web::user::OrgOption>,
    is_admin: bool,
}

#[server]
async fn list_orgs_for_cred() -> Result<CredFormData, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let orgs = crate::web::user::list_user_write_orgs(&user, &pool).await?;
    Ok(CredFormData {
        orgs,
        is_admin: user.is_admin,
    })
}

#[server]
async fn create_credential(
    org_id: Option<Uuid>,
    name: String,
    credential_type: String,
    data_json: String,
) -> Result<Uuid, ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    match org_id {
        None => user.require_admin()?,
        Some(oid) => user.require_org_write(&oid)?,
    }

    let _: serde_json::Value = serde_json::from_str(&data_json)
        .map_err(|e| ServerFnError::new(format!("invalid JSON: {e}")))?;

    let encrypted = crate::crypto::encrypt(data_json.as_bytes())
        .map_err(|e| ServerFnError::new(format!("encryption failed: {e}")))?;

    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO credentials (organization_id, name, credential_type, encrypted_data) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(org_id)
    .bind(&name)
    .bind(&credential_type)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(format!("failed to create credential: {e}")))?;

    Ok(id)
}

#[server]
async fn test_credential(credential_id: Uuid) -> Result<String, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    crate::web::user::require_credential_read(&user, &pool, credential_id).await?;

    let cred_type =
        sqlx::query_scalar::<_, String>("SELECT credential_type FROM credentials WHERE id = $1")
            .bind(credential_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

    match cred_type.as_str() {
        "cloudflare" => {
            let client = crate::credentials::cf_client(&pool, credential_id)
                .await
                .map_err(|e| ServerFnError::new(format!("{e}")))?;
            let zones = client
                .list_zones(None)
                .await
                .map_err(|e| ServerFnError::new(format!("Cloudflare API error: {e}")))?;
            Ok(format!("OK - {} zone(s) accessible", zones.len()))
        }
        "spaceship" => {
            let client = crate::credentials::spaceship_client(&pool, credential_id)
                .await
                .map_err(|e| ServerFnError::new(format!("{e}")))?;
            let domains = client
                .list_domains(0, 1)
                .await
                .map_err(|e| ServerFnError::new(format!("Spaceship API error: {e}")))?;
            Ok(format!(
                "OK - {} domain(s) in account",
                domains.total_count.unwrap_or(0)
            ))
        }
        "changedetection" => {
            let (client, _group) = crate::credentials::changedetection_client(&pool, credential_id)
                .await
                .map_err(|e| ServerFnError::new(format!("{e}")))?;
            let info = client
                .get_system_info()
                .await
                .map_err(|e| ServerFnError::new(format!("ChangeDetection API error: {e}")))?;
            let info = info.into_inner();
            Ok(format!(
                "OK - v{}, {} watch(es)",
                info.version.as_deref().unwrap_or("?"),
                info.watch_count.unwrap_or(0),
            ))
        }
        _ => Err(ServerFnError::new("unknown credential type")),
    }
}

#[component]
pub fn CredentialForm() -> Element {
    let orgs = use_server_future(list_orgs_for_cred)?;
    let (org_list, is_admin) = match &*orgs.read() {
        Some(Ok(d)) => (d.orgs.clone(), d.is_admin),
        _ => (vec![], false),
    };

    let mut name = use_signal(String::new);
    let mut credential_type = use_signal(|| "cloudflare".to_string());
    let mut data_json = use_signal(|| r#"{"api_token": "", "account_id": ""}"#.to_string());
    let mut org_id = use_signal(String::new); // empty = global (no org)
    let mut error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let nav = use_navigator();

    rsx! {
        PageHeader { "Add Credential" }

        form {
            class: "card p-6 mt-4 max-w-xl space-y-4",
            onsubmit: move |evt| {
                evt.prevent_default();
                let name_val = name.read().clone();
                let cred_type = credential_type.read().clone();
                let json_val = data_json.read().clone();
                let org_val = org_id.read().clone();
                let nav = nav.clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    let oid = uuid::Uuid::parse_str(&org_val).ok(); // None if empty
                    match create_credential(oid, name_val, cred_type, json_val).await {
                        Ok(_) => { nav.push(crate::web::app::Route::CredentialList {}); }
                        Err(e) => error.set(Some(format!("{e}"))),
                    }
                    saving.set(false);
                });
            },

            FormField { label: "Name",
                input {
                    class: "input",
                    r#type: "text",
                    placeholder: "e.g. My Cloudflare Token",
                    required: true,
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                }
            }

            FormField { label: if is_admin { "Organization (optional)" } else { "Organization" },
                help: if is_admin { "Leave as Global to make this credential available to all organizations." } else { "" },
                select {
                    class: "input",
                    value: "{org_id}",
                    oninput: move |evt| org_id.set(evt.value()),
                    if is_admin {
                        option { value: "", "Global (no organization)" }
                    }
                    for org in &org_list {
                        option { value: "{org.id}", "{org.name}" }
                    }
                }
            }

            FormField { label: "Type",
                select {
                    class: "input",
                    value: "{credential_type}",
                    oninput: move |evt| {
                        let val = evt.value();
                        match val.as_str() {
                            "cloudflare" => data_json.set(r#"{"api_token": "", "account_id": ""}"#.to_string()),
                            "spaceship" => data_json.set(r#"{"api_key": "", "api_secret": ""}"#.to_string()),
                            "mac-mgmt" => data_json.set(r#"{"server_url": "", "token": ""}"#.to_string()),
                            "changedetection" => data_json.set(r#"{"api_url": "", "api_key": "", "group": ""}"#.to_string()),
                            _ => {}
                        }
                        credential_type.set(val);
                    },
                    option { value: "cloudflare", "Cloudflare" }
                    option { value: "spaceship", "Spaceship" }
                    option { value: "mac-mgmt", "mac-mgmt (Relay)" }
                    option { value: "changedetection", "ChangeDetection.io" }
                }
            }

            FormField { label: "Credential Data (JSON)",
                textarea {
                    class: "input font-mono text-sm",
                    rows: "5",
                    value: "{data_json}",
                    oninput: move |evt| data_json.set(evt.value()),
                }
            }

            if let Some(err) = &*error.read() {
                div { class: "text-red-400 text-sm", "{err}" }
            }

            div { class: "flex gap-3",
                Button {
                    variant: ButtonVariant::Primary,
                    kind: ButtonKind::Submit,
                    disabled: *saving.read(),
                    if *saving.read() { "Saving..." } else { "Create" }
                }
                Link {
                    to: crate::web::app::Route::CredentialList {},
                    class: "btn btn-secondary",
                    "Cancel"
                }
            }
        }
    }
}
