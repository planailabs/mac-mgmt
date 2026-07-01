use dioxus::prelude::*;

use super::ui::{Badge, BadgeVariant, Card, PageHeader, Td, TdMuted, Th};

// ContactRow + the list endpoint now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::contacts::{ContactListInput, list_contacts};

#[component]
pub fn ContactList() -> Element {
    let contacts = use_server_future(move || list_contacts(ContactListInput::default()))?;
    let rows = contacts.read();
    let rows = match &*rows {
        Some(Ok(r)) => r.as_slice(),
        _ => &[],
    };

    rsx! {
        PageHeader { "Contacts" }

        div { class: "mt-4 flex justify-end mb-4",
            Link {
                to: crate::web::app::Route::ContactForm {},
                class: "btn btn-primary",
                "Add Contact"
            }
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Label" }
                            Th { "Name" }
                            Th { "Email" }
                            Th { "Country" }
                            Th { "Spaceship" }
                            Th { "Org" }
                        }
                    }
                    tbody {
                        if rows.is_empty() {
                            tr {
                                td { class: "td text-fg-muted text-center", colspan: "6", "No contacts yet" }
                            }
                        }
                        for row in rows {
                            tr {
                                Td { "{row.label}" }
                                Td { "{row.first_name} {row.last_name}" }
                                Td { "{row.email}" }
                                Td { "{row.country}" }
                                Td {
                                    if row.spaceship_synced {
                                        Badge { variant: BadgeVariant::Success, "Synced" }
                                    } else {
                                        span { class: "text-fg-muted", "Not synced" }
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
