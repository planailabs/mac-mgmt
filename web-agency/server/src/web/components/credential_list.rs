use dioxus::prelude::*;

use super::ui::{Card, PageHeader, Td, TdMuted, Th};

// CredentialRow + the list endpoint now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::credentials::{CredentialListInput, list_credentials};

#[component]
pub fn CredentialList() -> Element {
    let credentials = use_server_future(move || list_credentials(CredentialListInput::default()))?;
    let rows = credentials.read();
    let rows = match &*rows {
        Some(Ok(r)) => r.as_slice(),
        _ => &[],
    };

    rsx! {
        PageHeader { "Credentials" }

        div { class: "mt-4 flex justify-end mb-4",
            Link {
                to: crate::web::app::Route::CredentialForm {},
                class: "btn btn-primary",
                "Add Credential"
            }
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Name" }
                            Th { "Type" }
                            Th { "Organization" }
                            Th { "Created" }
                        }
                    }
                    tbody {
                        if rows.is_empty() {
                            tr {
                                td { class: "td text-fg-muted text-center", colspan: "4", "No credentials yet" }
                            }
                        }
                        for row in rows {
                            tr { class: "cursor-pointer hover:bg-surface-2",
                                onclick: {
                                    let id = row.id.to_string();
                                    move |_| { navigator().push(crate::web::app::Route::CredentialEdit { id: id.clone() }); }
                                },
                                Td { "{row.name}" }
                                Td {
                                    span { class: "badge badge-info", "{row.credential_type}" }
                                }
                                TdMuted { {row.organization_name.as_deref().unwrap_or("Global")} }
                                TdMuted { "{row.created_at}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
