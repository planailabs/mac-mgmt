use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Card, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenRow {
    id: Uuid,
    label: String,
    kind: String,
    revoked: bool,
    created_at: String,
    expires_at: Option<String>,
}

#[server]
async fn list_tokens() -> Result<Vec<TokenRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    use crate::web::user::WebUserExt;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let rows = sqlx::query_as::<_, (Uuid, String, String, bool, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT id, label, kind, revoked, created_at, expires_at FROM tokens ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(
            |(id, label, kind, revoked, created_at, expires_at)| TokenRow {
                id,
                label,
                kind,
                revoked,
                created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
                expires_at: expires_at.map(|d| d.format("%Y-%m-%d %H:%M").to_string()),
            },
        )
        .collect())
}

#[component]
pub fn TokenList() -> Element {
    let tokens = use_server_future(list_tokens)?;
    let rows = match &*tokens.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };

    rsx! {
        PageHeader { "API Tokens" }

        div { class: "mt-4 flex justify-end mb-4",
            Link { to: crate::web::app::Route::TokenForm {}, class: "btn btn-primary", "Create Token" }
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead { tr { Th { "Label" } Th { "Kind" } Th { "Status" } Th { "Created" } Th { "Expires" } } }
                    tbody {
                        if rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "5", "No tokens" } }
                        }
                        for row in &rows {
                            tr {
                                Td { "{row.label}" }
                                Td { Badge { variant: BadgeVariant::Info, "{row.kind}" } }
                                Td {
                                    if row.revoked {
                                        Badge { variant: BadgeVariant::Danger, "Revoked" }
                                    } else {
                                        Badge { variant: BadgeVariant::Success, "Active" }
                                    }
                                }
                                TdMuted { "{row.created_at}" }
                                TdMuted { {row.expires_at.as_deref().unwrap_or("Never")} }
                            }
                        }
                    }
                }
            }
        }
    }
}
