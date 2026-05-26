use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Card, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContactRow {
    id: Uuid,
    label: String,
    first_name: String,
    last_name: String,
    email: String,
    country: String,
    spaceship_synced: bool,
    organization_name: String,
}

#[server]
async fn list_contacts() -> Result<Vec<ContactRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_ids = user.org_ids();
    if org_ids.is_empty() && !user.is_admin {
        return Ok(vec![]);
    }

    let rows = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String, String, String, String, String, Option<String>, String)>(
            "SELECT c.id, c.label, c.first_name, c.last_name, c.email, c.country, c.spaceship_contact_id, o.name \
             FROM domain_contacts c JOIN organizations o ON o.id = c.organization_id \
             ORDER BY c.label",
        )
        .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, (Uuid, String, String, String, String, String, Option<String>, String)>(
            "SELECT c.id, c.label, c.first_name, c.last_name, c.email, c.country, c.spaceship_contact_id, o.name \
             FROM domain_contacts c JOIN organizations o ON o.id = c.organization_id \
             WHERE c.organization_id = ANY($1) \
             ORDER BY c.label",
        )
        .bind(&org_ids)
        .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                label,
                first_name,
                last_name,
                email,
                country,
                spaceship_id,
                organization_name,
            )| ContactRow {
                id,
                label,
                first_name,
                last_name,
                email,
                country,
                spaceship_synced: spaceship_id.is_some(),
                organization_name,
            },
        )
        .collect())
}

#[component]
pub fn ContactList() -> Element {
    let contacts = use_server_future(list_contacts)?;
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
