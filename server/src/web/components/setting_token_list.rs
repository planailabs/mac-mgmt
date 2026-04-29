use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::models::Token;
use crate::web::components::ui::{
    Alert, AlertVariant, Badge, BadgeVariant, Button, ButtonKind, ButtonSize, ErrorText, HelpText,
};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[server]
async fn list_setting_tokens(cluster_id: String) -> Result<Vec<Token>, ServerFnError> {
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
        "SELECT * FROM tokens WHERE cluster_id = $1 AND kind = 'setting' ORDER BY created_at DESC",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(tokens)
}

#[server]
async fn create_setting_token(cluster_id: String, label: String) -> Result<String, ServerFnError> {
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
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(6);

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at) VALUES ($1, $2, $3, 'setting', $4)",
    )
    .bind(uuid)
    .bind(&hash)
    .bind(&label)
    .bind(expires_at)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(raw_token)
}

#[server]
async fn revoke_setting_token(token_id: String) -> Result<(), ServerFnError> {
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
pub fn SettingTokenList(cluster_id: String, read_only: bool) -> Element {
    let cid = cluster_id.clone();
    let mut tokens = use_server_future(move || {
        let cid = cid.clone();
        async move { list_setting_tokens(cid).await }
    })?;

    let mut label = use_signal(String::new);
    let mut new_token = use_signal(|| None::<String>);

    let cid_create = cluster_id.clone();
    let on_create = move |evt: FormEvent| {
        evt.prevent_default();
        let cid = cid_create.clone();
        let label_val = label.read().clone();
        spawn(async move {
            match create_setting_token(cid, label_val).await {
                Ok(raw) => {
                    new_token.set(Some(raw));
                    label.set(String::new());
                    tokens.restart();
                }
                Err(e) => tracing::error!("failed to create setting token: {e}"),
            }
        });
    };

    rsx! {
        if !read_only {
            if let Some(raw) = &*new_token.read() {
                Alert { variant: AlertVariant::Success, class: "mb-4",
                    p { class: "text-sm font-medium", {t!("setting-token-new")} }
                    code { class: "block mt-1 text-xs break-all bg-success-soft p-2 rounded", "{raw}" }
                }
            }

            form { onsubmit: on_create, class: "flex gap-2 mb-4",
                input {
                    class: "input flex-1 w-auto py-1 text-sm",
                    r#type: "text",
                    required: true,
                    placeholder: t!("setting-token-label"),
                    value: "{label}",
                    oninput: move |evt| label.set(evt.value()),
                }
                Button { kind: ButtonKind::Submit, size: ButtonSize::Sm,
                    {t!("setting-token-create")}
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
                            expires_kind: "setting",
                            on_revoke: {
                                let tid = token.id.to_string();
                                move |_| {
                                    let tid = tid.clone();
                                    spawn(async move {
                                        if revoke_setting_token(tid).await.is_ok() {
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

/// Shared between setting + sync token lists. `expires_kind` selects the
/// `t!` keys for the two slightly different label sets — passing the
/// scoped identifier keeps i18n strings exact-match-grep-able.
#[component]
pub fn ExpiringTokenRow(
    token: Token,
    read_only: bool,
    expires_kind: &'static str,
    on_revoke: EventHandler<()>,
) -> Element {
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
        match (expires_kind, expired) {
            ("setting", true)  => t!("setting-token-expired", date: date),
            ("setting", false) => t!("setting-token-expires", date: date),
            ("sync", true)     => t!("sync-token-expired", date: date),
            ("sync", false)    => t!("sync-token-expires", date: date),
            _ => date,
        }
    });
    let revoke_label = if expires_kind == "setting" {
        t!("setting-token-revoke")
    } else {
        t!("sync-token-revoke")
    };

    rsx! {
        li { class: "py-2 flex justify-between items-center",
            div {
                span { class: "text-sm font-medium", "{display_label}" }
                span { class: "text-xs text-fg-muted ml-2", "{created}" }
                if let Some(exp) = &expires_label {
                    if expired {
                        span { class: "ml-2",
                            Badge { variant: BadgeVariant::Danger, "{exp}" }
                        }
                    } else {
                        span { class: "text-xs text-fg-muted ml-2", "{exp}" }
                    }
                }
                if revoked {
                    span { class: "ml-2",
                        Badge { variant: BadgeVariant::Danger, {t!("revoked")} }
                    }
                }
            }
            if !revoked && !expired && !read_only {
                button { class: "link-danger text-sm",
                    onclick: move |_| on_revoke.call(()),
                    {revoke_label}
                }
            }
        }
    }
}
