use dioxus::prelude::*;

use super::ui::{Card, PageHeader, Td, TdMuted, Th};

// BasicAuthListRow + the list endpoint now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::basic_auth::{BasicAuthListInput, list_basic_auth_lists};

#[component]
pub fn BasicAuthList() -> Element {
    let lists = use_server_future(move || list_basic_auth_lists(BasicAuthListInput::default()))?;
    let rows = match &*lists.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };

    rsx! {
        PageHeader { "Basic Auth Lists" }

        div { class: "mt-4 flex justify-end mb-4",
            Link { to: crate::web::app::Route::BasicAuthForm {}, class: "btn btn-primary", "Create List" }
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead { tr { Th { "Name" } Th { "Organization" } Th { "Credentials" } } }
                    tbody {
                        if rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "3", "No basic auth lists" } }
                        }
                        for row in &rows {
                            tr { class: "cursor-pointer hover:bg-surface-2",
                                onclick: {
                                    let id = row.id.to_string();
                                    move |_| {
                                        navigator().push(crate::web::app::Route::BasicAuthDetail { id: id.clone() });
                                    }
                                },
                                Td { "{row.name}" }
                                TdMuted { "{row.organization_name}" }
                                Td { "{row.credential_count}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
