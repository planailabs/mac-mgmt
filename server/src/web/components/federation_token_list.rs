use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::models::Token;
use crate::web::components::ui::{
    ErrorText, HelpText, TokenCreateForm, TokenCreateInput, TokenReveal, TokenRow, TokenTable,
};
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

#[server]
async fn list_federation_tokens() -> Result<Vec<Token>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let tokens = sqlx::query_as::<_, Token>(
        "SELECT * FROM tokens WHERE kind = 'federation' ORDER BY created_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(tokens)
}

#[server]
async fn create_federation_token(
    label: String,
    expires_in_secs: Option<i64>,
) -> Result<String, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let label = label.trim().to_string();
    if label.is_empty() {
        return Err(ServerFnError::new("label is required"));
    }

    let pool = crate::server_pool()?;

    let raw_token = format!("fed_{}", hex::encode(rand::rng().random::<[u8; 32]>()));
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = expires_in_secs.map(|s| chrono::Utc::now() + chrono::Duration::seconds(s));

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at) VALUES (NULL, $1, $2, 'federation', $3)",
    )
    .bind(&hash)
    .bind(&label)
    .bind(expires_at)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(raw_token)
}

#[server]
async fn revoke_federation_token(token_id: String) -> Result<(), ServerFnError> {
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

#[component]
pub fn FederationTokenList() -> Element {
    let mut tokens = use_server_future(move || async move { list_federation_tokens().await })?;
    let mut new_token = use_signal(|| None::<String>);

    rsx! {
        if let Some(raw) = &*new_token.read() {
            TokenReveal { value: raw.clone(), label: t!("federation-token-new") }
        }

        TokenCreateForm {
            submit_label: t!("federation-token-create"),
            on_submit: move |input: TokenCreateInput| {
                spawn(async move {
                    match create_federation_token(input.label, input.expires_in_secs).await {
                        Ok(raw) => {
                            new_token.set(Some(raw));
                            tokens.restart();
                        }
                        Err(e) => tracing::error!("failed to create federation token: {e}"),
                    }
                });
            },
        }

        {match &*tokens.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! { HelpText { {t!("federation-token-none")} } }
                } else {
                    let rows = list.iter().map(|t| TokenRow {
                        id: t.id.to_string(),
                        label: if t.label.is_empty() { t!("no-label") } else { t.label.clone() },
                        kind: None,
                        scope: None,
                        scope_href: None,
                        revoked: t.revoked,
                        expired: false,
                        created: t.created_at.format("%Y-%m-%d").to_string(),
                        expires: None,
                    }).collect::<Vec<_>>();
                    rsx! {
                        TokenTable {
                            rows,
                            on_revoke: move |id: String| {
                                spawn(async move {
                                    if revoke_federation_token(id).await.is_ok() {
                                        tokens.restart();
                                    }
                                });
                            },
                        }
                    }
                }
            }
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}
