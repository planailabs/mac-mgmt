use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::models::Token;
use crate::web::components::setting_token_list::ExpiringTokenRow;
use crate::web::components::ui::{
    Alert, AlertVariant, Button, ButtonKind, ButtonSize, ErrorText, HelpText,
};
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

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
                Alert { variant: AlertVariant::Success, class: "mb-4",
                    p { class: "text-sm font-medium", {t!("sync-token-new")} }
                    code { class: "block mt-1 text-xs break-all bg-success-soft p-2 rounded", "{raw}" }
                }
            }

            form { onsubmit: on_create, class: "flex gap-2 mb-4",
                input {
                    class: "input flex-1 w-auto py-1 text-sm",
                    r#type: "text",
                    required: true,
                    placeholder: t!("sync-token-label"),
                    value: "{label}",
                    oninput: move |evt| label.set(evt.value()),
                }
                Button { kind: ButtonKind::Submit, size: ButtonSize::Sm,
                    {t!("sync-token-create")}
                }
            }
        }

        {match &*tokens.read() {
            Some(Ok(list)) => rsx! {
                ul { class: "divide-y divide-line-soft",
                    for token in list {
                        ExpiringTokenRow {
                            key: "{token.id}",
                            token: token.clone(),
                            read_only,
                            expires_kind: "sync",
                            on_revoke: {
                                let tid = token.id.to_string();
                                move |_| {
                                    let tid = tid.clone();
                                    spawn(async move {
                                        if revoke_token(tid).await.is_ok() {
                                            tokens.restart();
                                        }
                                    });
                                }
                            },
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}
