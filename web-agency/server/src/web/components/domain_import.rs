use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, Td, TdMuted, Th};

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrgOption {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredOption {
    id: Uuid,
    name: String,
    credential_type: String,
}

/// A domain discovered from a credential, ready for import.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiscoveredDomain {
    name: String,
    zone_id: Option<String>,       // Cloudflare zone ID
    status: Option<String>,        // zone status or lifecycle status
    expires_at: Option<String>,
    already_imported: bool,         // true if domain already exists in this org
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportResult {
    imported: u32,
    skipped: u32,
    errors: Vec<String>,
}

// ── Server functions ──────────────────────────────────────────────────

#[server]
async fn list_orgs_and_creds() -> Result<(Vec<OrgOption>, Vec<CredOption>), ServerFnError> {
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
        "SELECT id, name, credential_type FROM credentials ORDER BY name",
    )
    .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok((
        orgs.into_iter().map(|(id, name)| OrgOption { id, name }).collect(),
        creds.into_iter().map(|(id, name, credential_type)| CredOption { id, name, credential_type }).collect(),
    ))
}

/// Discover domains from a credential (Cloudflare zones or Spaceship domains).
#[server]
async fn discover_domains(credential_id: Uuid, org_id: Uuid) -> Result<Vec<DiscoveredDomain>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    if !user.is_admin && !user.org_ids().contains(&org_id) {
        return Err(ServerFnError::new("access denied"));
    }

    let cred = sqlx::query_as::<_, (String, Vec<u8>)>(
        "SELECT credential_type, encrypted_data FROM credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("credential not found"))?;

    let (cred_type, encrypted_data) = cred;
    let decrypted = crate::crypto::decrypt(&encrypted_data)
        .map_err(|e| ServerFnError::new(format!("decryption failed: {e}")))?;
    let data: serde_json::Value = serde_json::from_slice(&decrypted)
        .map_err(|e| ServerFnError::new(format!("invalid credential data: {e}")))?;

    // Load existing domains in this org for deduplication
    let existing: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM domains WHERE organization_id = $1",
    )
    .bind(org_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut discovered = Vec::new();

    match cred_type.as_str() {
        "cloudflare" => {
            let token = data["api_token"].as_str()
                .ok_or_else(|| ServerFnError::new("missing api_token"))?;
            let client = cloudflare_api::Client::new(token);

            let zones = client.list_zones(None).await
                .map_err(|e| ServerFnError::new(format!("Cloudflare API error: {e}")))?;

            for zone in zones {
                discovered.push(DiscoveredDomain {
                    already_imported: existing.contains(&zone.name),
                    name: zone.name,
                    zone_id: Some(zone.id),
                    status: Some(zone.status),
                    expires_at: None,
                });
            }
        }
        "spaceship" => {
            let api_key = data["api_key"].as_str()
                .ok_or_else(|| ServerFnError::new("missing api_key"))?;
            let api_secret = data["api_secret"].as_str()
                .ok_or_else(|| ServerFnError::new("missing api_secret"))?;
            let client = spaceship_api::Client::new(api_key, api_secret);

            let resp = client.list_domains(0, 500).await
                .map_err(|e| ServerFnError::new(format!("Spaceship API error: {e}")))?;

            for domain in resp.items {
                discovered.push(DiscoveredDomain {
                    already_imported: existing.contains(&domain.name),
                    name: domain.name,
                    zone_id: None,
                    status: domain.lifecycle_status,
                    expires_at: domain.expiration_date,
                });
            }
        }
        _ => return Err(ServerFnError::new("unsupported credential type")),
    }

    Ok(discovered)
}

