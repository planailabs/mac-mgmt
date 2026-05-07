use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Card, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BasicAuthListRow {
    id: Uuid,
    name: String,
    organization_name: String,
    credential_count: i64,
}

#[server]
async fn list_basic_auth_lists() -> Result<Vec<BasicAuthListRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_ids = user.org_ids();
    if org_ids.is_empty() && !user.is_admin {
        return Ok(vec![]);
    }

    let rows = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String, String, i64)>(
            "SELECT b.id, b.name, o.name, \
             (SELECT count(*) FROM basic_auth_credentials c WHERE c.list_id = b.id) \
             FROM basic_auth_lists b JOIN organizations o ON o.id = b.organization_id \
             ORDER BY b.name",
        )
        .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, (Uuid, String, String, i64)>(
            "SELECT b.id, b.name, o.name, \
             (SELECT count(*) FROM basic_auth_credentials c WHERE c.list_id = b.id) \
             FROM basic_auth_lists b JOIN organizations o ON o.id = b.organization_id \
             WHERE b.organization_id = ANY($1) \
             ORDER BY b.name",
        )
        .bind(&org_ids)
        .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows.into_iter().map(|(id, name, organization_name, credential_count)| {
        BasicAuthListRow { id, name, organization_name, credential_count }
    }).collect())
}

#[component]
pub fn BasicAuthList() -> Element {
    let lists = use_server_future(list_basic_auth_lists)?;
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
