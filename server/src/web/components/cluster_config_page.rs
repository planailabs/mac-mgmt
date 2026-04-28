use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[cfg(feature = "server")]
fn encrypt_secret_value(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::{Aead, KeyInit, OsRng};
    use aes_gcm::{AeadCore, Aes256Gcm, Key};

    let cfg = crate::config::config();
    let key_b64 = &cfg.secrets.encryption_key;
    let key_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, key_b64)
        .map_err(|e| format!("invalid encryption key: {e}"))?;
    if key_bytes.len() != 32 {
        return Err("encryption key must be 32 bytes".into());
    }
    let key = *Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(&key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| "encryption failed")?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

use super::config_editor::ConfigEditor;
use super::config_history::ConfigHistory;

// ── Server functions ────────────────────────────────────────────────────

#[server]
async fn can_write_cluster(cluster_id: String) -> Result<bool, ServerFnError> {
    let user = current_user().await?;
    if user.is_admin {
        return Ok(true);
    }
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    match user
        .writable_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        Some(ids) => Ok(ids.contains(&uuid)),
        None => Ok(true),
    }
}

#[server]
async fn get_cluster_name(cluster_id: String) -> Result<String, ServerFnError> {
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
    let name: String = sqlx::query_scalar("SELECT name FROM clusters WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(name)
}

// ── Secrets types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecretEntry {
    name: String,
    created_at: String,
}

#[server]
async fn list_secrets(cluster_id: String) -> Result<Vec<SecretEntry>, ServerFnError> {
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

    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        created_at: chrono::DateTime<chrono::Utc>,
    }
    let rows = sqlx::query_as::<_, Row>(
        "SELECT name, created_at FROM cluster_secrets WHERE cluster_id = $1 ORDER BY name",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| SecretEntry {
            name: r.name,
            created_at: r.created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect())
}

#[server]
async fn create_secret(
    cluster_id: String,
    name: String,
    value: String,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
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

    let encrypted = encrypt_secret_value(value.as_bytes())
        .map_err(|e| ServerFnError::new(format!("encryption error: {e}")))?;

    sqlx::query(
        "INSERT INTO cluster_secrets (cluster_id, name, encrypted_value) VALUES ($1, $2, $3)",
    )
    .bind(uuid)
    .bind(&name)
    .bind(&encrypted)
    .execute(&pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate key") {
            ServerFnError::new(format!("secret '{name}' already exists"))
        } else {
            ServerFnError::new(e.to_string())
        }
    })?;

    crate::api::push::notify_global(uuid, crate::api::push::PushMessage::SyncConfig).await;
    Ok(())
}

#[server]
async fn update_secret(
    cluster_id: String,
    name: String,
    value: String,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
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

    let encrypted = encrypt_secret_value(value.as_bytes())
        .map_err(|e| ServerFnError::new(format!("encryption error: {e}")))?;

    let result = sqlx::query(
        "UPDATE cluster_secrets SET encrypted_value = $1, updated_at = now() \
         WHERE cluster_id = $2 AND name = $3",
    )
    .bind(&encrypted)
    .bind(uuid)
    .bind(&name)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ServerFnError::new("secret not found"));
    }

    crate::api::push::notify_global(uuid, crate::api::push::PushMessage::SyncConfig).await;
    Ok(())
}

#[server]
async fn delete_secret(cluster_id: String, name: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
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

    let result =
        sqlx::query("DELETE FROM cluster_secrets WHERE cluster_id = $1 AND name = $2")
            .bind(uuid)
            .bind(&name)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ServerFnError::new("secret not found"));
    }

    crate::api::push::notify_global(uuid, crate::api::push::PushMessage::SyncConfig).await;
    Ok(())
}

// ── Page component ──────────────────────────────────────────────────────

