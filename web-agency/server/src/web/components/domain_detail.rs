use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainData {
    id: Uuid,
    name: String,
    registrar_type: Option<String>,
    ssl_mode: String,
    dnssec_enabled: bool,
    cloudflare_zone_id: Option<String>,
    cloudflare_credential_id: Option<Uuid>,
    cloudflare_zone_status: Option<String>,
    cloudflare_nameservers: Vec<String>,
    registered_at: Option<String>,
    expires_at: Option<String>,
    organization_name: String,
    records: Vec<SubdomainRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubdomainRow {
    id: Uuid,
    name: String,
    record_type: String,
    record_value: String,
    proxied: bool,
    cloudflare_record_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CfCredOption {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeployResult {
    zone_id: String,
    status: String,
    nameservers: Vec<String>,
}

// ── Server functions ──────────────────────────────────────────────────

#[server]
async fn get_domain(domain_id: Uuid) -> Result<DomainData, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, String, bool, Option<String>, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>, Uuid)>(
        "SELECT d.id, d.name, d.registrar_type, d.ssl_mode, d.dnssec_enabled, d.cloudflare_zone_id, \
         d.cloudflare_credential_id, d.registered_at, d.expires_at, d.organization_id \
         FROM domains d WHERE d.id = $1",
    )
    .bind(domain_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("domain not found"))?;

    let (id, name, registrar_type, ssl_mode, dnssec_enabled, cloudflare_zone_id, cf_cred_id, registered_at, expires_at, org_id) = row;

    if !user.is_admin && !user.org_ids().contains(&org_id) {
        return Err(ServerFnError::new("access denied"));
    }

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // Fetch live zone info from Cloudflare if we have a zone_id and credential
    let mut cf_status = None;
    let mut cf_nameservers = Vec::new();

    if let (Some(zone_id), Some(cred_id)) = (&cloudflare_zone_id, cf_cred_id) {
        if let Ok(client) = build_cf_client(&pool, cred_id).await {
            if let Ok(zone) = client.get_zone(zone_id).await {
                cf_status = Some(zone.status);
                cf_nameservers = zone.name_servers.unwrap_or_default();
            }
        }
    }

    let records = sqlx::query_as::<_, (Uuid, String, String, String, bool, Option<String>)>(
        "SELECT id, name, record_type, record_value, proxied, cloudflare_record_id \
         FROM subdomains WHERE domain_id = $1 ORDER BY name, record_type",
    )
    .bind(domain_id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    .into_iter()
    .map(|(id, name, record_type, record_value, proxied, cloudflare_record_id)| SubdomainRow {
        id, name, record_type, record_value, proxied, cloudflare_record_id,
    })
    .collect();

    Ok(DomainData {
        id, name, registrar_type, ssl_mode, dnssec_enabled,
        cloudflare_zone_id, cloudflare_credential_id: cf_cred_id,
        cloudflare_zone_status: cf_status, cloudflare_nameservers: cf_nameservers,
        registered_at: registered_at.map(|d| d.format("%Y-%m-%d").to_string()),
        expires_at: expires_at.map(|d| d.format("%Y-%m-%d").to_string()),
        organization_name: org_name, records,
    })
}

#[server]
async fn list_cf_credentials() -> Result<Vec<CfCredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM credentials WHERE credential_type = 'cloudflare' ORDER BY name",
    )
    .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|(id, name)| CfCredOption { id, name }).collect())
}

