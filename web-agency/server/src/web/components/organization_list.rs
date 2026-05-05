use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Card, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrgRow {
    id: Uuid,
    name: String,
    member_count: i64,
    created_at: String,
}

#[server]
async fn list_organizations() -> Result<Vec<OrgRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    use crate::web::user::WebUserExt;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let rows = sqlx::query_as::<_, (Uuid, String, i64, chrono::DateTime<chrono::Utc>)>(
        "SELECT o.id, o.name, \
         (SELECT count(*) FROM organization_members om WHERE om.organization_id = o.id), \
         o.created_at FROM organizations o ORDER BY o.name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|(id, name, member_count, created_at)| OrgRow {
        id, name, member_count, created_at: created_at.format("%Y-%m-%d").to_string(),
    }).collect())
}

#[component]
pub fn OrganizationList() -> Element {
    let orgs = use_server_future(list_organizations)?;
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
                            tr {
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