#[component]
pub fn ClusterConfigPage(id: String) -> Element {
    let cid_name = id.clone();
    let cluster_name = use_server_future(move || {
        let cid = cid_name.clone();
        async move { get_cluster_name(cid).await }
    })?;

    let cid_write = id.clone();
    let write_check = use_server_future(move || {
        let cid = cid_write.clone();
        async move { can_write_cluster(cid).await }
    })?;

    let can_write = matches!(&*write_check.read(), Some(Ok(true)));
    let read_only = !can_write;

    let name = match &*cluster_name.read() {
        Some(Ok(n)) => n.clone(),
        _ => String::new(),
    };

    rsx! {
        div { class: "mb-4",
            Link {
                to: Route::ClusterDetail { id: id.clone() },
                class: "text-blue-600 dark:text-blue-400 hover:underline text-sm",
                {t!("cluster-config-back")}
            }
            if !name.is_empty() {
                h2 { class: "text-2xl font-bold mt-1", "{name} — {t!(\"cluster-detail-tab-config\")}" }
            }
        }

        div { class: "space-y-6",
            div {
                h3 { class: "text-lg font-semibold mb-3", {t!("cluster-detail-tab-config")} }
                ConfigEditor { cluster_id: id.clone(), read_only }
            }
            div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                div {
                    h3 { class: "text-lg font-semibold mb-3", {t!("secrets-title")} }
                    p { class: "text-sm text-gray-500 dark:text-gray-400 mb-3",
                        {t!("secrets-description")}
                    }
                    SecretsEditor { cluster_id: id.clone(), read_only }
                }
                div {
                    h3 { class: "text-lg font-semibold mb-3", {t!("cluster-detail-tab-config-history")} }
                    ConfigHistory { cluster_id: id.clone() }
                }
            }
        }
    }
}

// ── Secrets editor component ────────────────────────────────────────────

