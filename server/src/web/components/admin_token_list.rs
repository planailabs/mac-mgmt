use dioxus::prelude::*;

use crate::models::Token;

#[server]
async fn list_admin_tokens() -> Result<Vec<Token>, ServerFnError> {
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
        "INSERT INTO tokens (customer_id, token_hash, label, kind) VALUES (NULL, $1, $2, 'admin')",
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
            div { class: "bg-green-50 border border-green-200 rounded p-3 mb-4",
                p { class: "text-sm font-medium text-green-800", "New token (copy now, shown once):" }
                code { class: "block mt-1 text-xs break-all bg-green-100 p-2 rounded", "{raw}" }
            }
        }

        form { onsubmit: on_create, class: "flex gap-2 mb-4",
            input {
                class: "flex-1 border border-gray-300 rounded px-3 py-1 text-sm",
                r#type: "text",
                required: true,
                placeholder: "Admin token label",
                value: "{label}",
                oninput: move |evt| label.set(evt.value()),
            }
            button {
                class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700",
                r#type: "submit",
                "Create Admin Token"
            }
        }

        {match &*tokens.read() {
            Some(Ok(list)) => rsx! {
                ul { class: "divide-y divide-gray-200",
                    for token in list {
                        {
                            let display_label = if token.label.is_empty() {
                                "(no label)".to_string()
                            } else {
                                token.label.clone()
                            };
                            let created = token.created_at.format("%Y-%m-%d").to_string();
                            let revoked = token.revoked;
                            let tid = token.id.to_string();
                            rsx! {
                                li { class: "py-2 flex justify-between items-center",
                                    div {
                                        span { class: "text-sm font-medium", "{display_label}" }
                                        span { class: "text-xs text-gray-400 ml-2", "{created}" }
                                        if revoked {
                                            span { class: "text-xs text-red-500 ml-2", "revoked" }
                                        }
                                    }
                                    if !revoked {
                                        button {
                                            class: "text-xs text-red-600 hover:underline",
                                            onclick: move |_| {
                                                let tid = tid.clone();
                                                spawn(async move {
                                                    if revoke_admin_token(tid).await.is_ok() {
                                                        tokens.restart();
                                                    }
                                                });
                                            },
                                            "Revoke"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! { p { class: "text-red-600 text-sm", "Error: {e}" } },
            None => rsx! { p { class: "text-sm", "Loading..." } },
        }}
    }
}
