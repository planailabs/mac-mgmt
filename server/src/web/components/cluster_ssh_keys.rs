use dioxus::prelude::*;
use dioxus_i18n::t;

#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct SshKeyDisplay {
    pub id: uuid::Uuid,
    pub fingerprint: String,
    pub comment: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[server]
async fn list_ssh_keys(cluster_id: String) -> Result<Vec<SshKeyDisplay>, ServerFnError> {
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
    let keys = sqlx::query_as::<_, SshKeyDisplay>(
        "SELECT id, fingerprint, comment, created_at \
         FROM cluster_ssh_keys WHERE cluster_id = $1 ORDER BY created_at",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(keys)
}

#[server]
async fn add_ssh_key(cluster_id: String, public_key: String) -> Result<(), ServerFnError> {
    use base64::Engine;
    use sha2::{Digest, Sha256};

    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let org_ids = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT organization_id FROM organization_clusters WHERE cluster_id = $1",
    )
    .bind(cid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    if org_ids.is_empty() || !org_ids.iter().any(|oid| user.is_org_admin(oid)) {
        return Err(ServerFnError::new("organization admin access required"));
    }

    let trimmed = public_key.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(ServerFnError::new("Invalid SSH public key format"));
    }
    let b64_data = base64::engine::general_purpose::STANDARD
        .decode(parts[1])
        .map_err(|_| ServerFnError::new("Invalid base64 in SSH key"))?;
    let fingerprint = format!(
        "SHA256:{}",
        base64::engine::general_purpose::STANDARD.encode(Sha256::digest(&b64_data))
    );
    let comment = if parts.len() > 2 {
        parts[2..].join(" ")
    } else {
        String::new()
    };

    sqlx::query(
        "INSERT INTO cluster_ssh_keys (cluster_id, public_key, comment, fingerprint) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(cid)
    .bind(trimmed)
    .bind(&comment)
    .bind(&fingerprint)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncSshKeys).await;
    Ok(())
}

#[server]
async fn remove_ssh_key(ssh_key_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = ssh_key_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    // Check access before deleting
    let owner_cid = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT cluster_id FROM cluster_ssh_keys WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(owner_cid) = owner_cid {
        let org_ids = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT organization_id FROM organization_clusters WHERE cluster_id = $1",
        )
        .bind(owner_cid)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
        if org_ids.is_empty() || !org_ids.iter().any(|oid| user.is_org_admin(oid)) {
            return Err(ServerFnError::new("organization admin access required"));
        }
    }
    let cid = sqlx::query_scalar::<_, uuid::Uuid>(
        "DELETE FROM cluster_ssh_keys WHERE id = $1 RETURNING cluster_id",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(cid) = cid {
        crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncSshKeys).await;
    }
    Ok(())
}

#[component]
pub fn ClusterSshKeys(cluster_id: String, read_only: bool) -> Element {
    let cid_list = cluster_id.clone();
    let mut keys = use_server_future(move || {
        let cid = cid_list.clone();
        async move { list_ssh_keys(cid).await }
    })?;

    let mut key_input = use_signal(String::new);
    let mut error_msg = use_signal(|| None::<String>);

    let cid_add = cluster_id.clone();

    rsx! {
        if !read_only {
            if let Some(err) = &*error_msg.read() {
                p { class: "text-red-600 dark:text-red-400 text-sm mb-2", "{err}" }
            }
            form {
                class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let cid = cid_add.clone();
                    let pk = key_input.read().clone();
                    spawn(async move {
                        if !pk.trim().is_empty() {
                            match add_ssh_key(cid, pk).await {
                                Ok(()) => {
                                    error_msg.set(None);
                                    key_input.set(String::new());
                                    keys.restart();
                                }
                                Err(e) => {
                                    error_msg.set(Some(e.to_string()));
                                }
                            }
                        }
                    });
                },
                textarea {
                    class: "flex-1 border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm font-mono dark:bg-gray-700 dark:text-white",
                    rows: 2,
                    placeholder: t!("ssh-keys-placeholder"),
                    value: "{key_input}",
                    oninput: move |e| key_input.set(e.value()),
                }
                button {
                    class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700 self-start",
                    r#type: "submit",
                    {t!("add")}
                }
            }
        }
        {match &*keys.read() {
            Some(Ok(list)) if list.is_empty() => rsx! {
                p { class: "text-gray-500 dark:text-gray-400 text-sm", {t!("ssh-keys-no-keys")} }
            },
            Some(Ok(list)) => rsx! {
                ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                    for key in list {
                        {
                            let kid = key.id.to_string();
                            let fp = key.fingerprint.clone();
                            let comment = key.comment.clone();
                            rsx! {
                                li { class: "py-2 flex justify-between items-center",
                                    div {
                                        span { class: "text-sm font-mono", "{fp}" }
                                        if !comment.is_empty() {
                                            span { class: "text-xs text-gray-500 dark:text-gray-400 ml-2", "{comment}" }
                                        }
                                    }
                                    if !read_only {
                                        button {
                                            class: "text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 text-sm",
                                            onclick: move |_| {
                                                let kid = kid.clone();
                                                spawn(async move {
                                                    if remove_ssh_key(kid).await.is_ok() {
                                                        keys.restart();
                                                    }
                                                });
                                            },
                                            {t!("remove")}
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