/// Deploy a domain to Cloudflare: create or find the zone, store zone_id.
#[server]
async fn deploy_to_cloudflare(domain_id: Uuid, credential_id: Uuid) -> Result<DeployResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let domain = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT name, organization_id FROM domains WHERE id = $1",
    )
    .bind(domain_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("domain not found"))?;

    let (domain_name, org_id) = domain;
    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    let client = build_cf_client(&pool, credential_id).await?;
    let account_id = get_cf_account_id(&pool, credential_id).await?;

    // Check if zone already exists
    let existing = client.list_zones(Some(&domain_name)).await
        .map_err(|e| ServerFnError::new(format!("Cloudflare API error: {e}")))?;

    let zone = if let Some(zone) = existing.into_iter().find(|z| z.name == domain_name) {
        tracing::info!("domain {domain_name} already exists as zone {}", zone.id);
        zone
    } else {
        if account_id.is_empty() {
            return Err(ServerFnError::new(
                "account_id is required in the credential to create new zones"
            ));
        }
        let zone = client.create_zone(&domain_name, &account_id).await
            .map_err(|e| ServerFnError::new(format!("failed to create zone: {e}")))?;
        tracing::info!("created Cloudflare zone {} for {domain_name}", zone.id);
        zone
    };

    let zone_id = zone.id.clone();
    let status = zone.status.clone();
    let nameservers = zone.name_servers.clone().unwrap_or_default();

    // Update domain record
    sqlx::query(
        "UPDATE domains SET cloudflare_zone_id = $1, cloudflare_credential_id = $2, updated_at = now() WHERE id = $3",
    )
    .bind(&zone_id).bind(credential_id).bind(domain_id)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(DeployResult { zone_id, status, nameservers })
}

/// Update SSL mode on Cloudflare and in the database.
#[server]
async fn update_ssl_mode(domain_id: Uuid, ssl_mode: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (Option<String>, Option<Uuid>, Uuid)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id, organization_id FROM domains WHERE id = $1",
    )
    .bind(domain_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("domain not found"))?;

    let (zone_id, cred_id, org_id) = row;
    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    // Update on Cloudflare if deployed
    if let (Some(zone_id), Some(cred_id)) = (&zone_id, cred_id) {
        let client = build_cf_client(&pool, cred_id).await?;
        let value = if ssl_mode == "off" || ssl_mode == "flexible" || ssl_mode == "full" || ssl_mode == "strict" {
            "custom"
        } else {
            "auto"
        };
        client.set_ssl_mode(zone_id, value).await
            .map_err(|e| ServerFnError::new(format!("Cloudflare SSL error: {e}")))?;
    }

    sqlx::query("UPDATE domains SET ssl_mode = $1, updated_at = now() WHERE id = $2")
        .bind(&ssl_mode).bind(domain_id).execute(&pool).await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

/// Toggle DNSSEC on Cloudflare and in the database.
#[server]
async fn toggle_dnssec(domain_id: Uuid, enable: bool) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (Option<String>, Option<Uuid>, Uuid)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id, organization_id FROM domains WHERE id = $1",
    )
    .bind(domain_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("domain not found"))?;

    let (zone_id, cred_id, org_id) = row;
    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    if let (Some(zone_id), Some(cred_id)) = (&zone_id, cred_id) {
        let client = build_cf_client(&pool, cred_id).await?;
        let status = if enable { "active" } else { "disabled" };
        client.set_dnssec(zone_id, status).await
            .map_err(|e| ServerFnError::new(format!("Cloudflare DNSSEC error: {e}")))?;
    }

    sqlx::query("UPDATE domains SET dnssec_enabled = $1, updated_at = now() WHERE id = $2")
        .bind(enable).bind(domain_id).execute(&pool).await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────

#[cfg(feature = "server")]
async fn build_cf_client(pool: &sqlx::PgPool, cred_id: Uuid) -> Result<cloudflare_api::Client, ServerFnError> {
    let encrypted = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT encrypted_data FROM credentials WHERE id = $1 AND credential_type = 'cloudflare'",
    )
    .bind(cred_id).fetch_optional(pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Cloudflare credential not found"))?;

    let decrypted = crate::crypto::decrypt(&encrypted)
        .map_err(|e| ServerFnError::new(format!("decryption failed: {e}")))?;
    let data: serde_json::Value = serde_json::from_slice(&decrypted)
        .map_err(|e| ServerFnError::new(format!("invalid credential: {e}")))?;

    let token = data["api_token"].as_str()
        .ok_or_else(|| ServerFnError::new("missing api_token"))?;
    Ok(cloudflare_api::Client::new(token))
}

