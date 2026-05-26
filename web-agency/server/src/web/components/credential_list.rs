use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Card, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialRow {
    id: Uuid,
    name: String,
    credential_type: String,
    organization_name: Option<String>,
    created_at: String,
}

#[server]
async fn list_credentials() -> Result<Vec<CredentialRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let rows = if user.is_admin {
        sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                String,
                Option<String>,
                chrono::DateTime<chrono::Utc>,
            ),
        >(
            "SELECT c.id, c.name, c.credential_type, o.name, c.created_at \
             FROM credentials c LEFT JOIN organizations o ON o.id = c.organization_id \
             ORDER BY c.created_at DESC",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        let org_ids = user.org_ids();
        sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                String,
                Option<String>,
                chrono::DateTime<chrono::Utc>,
            ),
        >(
            "SELECT c.id, c.name, c.credential_type, o.name, c.created_at \
             FROM credentials c LEFT JOIN organizations o ON o.id = c.organization_id \
             WHERE c.organization_id = ANY($1) OR c.organization_id IS NULL \
             ORDER BY c.created_at DESC",
        )
        .bind(&org_ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows
        .into_iter()
        .map(
            |(id, name, credential_type, organization_name, created_at)| CredentialRow {
                id,
                name,
                credential_type,
                organization_name,
                created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
            },
        )
        .collect())
}

#[component]
pub fn CredentialList() -> Element {
    let credentials = use_server_future(list_credentials)?;
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
