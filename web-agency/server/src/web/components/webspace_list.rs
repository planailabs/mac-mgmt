//! Flat cross-host folder list. Each row is a `webspace` folder; domains,
//! CNAME, and ChangeDetection are managed on the parent host.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, ButtonVariant, Button, Card, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebspaceRow {
    id: Uuid,
    name: String,
    host_id: Uuid,
    host_name: String,
    host_kind: String,
    path_prefix: String,
    hosting_type: String,
    runtime: Option<String>,
    local_status: Option<String>,
    auth_mode: String,
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

    type Row = (
        Uuid,
        String,
        Uuid,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        String,
    );
    let query = "SELECT w.id, w.name, h.id, h.name, h.kind, w.path_prefix, w.hosting_type, w.runtime, w.local_status, w.auth_mode, o.name \
                 FROM webspaces w \
                 JOIN webspace_hosts h ON h.id = w.webspace_host_id \
                 JOIN organizations o ON o.id = w.organization_id";
    let rows = if user.is_admin {
        sqlx::query_as::<_, Row>(&format!("{query} ORDER BY h.name, w.path_prefix"))
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, Row>(&format!(
            "{query} WHERE w.organization_id = ANY($1) ORDER BY h.name, w.path_prefix"
        ))
        .bind(&org_ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                name,
                host_id,
                host_name,
                host_kind,
                path_prefix,
                hosting_type,
                runtime,
                local_status,
                auth_mode,
                organization_name,
            )| WebspaceRow {
                id,
                name,
                host_id,
                host_name,
                host_kind,
                path_prefix,
                hosting_type,
                runtime,
                local_status,
                auth_mode,
                organization_name,
            },
        )
        .collect())
}

#[component]
pub fn WebspaceList() -> Element {
    let webspaces = use_server_future(list_webspaces)?;
    let all_rows: Vec<WebspaceRow> = match &*webspaces.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };

    let mut filter = use_signal(|| "all".to_string());
    let filtered_rows: Vec<&WebspaceRow> = match filter.read().as_str() {
        "pages" => all_rows.iter().filter(|r| r.hosting_type == "cloudflare_pages").collect(),
        "local" => all_rows.iter().filter(|r| r.hosting_type == "local").collect(),
        "relay" => all_rows.iter().filter(|r| r.hosting_type == "relay").collect(),
        "tunnel" => all_rows.iter().filter(|r| r.hosting_type == "tunnel").collect(),
        _ => all_rows.iter().collect(),
    };

    rsx! {
        PageHeader { "Webspaces (folders)" }

        div { class: "mt-4 flex justify-end gap-3 mb-4",
            Link { to: crate::web::app::Route::WebspaceImport {}, class: "btn btn-secondary", "Import from Credential" }
            Link { to: crate::web::app::Route::WebspaceHostList {}, class: "btn btn-primary", "Manage Hosts" }
        }

        div { class: "mb-4 flex items-center gap-2 flex-wrap",
            span { class: "text-sm text-fg-muted", "Filter:" }
            {
                let filters: Vec<(&str, &str, usize)> = vec![
                    ("all", "All", all_rows.len()),
                    ("pages", "Pages", all_rows.iter().filter(|r| r.hosting_type == "cloudflare_pages").count()),
                    ("local", "Local", all_rows.iter().filter(|r| r.hosting_type == "local").count()),
                    ("relay", "Relay", all_rows.iter().filter(|r| r.hosting_type == "relay").count()),
                    ("tunnel", "Tunnel", all_rows.iter().filter(|r| r.hosting_type == "tunnel").count()),
                ];
                rsx! {
                    for (key, label, count) in filters {
                        Button {
                            variant: if *filter.read() == key { ButtonVariant::Primary } else { ButtonVariant::Secondary },
                            onclick: { let k = key.to_string(); move |_| filter.set(k.clone()) },
                            "{label} ({count})"
                        }
                    }
                }
            }
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Host" }
                            Th { "Path" }
                            Th { "Name" }
                            Th { "Type" }
                            Th { "Status" }
                            Th { "Auth" }
                            Th { "Org" }
                        }
                    }
                    tbody {
                        if filtered_rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "7", "No folders yet" } }
                        }
                        for row in &filtered_rows {
                            {
                                let fid = row.id.to_string();
                                let hid = row.host_id.to_string();
                                rsx! {
                                    tr { class: "hover:bg-surface-2",
                                        Td {
                                            Link {
                                                to: crate::web::app::Route::WebspaceHostDetail { id: hid.clone() },
                                                class: "text-brand underline",
                                                "{row.host_name}"
                                            }
                                        }
                                        Td { class: "font-mono", "{row.path_prefix}" }
                                        td { class: "td cursor-pointer",
                                            onclick: move |_| { navigator().push(crate::web::app::Route::WebspaceDetail { id: fid.clone() }); },
                                            "{row.name}"
                                        }
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
                                            match (row.hosting_type.as_str(), row.local_status.as_deref()) {
                                                ("local", Some("running")) => rsx! { Badge { variant: BadgeVariant::Success, "Running" } },
                                                ("local", Some("error")) => rsx! { Badge { variant: BadgeVariant::Danger, "Error" } },
                                                ("local", Some("starting")) => rsx! { Badge { variant: BadgeVariant::Warn, "Starting" } },
                                                ("local", Some(_)) => rsx! { span { class: "text-fg-muted", "Stopped" } },
                                                _ => {
                                                    if let Some(ref rt) = row.runtime {
                                                        rsx! { Badge { "{rt}" } }
                                                    } else {
                                                        rsx! { span { class: "text-fg-muted", "-" } }
                                                    }
                                                }
                                            }
                                        }
                                        Td {
                                            match row.auth_mode.as_str() {
                                                "oidc" => rsx! { Badge { variant: BadgeVariant::Info, "OIDC" } },
                                                "basic" => rsx! { Badge { variant: BadgeVariant::Accent, "Basic" } },
                                                _ => rsx! { span { class: "text-fg-muted", "-" } },
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