#[component]
fn SecretsEditor(cluster_id: String, read_only: bool) -> Element {
    let cid = cluster_id.clone();
    let mut secrets = use_server_future(move || {
        let cid = cid.clone();
        async move { list_secrets(cid).await }
    })?;

    let mut error = use_signal(|| None::<String>);
    let mut new_name = use_signal(String::new);
    let mut new_value = use_signal(String::new);
    let mut editing: Signal<Option<String>> = use_signal(|| None);
    let mut edit_value = use_signal(String::new);
    let mut confirm_delete: Signal<Option<String>> = use_signal(|| None);

    let entries = match &*secrets.read() {
        Some(Ok(list)) => list.clone(),
        Some(Err(e)) => {
            return rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", {t!("error-message", message: e.to_string())} } };
        }
        None => {
            return rsx! { p { class: "text-sm", {t!("loading")} } };
        }
    };

    let cid_create = cluster_id.clone();
    let do_create = move || {
        let cid = cid_create.clone();
        let name = new_name.read().trim().to_string();
        let value = new_value.read().clone();
        if name.is_empty() || value.is_empty() {
            return;
        }
        spawn(async move {
            match create_secret(cid, name, value).await {
                Ok(()) => {
                    error.set(None);
                    new_name.set(String::new());
                    new_value.set(String::new());
                    secrets.restart();
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    };

    rsx! {
        if let Some(err) = &*error.read() {
            p { class: "text-red-600 dark:text-red-400 text-sm mb-2", "{err}" }
        }

        if entries.is_empty() {
            p { class: "text-sm text-gray-500 dark:text-gray-400 mb-3", {t!("secrets-empty")} }
        } else {
            div { class: "border border-gray-300 dark:border-gray-600 rounded overflow-hidden mb-3",
                table { class: "w-full text-sm",
                    thead {
                        tr { class: "bg-gray-100 dark:bg-gray-700 text-left",
                            th { class: "px-3 py-2 font-medium", {t!("secrets-col-name")} }
                            th { class: "px-3 py-2 font-medium", {t!("secrets-col-reference")} }
                            th { class: "px-3 py-2 font-medium", {t!("secrets-col-created")} }
                            if !read_only {
                                th { class: "px-3 py-2 font-medium w-32", {t!("secrets-col-actions")} }
                            }
                        }
                    }
                    tbody {
                        for entry in &entries {
                            {
                                let name = entry.name.clone();
                                let created = entry.created_at.clone();
                                let reference = format!("secret:{name}");
                                let name_edit = name.clone();
                                let name_del = name.clone();
                                let name_del2 = name.clone();
                                let cid_upd = cluster_id.clone();
                                let cid_del = cluster_id.clone();
                                let is_editing = editing.read().as_deref() == Some(name.as_str());
                                let is_confirming = confirm_delete.read().as_deref() == Some(name.as_str());

                                rsx! {
                                    tr { class: "border-t border-gray-200 dark:border-gray-600",
                                        key: "{name}",
                                        td { class: "px-3 py-2 font-mono", "{name}" }
                                        td { class: "px-3 py-2 font-mono text-gray-500 dark:text-gray-400 select-all", "{reference}" }
                                        td { class: "px-3 py-2 text-gray-500 dark:text-gray-400", "{created}" }
                                        if !read_only {
                                            td { class: "px-3 py-2",
                                                if is_editing {
                                                    div { class: "flex gap-1",
                                                        input {
                                                            r#type: "password",
                                                            class: "border border-gray-300 dark:border-gray-600 rounded px-2 py-0.5 text-sm dark:bg-gray-700 dark:text-white w-32",
                                                            placeholder: t!("secrets-new-value"),
                                                            value: "{edit_value}",
                                                            oninput: move |e| edit_value.set(e.value()),
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "text-green-600 dark:text-green-400 hover:text-green-800 text-xs",
                                                            onclick: move |_| {
                                                                let cid = cid_upd.clone();
                                                                let n = name_edit.clone();
                                                                let v = edit_value.read().clone();
                                                                spawn(async move {
                                                                    match update_secret(cid, n, v).await {
                                                                        Ok(()) => {
                                                                            error.set(None);
                                                                            editing.set(None);
                                                                            edit_value.set(String::new());
                                                                            secrets.restart();
                                                                        }
                                                                        Err(e) => error.set(Some(e.to_string())),
                                                                    }
                                                                });
                                                            },
                                                            {t!("save")}
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "text-gray-500 dark:text-gray-400 text-xs",
                                                            onclick: move |_| {
                                                                editing.set(None);
                                                                edit_value.set(String::new());
                                                            },
                                                            {t!("cancel")}
                                                        }
                                                    }
                                                } else if is_confirming {
                                                    div { class: "flex gap-1 items-center",
                                                        span { class: "text-red-600 dark:text-red-400 text-xs", {t!("secrets-confirm-delete")} }
                                                        button {
                                                            r#type: "button",
                                                            class: "text-red-600 dark:text-red-400 hover:text-red-800 text-xs font-semibold",
                                                            onclick: move |_| {
                                                                let cid = cid_del.clone();
                                                                let n = name_del.clone();
                                                                spawn(async move {
                                                                    match delete_secret(cid, n).await {
                                                                        Ok(()) => {
                                                                            error.set(None);
                                                                            confirm_delete.set(None);
                                                                            secrets.restart();
                                                                        }
                                                                        Err(e) => error.set(Some(e.to_string())),
                                                                    }
                                                                });
                                                            },
                                                            {t!("delete")}
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "text-gray-500 dark:text-gray-400 text-xs",
                                                            onclick: move |_| confirm_delete.set(None),
                                                            {t!("cancel")}
                                                        }
                                                    }
                                                } else {
                                                    div { class: "flex gap-2",
                                                        button {
                                                            r#type: "button",
                                                            class: "text-blue-600 dark:text-blue-400 hover:underline text-xs",
                                                            onclick: move |_| {
                                                                editing.set(Some(name.clone()));
                                                                edit_value.set(String::new());
                                                            },
                                                            {t!("secrets-update")}
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "text-red-500 hover:text-red-700 text-xs",
                                                            onclick: move |_| {
                                                                confirm_delete.set(Some(name_del2.clone()));
                                                            },
                                                            {t!("delete")}
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if !read_only {
            div { class: "border border-gray-300 dark:border-gray-600 rounded p-3",
                h4 { class: "text-sm font-semibold mb-2", {t!("secrets-add")} }
                div { class: "flex gap-2 items-end flex-wrap",
                    div { class: "flex flex-col gap-1",
                        label { class: "text-xs text-gray-600 dark:text-gray-300", {t!("secrets-col-name")} }
                        input {
                            r#type: "text",
                            class: "border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm dark:bg-gray-700 dark:text-white w-48",
                            placeholder: "MY_API_KEY",
                            value: "{new_name}",
                            oninput: move |e| new_name.set(e.value()),
                        }
                    }
                    div { class: "flex flex-col gap-1",
                        label { class: "text-xs text-gray-600 dark:text-gray-300", {t!("secrets-col-value")} }
                        input {
                            r#type: "password",
                            class: "border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm dark:bg-gray-700 dark:text-white w-64",
                            placeholder: t!("secrets-value-placeholder"),
                            value: "{new_value}",
                            oninput: move |e| new_value.set(e.value()),
                        }
                    }
                    button {
                        r#type: "button",
                        class: "bg-green-600 text-white px-3 py-1 rounded text-sm hover:bg-green-700",
                        onclick: move |_| do_create(),
                        {t!("secrets-add")}
                    }
                }
            }
        }
    }
}