/// Import selected domains into the organization.
#[server]
async fn import_domains(
    credential_id: Uuid,
    org_id: Uuid,
    domain_names: Vec<String>,
) -> Result<ImportResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    let cred = sqlx::query_as::<_, (String, Vec<u8>)>(
        "SELECT credential_type, encrypted_data FROM credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("credential not found"))?;

    let (cred_type, encrypted_data) = cred;
    let decrypted = crate::crypto::decrypt(&encrypted_data)
        .map_err(|e| ServerFnError::new(format!("decryption failed: {e}")))?;
    let data: serde_json::Value = serde_json::from_slice(&decrypted)
        .map_err(|e| ServerFnError::new(format!("invalid credential data: {e}")))?;

    // Build a map of name -> zone_id for Cloudflare
    let mut zone_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let registrar_type = cred_type.as_str();

    if registrar_type == "cloudflare" {
        let token = data["api_token"].as_str()
            .ok_or_else(|| ServerFnError::new("missing api_token"))?;
        let client = cloudflare_api::Client::new(token);
        let zones = client.list_zones(None).await
            .map_err(|e| ServerFnError::new(format!("Cloudflare API error: {e}")))?;
        for zone in zones {
            zone_map.insert(zone.name.clone(), zone.id);
        }
    }

    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();

    for domain_name in &domain_names {
        // Skip if already exists
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM domains WHERE organization_id = $1 AND name = $2)",
        )
        .bind(org_id)
        .bind(domain_name)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        if exists {
            skipped += 1;
            continue;
        }

        let zone_id = zone_map.get(domain_name).cloned();
        let cf_cred = if registrar_type == "cloudflare" { Some(credential_id) } else { None };
        let reg_cred = match registrar_type {
            "spaceship" => Some(credential_id),
            _ => None,
        };

        let result = sqlx::query(
            "INSERT INTO domains (organization_id, name, registrar_type, registrar_credential_id, \
             cloudflare_credential_id, cloudflare_zone_id) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(org_id)
        .bind(domain_name)
        .bind(registrar_type)
        .bind(reg_cred)
        .bind(cf_cred)
        .bind(&zone_id)
        .execute(&pool)
        .await;

        match result {
            Ok(_) => imported += 1,
            Err(e) => errors.push(format!("{domain_name}: {e}")),
        }
    }

    Ok(ImportResult { imported, skipped, errors })
}

// ── Component ─────────────────────────────────────────────────────────

