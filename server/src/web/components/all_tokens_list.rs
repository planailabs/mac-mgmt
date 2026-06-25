use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::components::ui::{ErrorText, HelpText, TokenRow, TokenTable};
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

/// One token row for the master list, with its scope already resolved to a
/// human-readable string on the server (e.g. "org: Acme", "cluster: edge-1").
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct AllTokenRow {
    id: String,
    label: String,
    kind: String,
    scope: Option<String>,
    scope_href: Option<String>,
    revoked: bool,
    created: String,
    expires: Option<String>,
    expired: bool,
}

#[server]
async fn list_all_tokens() -> Result<Vec<AllTokenRow>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        label: String,
        kind: String,
        revoked: bool,
        created_at: chrono::DateTime<chrono::Utc>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        organization_id: Option<uuid::Uuid>,
        org_name: Option<String>,
        cluster_id: Option<uuid::Uuid>,
        cluster_name: Option<String>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT t.id, t.label, t.kind, t.revoked, t.created_at, t.expires_at, \
                t.organization_id, o.name AS org_name, t.cluster_id, c.name AS cluster_name \
         FROM tokens t \
         LEFT JOIN organizations o ON o.id = t.organization_id \
         LEFT JOIN clusters c ON c.id = t.cluster_id \
         ORDER BY t.created_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let now = chrono::Utc::now();
    Ok(rows
        .into_iter()
        .map(|r| {
            // Resolve scope: admin/federation are global kinds; otherwise show
            // the specific org or cluster the token is bound to.
            let scope = match r.kind.as_str() {
                "admin" => Some("admin".to_string()),
                "federation" => Some("federation".to_string()),
                _ => r
                    .org_name
                    .map(|n| format!("org: {n}"))
                    .or_else(|| r.cluster_name.map(|n| format!("cluster: {n}"))),
            };
            // Link admin/federation scopes nowhere; org/cluster link to their page.
            let scope_href = match r.kind.as_str() {
                "admin" | "federation" => None,
                _ => r
                    .organization_id
                    .map(|id| format!("/organizations/{id}"))
                    .or_else(|| r.cluster_id.map(|id| format!("/clusters/{id}"))),
            };
            AllTokenRow {
                id: r.id.to_string(),
                label: if r.label.is_empty() {
                    t!("no-label")
                } else {
                    r.label
                },
                kind: r.kind,
                scope,
                scope_href,
                revoked: r.revoked,
                created: r.created_at.format("%Y-%m-%d %H:%M").to_string(),
                expires: r.expires_at.map(|e| e.format("%Y-%m-%d %H:%M").to_string()),
                expired: r.expires_at.is_some_and(|e| e < now),
            }
        })
        .collect())
}

#[server]
async fn revoke_any_token(token_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = token_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("UPDATE tokens SET revoked = true WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

/// Read-only master list of every token across the system, with scope. No
/// creation menu — this is purely for visibility/audit (admin only).
#[component]
pub fn AllTokensList() -> Element {
    let mut tokens = use_server_future(move || async move { list_all_tokens().await })?;

    rsx! {
        {match &*tokens.read() {
            Some(Ok(list)) => {
                let rows = list.iter().map(|t| TokenRow {
                    id: t.id.clone(),
                    label: t.label.clone(),
                    kind: Some(t.kind.clone()),
                    scope: t.scope.clone(),
                    scope_href: t.scope_href.clone(),
                    revoked: t.revoked,
                    expired: t.expired,
                    created: t.created.clone(),
                    expires: t.expires.clone(),
                }).collect::<Vec<_>>();
                rsx! {
                    TokenTable {
                        rows,
                        show_kind: true,
                        show_scope: true,
                        show_expires: true,
                        on_revoke: move |id: String| {
                            spawn(async move {
                                if revoke_any_token(id).await.is_ok() {
                                    tokens.restart();
                                }
                            });
                        },
                    }
                }
            }
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}
