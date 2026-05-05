use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Card, PageHeader, Td, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainData {
    id: Uuid,
    name: String,
    registrar_type: Option<String>,
    ssl_mode: String,
    dnssec_enabled: bool,
    cloudflare_zone_id: Option<String>,
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

#[server]
async fn get_domain(domain_id: Uuid) -> Result<DomainData, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, String, bool, Option<String>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>, Uuid)>(
        "SELECT d.id, d.name, d.registrar_type, d.ssl_mode, d.dnssec_enabled, d.cloudflare_zone_id, d.registered_at, d.expires_at, d.organization_id \
         FROM domains d WHERE d.id = $1",
    )
    .bind(domain_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("domain not found"))?;

    let (id, name, registrar_type, ssl_mode, dnssec_enabled, cloudflare_zone_id, registered_at, expires_at, org_id) = row;

    if !user.is_admin && !user.org_ids().contains(&org_id) {
        return Err(ServerFnError::new("access denied"));
    }

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let records = sqlx::query_as::<_, (Uuid, String, String, String, bool, Option<String>)>(
        "SELECT id, name, record_type, record_value, proxied, cloudflare_record_id \
         FROM subdomains WHERE domain_id = $1 ORDER BY name, record_type",
    )
    .bind(domain_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .into_iter()
    .map(|(id, name, record_type, record_value, proxied, cloudflare_record_id)| SubdomainRow {
        id,
        name,
        record_type,
        record_value,
        proxied,
        cloudflare_record_id,
    })
    .collect();

    Ok(DomainData {
        id,
        name,
        registrar_type,
        ssl_mode,
        dnssec_enabled,
        cloudflare_zone_id,
        registered_at: registered_at.map(|d| d.format("%Y-%m-%d").to_string()),
        expires_at: expires_at.map(|d| d.format("%Y-%m-%d").to_string()),
        organization_name: org_name,
        records,
    })
}

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
            return rsx! { div { class: "text-red-400", "Error: {e}" } };
        }
        None => {
            return rsx! { div { class: "text-fg-muted", "Loading..." } };
        }
    };

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
                    div { class: "text-sm text-fg-muted", "Cloudflare Zone" }
                    if let Some(ref zid) = data.cloudflare_zone_id {
                        span { class: "font-mono text-sm", "{zid}" }
                    } else {
                        span { class: "text-fg-muted", "Not configured" }
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

        // DNS Records
        h3 { class: "h-section mt-6", "DNS Records" }
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
