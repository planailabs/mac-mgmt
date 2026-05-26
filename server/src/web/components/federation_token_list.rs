use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::models::Token;
use crate::web::components::ui::{
    Alert, AlertVariant, Badge, BadgeVariant, Button, ButtonKind, ButtonSize, ErrorText, HelpText,
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
async fn create_federation_token(label: String) -> Result<String, ServerFnError> {
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

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind) VALUES (NULL, $1, $2, 'federation')",
    )
    .bind(&hash)
    .bind(&label)
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

    let mut label = use_signal(String::new);
    let mut new_token = use_signal(|| None::<String>);

    let on_create = move |evt: FormEvent| {
        evt.prevent_default();
        let label_val = label.read().clone();
        spawn(async move {
            match create_federation_token(label_val).await {
                Ok(raw) => {
                    new_token.set(Some(raw));
                    label.set(String::new());
                    tokens.restart();
                }
                Err(e) => tracing::error!("failed to create federation token: {e}"),
            }
        });
    };

    rsx! {
        if let Some(raw) = &*new_token.read() {
            Alert { variant: AlertVariant::Success, class: "mb-4",
                p { class: "text-sm font-medium", {t!("federation-token-new")} }
                code { class: "block mt-1 text-xs break-all bg-success-soft p-2 rounded", "{raw}" }
            }
        }

        form { onsubmit: on_create, class: "flex gap-2 mb-4",
            input {
                class: "input flex-1 w-auto py-1 text-sm",
                r#type: "text",
                required: true,
                placeholder: t!("federation-token-label"),
                value: "{label}",
                oninput: move |evt| label.set(evt.value()),
            }
            Button { kind: ButtonKind::Submit, size: ButtonSize::Sm,
                {t!("federation-token-create")}
            }
        }

        {match &*tokens.read() {
            Some(Ok(list)) => rsx! {
                if list.is_empty() {
                    HelpText { {t!("federation-token-none")} }
                } else {
                    ul { class: "divide-y divide-line-soft",
                        for token in list {
                            TokenRow {
                                key: "{token.id}",
                                token: token.clone(),
                                on_revoke: {
                                    let tid = token.id.to_string();
                                    move |_| {
                                        let tid = tid.clone();
                                        spawn(async move {
                                            if revoke_federation_token(tid).await.is_ok() {
                                                tokens.restart();
                                            }
                                        });
                                    }
                                },
                            }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}

#[component]
fn TokenRow(token: Token, on_revoke: EventHandler<()>) -> Element {
    let display_label = if token.label.is_empty() {
        t!("no-label")
    } else {
        token.label.clone()
    };
    let created = token.created_at.format("%Y-%m-%d").to_string();
    let revoked = token.revoked;
    rsx! {
        li { class: "py-2 flex justify-between items-center",
            div {
                span { class: "text-sm font-medium", "{display_label}" }
                span { class: "text-xs text-fg-muted ml-2", "{created}" }
                if revoked {
                    span { class: "ml-2",
                        Badge { variant: BadgeVariant::Danger, {t!("revoked")} }
                    }
                }
            }
            if !revoked {
                button { class: "link-danger text-sm",
                    onclick: move |_| on_revoke.call(()),
                    {t!("federation-token-revoke")}
                }
            }
        }
    }
}
