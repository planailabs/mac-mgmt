use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Card, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainRow {
    id: Uuid,
    name: String,
    registrar_type: Option<String>,
    ssl_mode: String,
    dnssec_enabled: bool,
    cloudflare_zone_id: Option<String>,
    expires_at: Option<String>,
    organization_name: String,
}

#[server]
async fn list_domains() -> Result<Vec<DomainRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_ids = user.org_ids();
    if org_ids.is_empty() && !user.is_admin {
        return Ok(vec![]);
    }

    let rows = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String, Option<String>, String, bool, Option<String>, Option<chrono::DateTime<chrono::Utc>>, String)>(
            "SELECT d.id, d.name, d.registrar_type, d.ssl_mode, d.dnssec_enabled, d.cloudflare_zone_id, d.expires_at, o.name \
             FROM domains d JOIN organizations o ON o.id = d.organization_id \
             ORDER BY d.name",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, (Uuid, String, Option<String>, String, bool, Option<String>, Option<chrono::DateTime<chrono::Utc>>, String)>(
            "SELECT d.id, d.name, d.registrar_type, d.ssl_mode, d.dnssec_enabled, d.cloudflare_zone_id, d.expires_at, o.name \
             FROM domains d JOIN organizations o ON o.id = d.organization_id \
             WHERE d.organization_id = ANY($1) \
             ORDER BY d.name",
        )
        .bind(&org_ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows
        .into_iter()
        .map(|(id, name, registrar_type, ssl_mode, dnssec_enabled, cloudflare_zone_id, expires_at, organization_name)| DomainRow {
            id,
            name,
            registrar_type,
            ssl_mode,
            dnssec_enabled,
            cloudflare_zone_id,
            expires_at: expires_at.map(|d| d.format("%Y-%m-%d").to_string()),
            organization_name,
        })
        .collect())
}

#[component]
pub fn DomainList() -> Element {
    let domains = use_server_future(list_domains)?;
    let rows = domains.read();
    let rows = match &*rows {
        Some(Ok(r)) => r.as_slice(),
        _ => &[],
    };

    rsx! {
        PageHeader { "Domains" }

        div { class: "mt-4 flex justify-end mb-4",
            Link {
                to: crate::web::app::Route::DomainAdd {},
                class: "btn btn-primary",
                "Add Domain"
            }
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Domain" }
                            Th { "Registrar" }
                            Th { "SSL" }
                            Th { "DNSSEC" }
                            Th { "CF Zone" }
                            Th { "Expires" }
                            Th { "Org" }
                        }
                    }
                    tbody {
                        if rows.is_empty() {
                            tr {
                                td { class: "td text-fg-muted text-center", colspan: "7", "No domains yet" }
                            }
                        }
                        for row in rows {
                            tr { class: "cursor-pointer hover:bg-surface-2",
                                onclick: {
                                    let id = row.id.to_string();
                                    move |_| {
                                        let nav = navigator();
                                        nav.push(crate::web::app::Route::DomainDetail { id: id.clone() });
                                    }
                                },
                                Td { "{row.name}" }
                                Td {
                                    match row.registrar_type.as_deref() {
                                        Some("cloudflare") => rsx! { Badge { variant: BadgeVariant::Info, "CF" } },
                                        Some("spaceship") => rsx! { Badge { variant: BadgeVariant::Info, "SS" } },
                                        Some("external") => rsx! { Badge { "Ext" } },
                                        _ => rsx! { span { class: "text-fg-muted", "-" } },
                                    }
                                }
                                Td {
                                    Badge { "{row.ssl_mode}" }
                                }
                                Td {
                                    if row.dnssec_enabled {
                                        Badge { variant: BadgeVariant::Success, "On" }
                                    } else {
                                        span { class: "text-fg-muted", "Off" }
                                    }
                                }
                                Td {
                                    if row.cloudflare_zone_id.is_some() {
                                        Badge { variant: BadgeVariant::Success, "Yes" }
                                    } else {
                                        span { class: "text-fg-muted", "-" }
                                    }
                                }
                                TdMuted { {row.expires_at.as_deref().unwrap_or("-")} }
                                TdMuted { "{row.organization_name}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
