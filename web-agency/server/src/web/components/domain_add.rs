use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Button, ButtonKind, ButtonVariant, FormField, PageHeader};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrgOption {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialOption {
    id: Uuid,
    name: String,
    credential_type: String,
}

#[server]
async fn list_orgs_and_cf_creds() -> Result<(Vec<OrgOption>, Vec<CredentialOption>), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let orgs = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM organizations ORDER BY name")
            .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT o.id, o.name FROM organizations o \
             JOIN organization_members om ON om.organization_id = o.id \
             WHERE om.user_id = $1 ORDER BY o.name",
        )
        .bind(user.id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    };

    let creds = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, name, credential_type FROM credentials \
         WHERE credential_type = 'cloudflare' ORDER BY name",
    )
    .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok((
        orgs.into_iter().map(|(id, name)| OrgOption { id, name }).collect(),
        creds.into_iter().map(|(id, name, credential_type)| CredentialOption { id, name, credential_type }).collect(),
    ))
}

/// Add a domain: creates a Cloudflare zone (or finds existing one).
#[server]
async fn add_domain(
    org_id: Uuid,
    domain_name: String,
    cf_credential_id: Option<Uuid>,
    registrar_type: String,
) -> Result<Uuid, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    let mut zone_id: Option<String> = None;

    // If a Cloudflare credential is provided, try to add/find the zone
    if let Some(cred_id) = cf_credential_id {
        let cred = sqlx::query_as::<_, (Vec<u8>,)>(
            "SELECT encrypted_data FROM credentials WHERE id = $1 AND credential_type = 'cloudflare'",
        )
        .bind(cred_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Cloudflare credential not found"))?;

        let decrypted = crate::crypto::decrypt(&cred.0)
            .map_err(|e| ServerFnError::new(format!("decryption failed: {e}")))?;
        let data: serde_json::Value = serde_json::from_slice(&decrypted)
            .map_err(|e| ServerFnError::new(format!("invalid credential data: {e}")))?;

        let token = data["api_token"].as_str()
            .ok_or_else(|| ServerFnError::new("missing api_token"))?;
        let configured_account_id = data["account_id"].as_str().unwrap_or("");

        let client = cloudflare_api::Client::new(token);
        let account_id = client.resolve_account_id(configured_account_id).await
            .map_err(|e| ServerFnError::new(format!("failed to resolve account ID: {e}")))?;

        // Check if zone already exists
        let existing = client.list_zones(Some(&domain_name)).await
            .map_err(|e| ServerFnError::new(format!("CF API error: {e}")))?;

        if let Some(zone) = existing.first() {
            zone_id = Some(zone.id.clone());
            tracing::info!("domain {domain_name} already exists as zone {}", zone.id);
        } else {
            let zone = client.create_zone(&domain_name, &account_id).await
                .map_err(|e| ServerFnError::new(format!("CF zone creation failed: {e}")))?;
            zone_id = Some(zone.id);
            tracing::info!("created CF zone for {domain_name}");
        }
    }

    let registrar = if registrar_type.is_empty() { None } else { Some(registrar_type.as_str()) };

    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO domains (organization_id, name, registrar_type, cloudflare_credential_id, cloudflare_zone_id) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(org_id)
    .bind(&domain_name)
    .bind(registrar)
    .bind(cf_credential_id)
    .bind(&zone_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(format!("failed to add domain: {e}")))?;

    Ok(id)
}

#[component]
pub fn DomainAdd() -> Element {
    let data = use_server_future(list_orgs_and_cf_creds)?;
    let (org_list, cred_list) = match &*data.read() {
        Some(Ok((o, c))) => (o.clone(), c.clone()),
        _ => (vec![], vec![]),
    };

    let mut domain_name = use_signal(String::new);
    let mut org_id = use_signal(|| org_list.first().map(|o| o.id.to_string()).unwrap_or_default());
    let mut cf_cred_id = use_signal(String::new);
    let mut registrar_type = use_signal(|| "external".to_string());
    let mut error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let nav = use_navigator();

    rsx! {
        PageHeader { "Add Domain" }

        form {
            class: "card p-6 mt-4 max-w-xl space-y-4",
            onsubmit: move |evt| {
                evt.prevent_default();
                let dn = domain_name.read().clone();
                let oid_str = org_id.read().clone();
                let cred_str = cf_cred_id.read().clone();
                let reg = registrar_type.read().clone();
                let nav = nav.clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    let oid = uuid::Uuid::parse_str(&oid_str).ok();
                    let cred = uuid::Uuid::parse_str(&cred_str).ok();
                    if let Some(oid) = oid {
                        match add_domain(oid, dn, cred, reg).await {
                            Ok(_) => { nav.push(crate::web::app::Route::DomainList {}); }
                            Err(e) => error.set(Some(format!("{e}"))),
                        }
                    } else {
                        error.set(Some("Select an organization".into()));
                    }
                    saving.set(false);
                });
            },

            FormField { label: "Organization",
                select {
                    class: "input",
                    value: "{org_id}",
                    oninput: move |evt| org_id.set(evt.value()),
                    for org in &org_list {
                        option { value: "{org.id}", "{org.name}" }
                    }
                }
            }

            FormField { label: "Domain Name",
                input {
                    class: "input",
                    r#type: "text",
                    placeholder: "example.com",
                    required: true,
                    value: "{domain_name}",
                    oninput: move |evt| domain_name.set(evt.value()),
                }
            }

            FormField { label: "Cloudflare Credential (optional)",
                help: "Select to manage DNS via Cloudflare. Existing zones are detected automatically.",
                select {
                    class: "input",
                    value: "{cf_cred_id}",
                    oninput: move |evt| cf_cred_id.set(evt.value()),
                    option { value: "", "None" }
                    for cred in &cred_list {
                        option { value: "{cred.id}", "{cred.name}" }
                    }
                }
            }

            FormField { label: "Registrar",
                select {
                    class: "input",
                    value: "{registrar_type}",
                    oninput: move |evt| registrar_type.set(evt.value()),
                    option { value: "external", "External" }
                    option { value: "cloudflare", "Cloudflare" }
                    option { value: "spaceship", "Spaceship" }
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
                    if *saving.read() { "Adding..." } else { "Add Domain" }
                }
                Link {
                    to: crate::web::app::Route::DomainList {},
                    class: "btn btn-secondary",
                    "Cancel"
                }
            }
        }
    }
}
