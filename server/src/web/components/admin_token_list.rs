use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::models::Token;
use crate::web::components::ui::{
    Button, ButtonKind, ButtonSize, ErrorText, HelpText, TokenReveal, TokenRow, TokenTable,
};
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

#[server]
async fn list_admin_tokens() -> Result<Vec<Token>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let tokens = sqlx::query_as::<_, Token>(
        "SELECT * FROM tokens WHERE kind = 'admin' ORDER BY created_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(tokens)
}

#[server]
async fn create_admin_token(label: String) -> Result<String, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let label = label.trim().to_string();
    if label.is_empty() {
        return Err(ServerFnError::new("label is required"));
    }

    let pool = crate::server_pool()?;

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind) VALUES (NULL, $1, $2, 'admin')",
    )
    .bind(&hash)
    .bind(&label)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(raw_token)
}

#[server]
async fn revoke_admin_token(token_id: String) -> Result<(), ServerFnError> {
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
pub fn AdminTokenList() -> Element {
    let mut tokens = use_server_future(move || async move { list_admin_tokens().await })?;

    let mut label = use_signal(String::new);
    let mut new_token = use_signal(|| None::<String>);

    let on_create = move |evt: FormEvent| {
        evt.prevent_default();
        let label_val = label.read().clone();
        spawn(async move {
            match create_admin_token(label_val).await {
                Ok(raw) => {
                    new_token.set(Some(raw));
                    label.set(String::new());
                    tokens.restart();
                }
                Err(e) => tracing::error!("failed to create admin token: {e}"),
            }
        });
    };

    rsx! {
        if let Some(raw) = &*new_token.read() {
            TokenReveal { value: raw.clone(), label: t!("admin-token-new") }
        }

        form { onsubmit: on_create, class: "flex gap-2 mb-4",
            input {
                class: "input flex-1 w-auto py-1 text-sm",
                r#type: "text",
                required: true,
                placeholder: t!("admin-token-label-placeholder"),
                value: "{label}",
                oninput: move |evt| label.set(evt.value()),
            }
            Button { kind: ButtonKind::Submit, size: ButtonSize::Sm,
                {t!("admin-token-create")}
            }
        }

        {match &*tokens.read() {
            Some(Ok(list)) => {
                let rows = list.iter().map(|t| TokenRow {
                    id: t.id.to_string(),
                    label: if t.label.is_empty() { t!("no-label") } else { t.label.clone() },
                    kind: None,
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
                                if revoke_admin_token(id).await.is_ok() {
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
