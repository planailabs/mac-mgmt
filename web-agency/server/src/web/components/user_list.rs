use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Card, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserRow {
    id: Uuid,
    email: String,
    name: String,
    is_admin: bool,
    created_at: String,
}

#[server]
async fn list_users() -> Result<Vec<UserRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    use crate::web::user::WebUserExt;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let rows = sqlx::query_as::<_, (Uuid, String, String, bool, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, email, name, is_admin, created_at FROM users ORDER BY email",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|(id, email, name, is_admin, created_at)| UserRow {
        id, email, name, is_admin, created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
    }).collect())
}

#[component]
pub fn UserList() -> Element {
    let users = use_server_future(list_users)?;
    let rows = match &*users.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };

    rsx! {
        PageHeader { "Users" }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead { tr { Th { "Email" } Th { "Name" } Th { "Admin" } Th { "Joined" } } }
                    tbody {
                        if rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "4", "No users" } }
                        }
                        for row in &rows {
                            tr {
                                Td { "{row.email}" }
                                Td { "{row.name}" }
                                Td {
                                    if row.is_admin {
                                        Badge { variant: BadgeVariant::Accent, "Admin" }
                                    } else {
                                        span { class: "text-fg-muted", "No" }
                                    }
                                }
                                TdMuted { "{row.created_at}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
