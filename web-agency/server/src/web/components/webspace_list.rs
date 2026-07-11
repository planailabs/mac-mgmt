//! Flat cross-host folder list. Each row is a `webspace` folder; domains,
//! CNAME, and ChangeDetection are managed on the parent host.

use dioxus::prelude::*;

use super::ui::{Badge, BadgeVariant, Button, ButtonVariant, Card, PageHeader, Td, TdMuted, Th};

// WebspaceRow + the list endpoint now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::webspaces::{WebspaceListInput, WebspaceRow, list_webspaces};

#[component]
pub fn WebspaceList() -> Element {
    let webspaces = use_server_future(move || list_webspaces(WebspaceListInput::default()))?;
    let all_rows: Vec<WebspaceRow> = match &*webspaces.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };

    let mut filter = use_signal(|| "all".to_string());
    let filtered_rows: Vec<&WebspaceRow> = match filter.read().as_str() {
        "pages" => all_rows
            .iter()
            .filter(|r| r.hosting_type == "cloudflare_pages")
            .collect(),
        "local" => all_rows
            .iter()
            .filter(|r| r.hosting_type == "local")
            .collect(),
        "relay" => all_rows
            .iter()
            .filter(|r| r.hosting_type == "relay")
            .collect(),
        "tunnel" => all_rows
            .iter()
            .filter(|r| r.hosting_type == "tunnel")
            .collect(),
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
