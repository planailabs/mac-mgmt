//! Webspace-host list page.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Card, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HostRow {
    id: Uuid,
    name: String,
    kind: String,
    organization_name: String,
    folder_count: i64,
    hostname: Option<String>,
    has_changedetection: bool,
}

#[server]
async fn list_hosts() -> Result<Vec<HostRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_ids = user.org_ids();
    if org_ids.is_empty() && !user.is_admin {
        return Ok(vec![]);
    }

    // hostname: first bound (domain, subdomain) for the host, rendered as a FQDN.
    let hostname_subquery = "(SELECT CASE WHEN s.name IS NOT NULL AND s.name != '@' \
            THEN s.name || '.' || d.name ELSE d.name END \
         FROM webspace_host_domains whd \
         JOIN domains d ON d.id = whd.domain_id \
         LEFT JOIN subdomains s ON s.id = whd.subdomain_id \
         WHERE whd.webspace_host_id = h.id ORDER BY d.name LIMIT 1)";

    type Row = (Uuid, String, String, String, i64, Option<String>, bool);
    let query = format!(
        "SELECT h.id, h.name, h.kind, o.name, \
         (SELECT count(*) FROM webspaces w WHERE w.webspace_host_id = h.id), \
         {hostname_subquery}, \
         h.changedetection_credential_id IS NOT NULL \
         FROM webspace_hosts h JOIN organizations o ON o.id = h.organization_id"
    );

    let rows = if user.is_admin {
        sqlx::query_as::<_, Row>(&format!("{query} ORDER BY h.name"))
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, Row>(&format!(
            "{query} WHERE h.organization_id = ANY($1) ORDER BY h.name"
        ))
        .bind(&org_ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows
        .into_iter()
        .map(
            |(id, name, kind, organization_name, folder_count, hostname, has_changedetection)| {
                HostRow {
                    id,
                    name,
                    kind,
                    organization_name,
                    folder_count,
                    hostname,
                    has_changedetection,
                }
            },
        )
        .collect())
}

#[component]
pub fn WebspaceHostList() -> Element {
    let hosts = use_server_future(list_hosts)?;
    let rows: Vec<HostRow> = match &*hosts.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };

    rsx! {
        PageHeader { "Webspace Hosts" }

        div { class: "mt-4 flex justify-end mb-4",
            Link {
                to: crate::web::app::Route::WebspaceHostForm {},
                class: "btn btn-primary",
                "Create Host"
            }
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Name" }
                            Th { "Kind" }
                            Th { "Hostname" }
                            Th { "Folders" }
                            Th { "CD" }
                            Th { "Org" }
                        }
                    }
                    tbody {
                        if rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "6", "No hosts yet" } }
                        }
                        for row in &rows {
                            {
                                let id = row.id.to_string();
                                rsx! {
                                    tr { class: "cursor-pointer hover:bg-surface-2",
                                        onclick: move |_| { navigator().push(crate::web::app::Route::WebspaceHostDetail { id: id.clone() }); },
                                        Td { "{row.name}" }
                                        Td {
                                            if row.kind == "cloudflare" {
                                                Badge { variant: BadgeVariant::Info, "Cloudflare" }
                                            } else {
                                                Badge { variant: BadgeVariant::Accent, "Proxy" }
                                            }
                                        }
                                        Td {
                                            if let Some(ref h) = row.hostname {
                                                span { class: "font-mono text-sm", "{h}" }
                                            } else {
                                                span { class: "text-fg-muted", "-" }
                                            }
                                        }
                                        Td { "{row.folder_count}" }
                                        Td {
                                            if row.has_changedetection {
                                                Badge { variant: BadgeVariant::Success, "Yes" }
                                            } else {
                                                span { class: "text-fg-muted", "-" }
                                            }
                                        }
                                        TdMuted { "{row.organization_name}" }
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
