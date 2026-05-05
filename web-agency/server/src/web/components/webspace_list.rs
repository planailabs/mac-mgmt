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

    let rows = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<String>, Option<String>, i64, String)>(
            "SELECT w.id, w.name, w.hosting_type, w.runtime, w.local_status, w.cloudflare_pages_project, \
             (SELECT count(*) FROM webspace_domains wd WHERE wd.webspace_id = w.id), o.name \
             FROM webspaces w JOIN organizations o ON o.id = w.organization_id \
             ORDER BY w.name",
        )
        .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<String>, Option<String>, i64, String)>(
            "SELECT w.id, w.name, w.hosting_type, w.runtime, w.local_status, w.cloudflare_pages_project, \
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
        .map(|(id, name, hosting_type, runtime, local_status, cloudflare_pages_project, domain_count, organization_name)| {
            WebspaceRow { id, name, hosting_type, runtime, local_status, cloudflare_pages_project, domain_count, organization_name }
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

        div { class: "mt-4 flex justify-end mb-4",
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
                            Th { "Runtime / Pages" }
                            Th { "Status" }
                            Th { "Domains" }
                            Th { "Org" }
                        }
                    }
                    tbody {
                        if rows.is_empty() {
                            tr {
                                td { class: "td text-fg-muted text-center", colspan: "6", "No webspaces yet" }
                            }
                        }
                        for row in rows {
                            tr {
                                Td { "{row.name}" }
                                Td {
                                    match row.hosting_type.as_str() {
                                        "cloudflare_pages" => rsx! { Badge { variant: BadgeVariant::Info, "CF Pages" } },
                                        "local" => rsx! { Badge { "Local" } },
                                        _ => rsx! { span { "-" } },
                                    }
                                }
                                Td {
                                    if let Some(ref proj) = row.cloudflare_pages_project {
                                        span { class: "font-mono text-sm", "{proj}" }
                                    } else if let Some(ref rt) = row.runtime {
                                        Badge { "{rt}" }
                                    } else {
                                        span { class: "text-fg-muted", "-" }
                                    }
                                }
                                Td {
                                    match row.local_status.as_deref() {
                                        Some("running") => rsx! { Badge { variant: BadgeVariant::Success, "Running" } },
                                        Some("error") => rsx! { Badge { variant: BadgeVariant::Danger, "Error" } },
                                        Some("starting") => rsx! { Badge { variant: BadgeVariant::Warn, "Starting" } },
                                        Some("stopped") => rsx! { span { class: "text-fg-muted", "Stopped" } },
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
