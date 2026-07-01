use dioxus::prelude::*;

use super::ui::{Card, PageHeader, Td, TdMuted, Th};

// OrgRow + the list endpoint now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::organizations::{OrgListInput, list_organizations};

#[component]
pub fn OrganizationList() -> Element {
    let orgs = use_server_future(move || list_organizations(OrgListInput::default()))?;
    let rows = match &*orgs.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };

    rsx! {
        PageHeader { "Organizations" }

        div { class: "mt-4 flex justify-end mb-4",
            Link { to: crate::web::app::Route::OrganizationForm {}, class: "btn btn-primary", "Create Organization" }
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead { tr { Th { "Name" } Th { "Members" } Th { "Created" } } }
                    tbody {
                        if rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "3", "No organizations" } }
                        }
                        for row in &rows {
                            tr { class: "cursor-pointer hover:bg-surface-2",
                                onclick: {
                                    let id = row.id.to_string();
                                    move |_| { navigator().push(crate::web::app::Route::OrganizationDetail { id: id.clone() }); }
                                },
                                Td { "{row.name}" }
                                Td { "{row.member_count}" }
                                TdMuted { "{row.created_at}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
