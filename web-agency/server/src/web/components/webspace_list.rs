use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Card, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebspaceRow {
    id: Uuid,
    name: String,
    hosting_type: String,
    runtime: Option<String>,
    local_status: Option<String>,
    cloudflare_pages_project: Option<String>,
    relay_url: Option<String>,
    auth_mode: String,
    domain_count: i64,
    organization_name: String,
}

#[server]
async fn list_webspaces() -> Result<Vec<WebspaceRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_ids = user.org_ids();
    if org_ids.is_empty() && !user.is_admin {
        return Ok(vec![]);
    }

    type Row = (Uuid, String, String, Option<String>, Option<String>, Option<String>, Option<String>, String, i64, String);
    let rows = if user.is_admin {
        sqlx::query_as::<_, Row>(
            "SELECT w.id, w.name, w.hosting_type, w.runtime, w.local_status, w.cloudflare_pages_project, w.relay_url, w.auth_mode, \
             (SELECT count(*) FROM webspace_domains wd WHERE wd.webspace_id = w.id), o.name \
             FROM webspaces w JOIN organizations o ON o.id = w.organization_id \
             ORDER BY w.name",
        )
        .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, Row>(
            "SELECT w.id, w.name, w.hosting_type, w.runtime, w.local_status, w.cloudflare_pages_project, w.relay_url, w.auth_mode, \
             (SELECT count(*) FROM webspace_domains wd WHERE wd.webspace_id = w.id), o.name \
             FROM webspaces w JOIN organizations o ON o.id = w.organization_id \
             WHERE w.organization_id = ANY($1) \
             ORDER BY w.name",
        )
        .bind(&org_ids)
        .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows
        .into_iter()
        .map(|(id, name, hosting_type, runtime, local_status, cloudflare_pages_project, relay_url, auth_mode, domain_count, organization_name)| {
            WebspaceRow { id, name, hosting_type, runtime, local_status, cloudflare_pages_project, relay_url, auth_mode, domain_count, organization_name }
        })
        .collect())
}

#[component]
pub fn WebspaceList() -> Element {
    let webspaces = use_server_future(list_webspaces)?;
    let rows = webspaces.read();
    let rows = match &*rows {
        Some(Ok(r)) => r.as_slice(),
        _ => &[],
    };

    rsx! {
        PageHeader { "Webspaces" }

        div { class: "mt-4 flex justify-end gap-3 mb-4",
            Link {
                to: crate::web::app::Route::WebspaceImport {},
                class: "btn btn-secondary",
                "Import from Credential"
            }
            Link {
                to: crate::web::app::Route::WebspaceForm {},
                class: "btn btn-primary",
                "Create Webspace"
            }
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Name" }
                            Th { "Type" }
                            Th { "Target" }
                            Th { "Status" }
                            Th { "Auth" }
                            Th { "Domains" }
                            Th { "Org" }
                        }
                    }
                    tbody {
                        if rows.is_empty() {
                            tr {
                                td { class: "td text-fg-muted text-center", colspan: "7", "No webspaces yet" }
                            }
                        }
                        for row in rows {
                            tr { class: "cursor-pointer hover:bg-surface-2",
                                onclick: {
                                    let id = row.id.to_string();
                                    move |_| {
                                        navigator().push(crate::web::app::Route::WebspaceDetail { id: id.clone() });
                                    }
                                },
                                Td { "{row.name}" }
                                Td {
                                    match row.hosting_type.as_str() {
                                        "cloudflare_pages" => rsx! { Badge { variant: BadgeVariant::Info, "CF Pages" } },
                                        "local" => rsx! { Badge { "Local" } },
                                        "relay" => rsx! { Badge { variant: BadgeVariant::Accent, "Relay" } },
                                        "tunnel" => rsx! { Badge { variant: BadgeVariant::Accent, "Tunnel" } },
                                        _ => rsx! { span { "-" } },
                                    }
                                }
                                Td {
                                    if let Some(ref proj) = row.cloudflare_pages_project {
                                        span { class: "font-mono text-sm", "{proj}" }
                                    } else if let Some(ref url) = row.relay_url {
                                        span { class: "font-mono text-sm truncate max-w-xs", title: "{url}", "{url}" }
                                    } else if let Some(ref rt) = row.runtime {
                                        Badge { "{rt}" }
                                    } else {
                                        span { class: "text-fg-muted", "-" }
                                    }
                                }
                                Td {
                                    match (row.hosting_type.as_str(), row.local_status.as_deref()) {
                                        ("local", Some("running")) => rsx! { Badge { variant: BadgeVariant::Success, "Running" } },
                                        ("local", Some("error")) => rsx! { Badge { variant: BadgeVariant::Danger, "Error" } },
                                        ("local", Some("starting")) => rsx! { Badge { variant: BadgeVariant::Warn, "Starting" } },
                                        ("local", Some("stopped")) => rsx! { span { class: "text-fg-muted", "Stopped" } },
                                        _ => rsx! { span { class: "text-fg-muted", "-" } },
                                    }
                                }
                                Td {
                                    match row.auth_mode.as_str() {
                                        "oidc" => rsx! { Badge { variant: BadgeVariant::Info, "OIDC" } },
                                        "basic" => rsx! { Badge { variant: BadgeVariant::Accent, "Basic" } },
                                        _ => rsx! { span { class: "text-fg-muted", "-" } },
                                    }
                                }
                                Td { "{row.domain_count}" }
                                TdMuted { "{row.organization_name}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
