use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::models::Token;
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[server]
async fn list_tokens(cluster_id: String) -> Result<Vec<Token>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    let tokens = sqlx::query_as::<_, Token>(
        "SELECT * FROM tokens WHERE cluster_id = $1 AND kind = 'sync' ORDER BY created_at DESC",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(tokens)
}

#[server]
async fn create_token(cluster_id: String, label: String) -> Result<String, ServerFnError> {
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let user = current_user().await?;

    let label = label.trim().to_string();
    if label.is_empty() {
        return Err(ServerFnError::new("label is required"));
    }

    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .writable_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind) VALUES ($1, $2, $3, 'sync')",
    )
    .bind(uuid)
    .bind(&hash)
    .bind(&label)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(raw_token)
}

#[server]
async fn revoke_token(token_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = token_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    // Check access before revoking
    let owner_cid =
        sqlx::query_scalar::<_, uuid::Uuid>("SELECT cluster_id FROM tokens WHERE id = $1")
            .bind(uuid)
            .fetch_optional(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(owner_cid) = owner_cid {
        if let Some(ids) = user
            .writable_cluster_ids(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        {
            if !ids.contains(&owner_cid) {
                return Err(ServerFnError::new("access denied"));
            }
        }
    }
    sqlx::query("UPDATE tokens SET revoked = true WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn SyncTokenList(cluster_id: String, read_only: bool) -> Element {
    let cid = cluster_id.clone();
    let mut tokens = use_server_future(move || {
        let cid = cid.clone();
        async move { list_tokens(cid).await }
    })?;

    let mut label = use_signal(String::new);
    let mut new_token = use_signal(|| None::<String>);

    let cid_create = cluster_id.clone();
    let on_create = move |evt: FormEvent| {
        evt.prevent_default();
        let cid = cid_create.clone();
        let label_val = label.read().clone();
        spawn(async move {
            match create_token(cid, label_val).await {
                Ok(raw) => {
                    new_token.set(Some(raw));
                    label.set(String::new());
                    tokens.restart();
                }
                Err(e) => tracing::error!("failed to create token: {e}"),
            }
        });
    };

    rsx! {
        if !read_only {
            if let Some(raw) = &*new_token.read() {
                div { class: "bg-green-50 dark:bg-green-900/30 border border-green-200 dark:border-green-700 rounded p-3 mb-4",
                    p { class: "text-sm font-medium text-green-800 dark:text-green-300", {t!("sync-token-new")} }
                    code { class: "block mt-1 text-xs break-all bg-green-100 dark:bg-green-900/50 p-2 rounded", "{raw}" }
                }
            }

            form { onsubmit: on_create, class: "flex gap-2 mb-4",
                input {
                    class: "flex-1 border border-gray-300 dark:border-gray-600 rounded px-3 py-1 text-sm dark:bg-gray-700 dark:text-white",
                    r#type: "text",
                    required: true,
                    placeholder: t!("sync-token-label"),
                    value: "{label}",
                    oninput: move |evt| label.set(evt.value()),
                }
                button {
                    class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700",
                    r#type: "submit",
                    {t!("sync-token-create")}
                }
            }
        }

        {match &*tokens.read() {
            Some(Ok(list)) => rsx! {
                ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                    for token in list {
                        {
                            let display_label = if token.label.is_empty() {
                                t!("no-label")
                            } else {
                                token.label.clone()
                            };
                            let created = token.created_at.format("%Y-%m-%d %H:%M").to_string();
                            let revoked = token.revoked;
                            let expired = token.expires_at.is_some_and(|e| e < chrono::Utc::now());
                            let expires_label = token.expires_at.map(|e| {
                                let date = e.format("%Y-%m-%d %H:%M").to_string();
                                if expired {
                                    t!("sync-token-expired", date: date)
                                } else {
                                    t!("sync-token-expires", date: date)
                                }
                            });
                            let tid = token.id.to_string();
                            rsx! {
                                li { class: "py-2 flex justify-between items-center",
                                    div {
                                        span { class: "text-sm font-medium", "{display_label}" }
                                        span { class: "text-xs text-gray-500 dark:text-gray-400 ml-2", "{created}" }
                                        if let Some(exp) = &expires_label {
                                            if expired {
                                                span { class: "px-2 py-0.5 rounded text-xs font-medium bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200 ml-2", "{exp}" }
                                            } else {
                                                span { class: "text-xs text-gray-500 dark:text-gray-400 ml-2", "{exp}" }
                                            }
                                        }
                                        if revoked {
                                            span { class: "px-2 py-0.5 rounded text-xs font-medium bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200 ml-2", {t!("revoked")} }
                                        }
                                    }
                                    if !revoked && !expired && !read_only {
                                        button {
                                            class: "text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 text-sm",
                                            onclick: move |_| {
                                                let tid = tid.clone();
                                                spawn(async move {
                                                    if revoke_token(tid).await.is_ok() {
                                                        tokens.restart();
                                                    }
                                                });
                                            },
                                            {t!("sync-token-revoke")}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", {t!("error-message", message: e.to_string())} } },
            None => rsx! { p { class: "text-gray-500 dark:text-gray-400 text-sm", {t!("loading")} } },
        }}
    }
}