#[component]
pub fn DomainImport() -> Element {
    let initial = use_server_future(list_orgs_and_creds)?;
    let (org_list, cred_list) = match &*initial.read() {
        Some(Ok((o, c))) => (o.clone(), c.clone()),
        _ => (vec![], vec![]),
    };

    let mut org_id = use_signal(|| org_list.first().map(|o| o.id.to_string()).unwrap_or_default());
    let mut cred_id = use_signal(String::new);
    let mut discovered: Signal<Vec<DiscoveredDomain>> = use_signal(Vec::new);
    let mut selected: Signal<Vec<String>> = use_signal(Vec::new);
    let mut loading = use_signal(|| false);
    let mut importing = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut result = use_signal(|| None::<ImportResult>);

    rsx! {
        PageHeader { "Import Domains" }

        // Step 1: Select credential and org
        Card {
            div { class: "p-6 space-y-4",
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Organization",
                        select {
                            class: "input",
                            value: "{org_id}",
                            oninput: move |evt| {
                                org_id.set(evt.value());
                                discovered.set(vec![]);
                                selected.set(vec![]);
                                result.set(None);
                            },
                            for org in &org_list {
                                option { value: "{org.id}", "{org.name}" }
                            }
                        }
                    }
                    FormField { label: "Credential",
                        select {
                            class: "input",
                            value: "{cred_id}",
                            oninput: move |evt| {
                                cred_id.set(evt.value());
                                discovered.set(vec![]);
                                selected.set(vec![]);
                                result.set(None);
                            },
                            option { value: "", "Select a credential..." }
                            for cred in &cred_list {
                                option { value: "{cred.id}", "{cred.name} ({cred.credential_type})" }
                            }
                        }
                    }
                }

                div { class: "flex gap-3",
                    Button {
                        variant: ButtonVariant::Secondary,
                        disabled: cred_id.read().is_empty() || *loading.read(),
                        onclick: {
                            let cred_str = cred_id.read().clone();
                            let org_str = org_id.read().clone();
                            move |_| {
                                let cred_str = cred_str.clone();
                                let org_str = org_str.clone();
                                loading.set(true);
                                error.set(None);
                                result.set(None);
                                spawn(async move {
                                    let cid = uuid::Uuid::parse_str(&cred_str).ok();
                                    let oid = uuid::Uuid::parse_str(&org_str).ok();
                                    if let (Some(cid), Some(oid)) = (cid, oid) {
                                        match discover_domains(cid, oid).await {
                                            Ok(domains) => {
                                                let new_names: Vec<String> = domains.iter()
                                                    .filter(|d| !d.already_imported)
                                                    .map(|d| d.name.clone())
                                                    .collect();
                                                selected.set(new_names);
                                                discovered.set(domains);
                                            }
                                            Err(e) => error.set(Some(format!("{e}"))),
                                        }
                                    }
                                    loading.set(false);
                                });
                            }
                        },
                        if *loading.read() { "Discovering..." } else { "Discover Domains" }
                    }
                }
            }
        }

        if let Some(err) = &*error.read() {
            div { class: "mt-4 text-danger", "{err}" }
        }

        // Step 2: Preview discovered domains
        if !discovered.read().is_empty() {
            h3 { class: "h-section mt-6",
                "Found {discovered.read().len()} domain(s)"
            }

            Card {
                div { class: "overflow-x-auto",
                    table { class: "table w-full",
                        thead {
                            tr {
                                Th {
                                    input {
                                        r#type: "checkbox",
                                        checked: {
                                            let sel = selected.read();
                                            let disc = discovered.read();
                                            let importable: Vec<_> = disc.iter().filter(|d| !d.already_imported).collect();
                                            !importable.is_empty() && importable.iter().all(|d| sel.contains(&d.name))
                                        },
                                        oninput: move |evt| {
                                            if evt.checked() {
                                                let names: Vec<String> = discovered.read().iter()
                                                    .filter(|d| !d.already_imported)
                                                    .map(|d| d.name.clone())
                                                    .collect();
                                                selected.set(names);
                                            } else {
                                                selected.set(vec![]);
                                            }
                                        },
                                    }
                                }
                                Th { "Domain" }
                                Th { "Status" }
                                Th { "Zone ID" }
                                Th { "Expires" }
                                Th { "State" }
                            }
                        }
                        tbody {
                            for domain in discovered.read().iter() {
                                {
                                    let name = domain.name.clone();
                                    let is_selected = selected.read().contains(&name);
                                    let already = domain.already_imported;
                                    rsx! {
                                        tr { class: if already { "opacity-50" } else { "" },
                                            Td {
                                                input {
                                                    r#type: "checkbox",
                                                    disabled: already,
                                                    checked: is_selected,
                                                    oninput: {
                                                        let name = name.clone();
                                                        move |evt| {
                                                            let mut sel = selected.write();
                                                            if evt.checked() {
                                                                if !sel.contains(&name) {
                                                                    sel.push(name.clone());
                                                                }
                                                            } else {
                                                                sel.retain(|n| n != &name);
                                                            }
                                                        }
                                                    },
                                                }
                                            }
                                            Td { "{domain.name}" }
                                            Td {
                                                match domain.status.as_deref() {
                                                    Some("active") | Some("registered") => rsx! { Badge { variant: BadgeVariant::Success, {domain.status.as_deref().unwrap_or("-")} } },
                                                    Some("pending") | Some("creating") => rsx! { Badge { variant: BadgeVariant::Warn, {domain.status.as_deref().unwrap_or("-")} } },
                                                    Some(s) => rsx! { Badge { "{s}" } },
                                                    None => rsx! { span { class: "text-fg-muted", "-" } },
                                                }
                                            }
                                            TdMuted { {domain.zone_id.as_deref().unwrap_or("-")} }
                                            TdMuted { {domain.expires_at.as_deref().unwrap_or("-")} }
                                            Td {
                                                if already {
                                                    Badge { variant: BadgeVariant::Neutral, "Already imported" }
                                                } else {
                                                    Badge { variant: BadgeVariant::Info, "New" }
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

            // Step 3: Import button
            div { class: "mt-4 flex items-center gap-4",
                Button {
                    variant: ButtonVariant::Primary,
                    disabled: selected.read().is_empty() || *importing.read(),
                    onclick: {
                        let cred_str = cred_id.read().clone();
                        let org_str = org_id.read().clone();
                        move |_| {
                            let cred_str = cred_str.clone();
                            let org_str = org_str.clone();
                            let names = selected.read().clone();
                            importing.set(true);
                            error.set(None);
                            spawn(async move {
                                let cid = uuid::Uuid::parse_str(&cred_str).ok();
                                let oid = uuid::Uuid::parse_str(&org_str).ok();
                                if let (Some(cid), Some(oid)) = (cid, oid) {
                                    match import_domains(cid, oid, names).await {
                                        Ok(r) => result.set(Some(r)),
                                        Err(e) => error.set(Some(format!("{e}"))),
                                    }
                                }
                                importing.set(false);
                            });
                        }
                    },
                    if *importing.read() {
                        "Importing..."
                    } else {
                        {format!("Import {} domain(s)", selected.read().len())}
                    }
                }

                Link {
                    to: crate::web::app::Route::DomainList {},
                    class: "btn btn-secondary",
                    "Back to Domains"
                }
            }
        }

        // Step 4: Results
        if let Some(res) = &*result.read() {
            Card {
                div { class: "p-6 mt-4",
                    h3 { class: "h-section", "Import Complete" }
                    div { class: "flex gap-6 mt-2",
                        div {
                            span { class: "text-fg-muted", "Imported: " }
                            span { class: "font-bold text-success", "{res.imported}" }
                        }
                        div {
                            span { class: "text-fg-muted", "Skipped: " }
                            span { class: "font-bold", "{res.skipped}" }
                        }
                        if !res.errors.is_empty() {
                            div {
                                span { class: "text-fg-muted", "Errors: " }
                                span { class: "font-bold text-danger", "{res.errors.len()}" }
                            }
                        }
                    }
                    if !res.errors.is_empty() {
                        ul { class: "mt-2 text-sm text-danger list-disc pl-4",
                            for err in &res.errors {
                                li { "{err}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