#[cfg(feature = "server")]
async fn get_cf_account_id(pool: &sqlx::PgPool, cred_id: Uuid) -> Result<String, ServerFnError> {
    let encrypted = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT encrypted_data FROM credentials WHERE id = $1",
    )
    .bind(cred_id).fetch_one(pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let decrypted = crate::crypto::decrypt(&encrypted)
        .map_err(|e| ServerFnError::new(format!("decryption failed: {e}")))?;
    let data: serde_json::Value = serde_json::from_slice(&decrypted)
        .map_err(|e| ServerFnError::new(format!("invalid credential: {e}")))?;

    Ok(data["account_id"].as_str().unwrap_or("").to_string())
}

// ── Component ─────────────────────────────────────────────────────────

#[component]
pub fn DomainDetail(id: String) -> Element {
    let domain_id = Uuid::parse_str(&id).ok();
    let domain = use_server_future(move || {
        let did = domain_id;
        async move {
            match did {
                Some(id) => get_domain(id).await,
                None => Err(ServerFnError::new("invalid domain ID")),
            }
        }
    })?;

    let data = match &*domain.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => {
            return rsx! { div { class: "text-danger", "Error: {e}" } };
        }
        None => {
            return rsx! { div { class: "text-fg-muted", "Loading..." } };
        }
    };

    let has_cf = data.cloudflare_zone_id.is_some();

    rsx! {
        PageHeader { "{data.name}" }

        // Domain info card
        Card {
            div { class: "p-6 grid grid-cols-2 md:grid-cols-4 gap-4",
                div {
                    div { class: "text-sm text-fg-muted", "Registrar" }
                    div { class: "font-medium", {data.registrar_type.as_deref().unwrap_or("External")} }
                }
                div {
                    div { class: "text-sm text-fg-muted", "SSL Mode" }
                    Badge { "{data.ssl_mode}" }
                }
                div {
                    div { class: "text-sm text-fg-muted", "DNSSEC" }
                    if data.dnssec_enabled {
                        Badge { variant: BadgeVariant::Success, "Enabled" }
                    } else {
                        span { class: "text-fg-muted", "Disabled" }
                    }
                }
                div {
                    div { class: "text-sm text-fg-muted", "Expires" }
                    div { {data.expires_at.as_deref().unwrap_or("-")} }
                }
                div {
                    div { class: "text-sm text-fg-muted", "Organization" }
                    div { "{data.organization_name}" }
                }
            }
        }

        // Cloudflare deployment section
        SectionHeading { class: "mt-6", "Cloudflare" }
        if has_cf {
            CloudflareDeployed {
                domain_id: data.id,
                zone_id: data.cloudflare_zone_id.clone().unwrap_or_default(),
                zone_status: data.cloudflare_zone_status.clone(),
                nameservers: data.cloudflare_nameservers.clone(),
                ssl_mode: data.ssl_mode.clone(),
                dnssec_enabled: data.dnssec_enabled,
            }
        } else {
            CloudflareDeployForm { domain_id: data.id, domain_name: data.name.clone() }
        }

        // DNS Records
        SectionHeading { class: "mt-6", "DNS Records" }
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Name" }
                            Th { "Type" }
                            Th { "Value" }
                            Th { "Proxied" }
                            Th { "CF Record" }
                        }
                    }
                    tbody {
                        if data.records.is_empty() {
                            tr {
                                td { class: "td text-fg-muted text-center", colspan: "5", "No DNS records" }
                            }
                        }
                        for rec in &data.records {
                            tr {
                                Td { "{rec.name}" }
                                Td { Badge { "{rec.record_type}" } }
                                Td { class: "font-mono text-sm", "{rec.record_value}" }
                                Td {
                                    if rec.proxied {
                                        Badge { variant: BadgeVariant::Success, "Yes" }
                                    } else {
                                        span { class: "text-fg-muted", "No" }
                                    }
                                }
                                Td {
                                    if rec.cloudflare_record_id.is_some() {
                                        Badge { variant: BadgeVariant::Info, "Synced" }
                                    } else {
                                        span { class: "text-fg-muted", "-" }
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

/// Shown when the domain is NOT yet deployed to Cloudflare.
#[component]
fn CloudflareDeployForm(domain_id: Uuid, domain_name: String) -> Element {
    let creds = use_server_future(list_cf_credentials)?;
    let cred_list = match &*creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    let mut cred_id = use_signal(|| cred_list.first().map(|c| c.id.to_string()).unwrap_or_default());
    let mut deploying = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut result = use_signal(|| None::<DeployResult>);

    if let Some(res) = &*result.read() {
        return rsx! {
            Card {
                div { class: "p-6",
                    div { class: "flex items-center gap-2 mb-3",
                        Badge { variant: BadgeVariant::Success, "Deployed" }
                        span { class: "font-mono text-sm", "Zone: {res.zone_id}" }
                    }
                    div { class: "mb-2",
                        span { class: "text-sm text-fg-muted", "Status: " }
                        match res.status.as_str() {
                            "active" => rsx! { Badge { variant: BadgeVariant::Success, "Active" } },
                            "pending" => rsx! { Badge { variant: BadgeVariant::Warn, "Pending" } },
                            s => rsx! { Badge { "{s}" } },
                        }
                    }
                    if !res.nameservers.is_empty() {
                        div { class: "mt-3",
                            div { class: "text-sm text-fg-muted mb-1", "Point your domain to these nameservers:" }
                            for ns in &res.nameservers {
                                div { class: "font-mono text-sm bg-surface-2 px-3 py-1 rounded mb-1", "{ns}" }
                            }
                        }
                    }
                    div { class: "mt-3 text-sm text-fg-muted",
                        "Reload the page to see updated zone status."
                    }
                }
            }
        };
    }

    rsx! {
        Card {
            div { class: "p-6",
                p { class: "text-fg-muted mb-4",
                    "Deploy " span { class: "font-semibold text-fg", "{domain_name}" }
                    " to Cloudflare to manage DNS, SSL, and DNSSEC."
                }

                if cred_list.is_empty() {
                    p { class: "text-fg-muted",
                        "No Cloudflare credentials available. "
                        Link { to: crate::web::app::Route::CredentialForm {}, class: "text-brand underline", "Add one" }
                        " first."
                    }
                } else {
                    div { class: "flex items-end gap-3",
                        FormField { label: "Cloudflare Credential",
                            select {
                                class: "input",
                                value: "{cred_id}",
                                oninput: move |evt| cred_id.set(evt.value()),
                                for c in &cred_list {
                                    option { value: "{c.id}", "{c.name}" }
                                }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            disabled: *deploying.read(),
                            onclick: {
                                let did = domain_id;
                                let cid_str = cred_id.read().clone();
                                move |_| {
                                    let cid_str = cid_str.clone();
                                    deploying.set(true);
                                    error.set(None);
                                    spawn(async move {
                                        if let Ok(cid) = uuid::Uuid::parse_str(&cid_str) {
                                            match deploy_to_cloudflare(did, cid).await {
                                                Ok(r) => result.set(Some(r)),
                                                Err(e) => error.set(Some(format!("{e}"))),
                                            }
                                        }
                                        deploying.set(false);
                                    });
                                }
                            },
                            if *deploying.read() { "Deploying..." } else { "Deploy to Cloudflare" }
                        }
                    }
                }

                if let Some(err) = &*error.read() {
                    div { class: "mt-3 text-danger text-sm", "{err}" }
                }
            }
        }
    }
}

/// Shown when the domain IS deployed to Cloudflare.
#[component]
fn CloudflareDeployed(
    domain_id: Uuid,
    zone_id: String,
    zone_status: Option<String>,
    nameservers: Vec<String>,
    ssl_mode: String,
    dnssec_enabled: bool,
) -> Element {
    let mut ssl = use_signal(move || ssl_mode.clone());
    let mut dnssec = use_signal(move || dnssec_enabled);
    let mut saving_ssl = use_signal(|| false);
    let mut saving_dnssec = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);

    rsx! {
        Card {
            div { class: "p-6 space-y-4",
                // Zone info
                div { class: "flex items-center gap-3 flex-wrap",
                    span { class: "text-sm text-fg-muted", "Zone:" }
                    span { class: "font-mono text-sm", "{zone_id}" }
                    match zone_status.as_deref() {
                        Some("active") => rsx! { Badge { variant: BadgeVariant::Success, "Active" } },
                        Some("pending") => rsx! { Badge { variant: BadgeVariant::Warn, "Pending - update nameservers" } },
                        Some(s) => rsx! { Badge { "{s}" } },
                        None => rsx! {},
                    }
                }

                // Nameservers
                if !nameservers.is_empty() {
                    div {
                        div { class: "text-sm text-fg-muted mb-1", "Nameservers:" }
                        div { class: "flex gap-2 flex-wrap",
                            for ns in &nameservers {
                                span { class: "font-mono text-sm bg-surface-2 px-3 py-1 rounded", "{ns}" }
                            }
                        }
                    }
                }

                // SSL mode
                div { class: "flex items-end gap-3",
                    FormField { label: "SSL Mode",
                        select {
                            class: "input w-40",
                            value: "{ssl}",
                            oninput: move |evt| ssl.set(evt.value()),
                            option { value: "off", "Off" }
                            option { value: "flexible", "Flexible" }
                            option { value: "full", "Full" }
                            option { value: "strict", "Strict" }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Secondary,
                        disabled: *saving_ssl.read(),
                        onclick: {
                            let did = domain_id;
                            move |_| {
                                let mode = ssl.read().clone();
                                saving_ssl.set(true);
                                message.set(None);
                                spawn(async move {
                                    match update_ssl_mode(did, mode).await {
                                        Ok(()) => message.set(Some("SSL mode updated".into())),
                                        Err(e) => message.set(Some(format!("Error: {e}"))),
                                    }
                                    saving_ssl.set(false);
                                });
                            }
                        },
                        if *saving_ssl.read() { "Saving..." } else { "Update SSL" }
                    }
                }

                // DNSSEC toggle
                div { class: "flex items-center gap-3",
                    span { class: "text-sm", "DNSSEC:" }
                    Button {
                        variant: if *dnssec.read() { ButtonVariant::Danger } else { ButtonVariant::Primary },
                        disabled: *saving_dnssec.read(),
                        onclick: {
                            let did = domain_id;
                            move |_| {
                                let new_state = !*dnssec.read();
                                saving_dnssec.set(true);
                                message.set(None);
                                spawn(async move {
                                    match toggle_dnssec(did, new_state).await {
                                        Ok(()) => {
                                            dnssec.set(new_state);
                                            message.set(Some(if new_state { "DNSSEC enabled".into() } else { "DNSSEC disabled".into() }));
                                        }
                                        Err(e) => message.set(Some(format!("Error: {e}"))),
                                    }
                                    saving_dnssec.set(false);
                                });
                            }
                        },
                        if *saving_dnssec.read() {
                            "Saving..."
                        } else if *dnssec.read() {
                            "Disable DNSSEC"
                        } else {
                            "Enable DNSSEC"
                        }
                    }
                    if *dnssec.read() {
                        Badge { variant: BadgeVariant::Success, "Enabled" }
                    }
                }

                if let Some(msg) = &*message.read() {
                    div { class: "text-sm text-fg-muted", "{msg}" }
                }
            }
        }
    }
}
