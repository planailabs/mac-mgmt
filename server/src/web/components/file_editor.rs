use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use crate::web::user::current_user;

// ── Wire types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListResult {
    pub entries: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadResult {
    pub content: String,
    pub mtime: i64,
    pub size: u64,
    pub is_binary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteResult {
    pub status: u16,
    pub mtime: Option<i64>,
    pub error: Option<String>,
}

// ── Server functions ────────────────────────────────────────────────────

/// Resolve the relay API URL and a short-lived proxy token for a given instance.
/// Uses `relay_proxy_url` from the daemon's heartbeat (full URL with scheme and
/// port), so no extra `[relay]` config section is needed on the server.
#[cfg(feature = "server")]
async fn resolve_relay(
    pool: &sqlx::PgPool,
    instance_id: &str,
    cluster_id: uuid::Uuid,
) -> Result<(String, String), ServerFnError> {
    // Get the relay proxy URL from the heartbeat.
    let relay_url: Option<String> = sqlx::query_scalar(
        "SELECT relay_proxy_url FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
    )
    .bind(instance_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("instance not found"))?;

    let relay_url = relay_url
        .ok_or_else(|| ServerFnError::new("daemon has no relay proxy URL — relay may be outdated"))?;

    // Create a short-lived proxy token.
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(5);

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at) \
         VALUES ($1, $2, 'file-tunnel', 'proxy', $3)",
    )
    .bind(cluster_id)
    .bind(&hash)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok((relay_url, raw_token))
}

/// List files in a file tunnel (directory listing or single-file metadata).
#[server]
pub async fn file_tunnel_list(
    instance_id: String,
    tunnel_name: String,
    path: Option<String>,
) -> Result<FileListResult, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    let cluster_id: uuid::Uuid = sqlx::query_scalar(
        "SELECT cluster_id FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
    )
    .bind(&instance_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("instance not found"))?;

    user.require_cluster_read(&pool, cluster_id).await?;

    let (relay_url, token) = resolve_relay(&pool, &instance_id, cluster_id).await?;

    let mut url = format!("{relay_url}/api/daemon/{instance_id}/files/{tunnel_name}");
    if let Some(ref p) = path {
        url = format!("{url}?path={}", urlencoding::encode(p));
    }

    let resp = reqwest::Client::new()
        .get(&url)
        .bearer_auth(&token)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ServerFnError::new(format!("relay error: {body}")));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let entries: Vec<FileEntry> = body["entries"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|e| serde_json::from_value(e.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    Ok(FileListResult { entries })
}

/// Read a file's contents from a file tunnel.
#[server]
pub async fn file_tunnel_read(
    instance_id: String,
    tunnel_name: String,
    path: Option<String>,
) -> Result<FileReadResult, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    let cluster_id: uuid::Uuid = sqlx::query_scalar(
        "SELECT cluster_id FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
    )
    .bind(&instance_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("instance not found"))?;

    user.require_cluster_read(&pool, cluster_id).await?;

    let (relay_url, token) = resolve_relay(&pool, &instance_id, cluster_id).await?;

    let mut url = format!("{relay_url}/api/daemon/{instance_id}/files/{tunnel_name}/read");
    if let Some(ref p) = path {
        url = format!("{url}?path={}", urlencoding::encode(p));
    }

    let resp = reqwest::Client::new()
        .get(&url)
        .bearer_auth(&token)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ServerFnError::new(format!("relay error: {body}")));
    }

    let mtime: i64 = resp
        .headers()
        .get("x-file-mtime")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let size: u64 = resp
        .headers()
        .get("x-file-size")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if bytes.len() > 10 * 1024 * 1024 {
        return Err(ServerFnError::new("file too large for editor (>10MB)"));
    }

    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => Ok(FileReadResult {
            content: text,
            mtime,
            size,
            is_binary: false,
        }),
        Err(_) => {
            use base64::Engine;
            Ok(FileReadResult {
                content: base64::engine::general_purpose::STANDARD.encode(&bytes),
                mtime,
                size,
                is_binary: true,
            })
        }
    }
}

/// Write a file's contents to a file tunnel.
#[server]
pub async fn file_tunnel_write(
    instance_id: String,
    tunnel_name: String,
    path: Option<String>,
    content: String,
    is_binary: bool,
    expected_mtime: Option<i64>,
) -> Result<FileWriteResult, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    let cluster_id: uuid::Uuid = sqlx::query_scalar(
        "SELECT cluster_id FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
    )
    .bind(&instance_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("instance not found"))?;

    user.require_cluster_write(&pool, cluster_id).await?;

    let (relay_url, token) = resolve_relay(&pool, &instance_id, cluster_id).await?;

    let mut url = format!("{relay_url}/api/daemon/{instance_id}/files/{tunnel_name}/write");
    let mut query_parts = Vec::new();
    if let Some(ref p) = path {
        query_parts.push(format!("path={}", urlencoding::encode(p)));
    }
    if let Some(mt) = expected_mtime {
        query_parts.push(format!("expected_mtime={mt}"));
    }
    if !query_parts.is_empty() {
        url = format!("{url}?{}", query_parts.join("&"));
    }

    let body_bytes: Vec<u8> = if is_binary {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(&content)
            .map_err(|e| ServerFnError::new(format!("invalid base64: {e}")))?
    } else {
        content.into_bytes()
    };

    let resp = reqwest::Client::new()
        .post(&url)
        .bearer_auth(&token)
        .body(body_bytes)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let result: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(FileWriteResult {
        status: result["status"].as_u64().unwrap_or(500) as u16,
        mtime: result["mtime"].as_i64(),
        error: result["error"].as_str().map(String::from),
    })
}

// ── UI Components ──────────────────────────────────────────────────────

/// File editor panel embedded in the fleet detail page.
/// Shows available file tunnels on the left and an editor on the right.
#[component]
pub fn FileEditorPanel(instance_id: String, file_tunnels: Vec<serde_json::Value>) -> Element {
    let mut selected_tunnel = use_signal(|| Option::<String>::None);
    let mut selected_path = use_signal(|| Option::<String>::None);
    let mut editor_content = use_signal(|| String::new());
    let mut editor_mtime = use_signal(|| 0i64);
    let mut editor_dirty = use_signal(|| false);
    let mut editor_loading = use_signal(|| false);
    let mut editor_error = use_signal(|| Option::<String>::None);
    let mut editor_is_binary = use_signal(|| false);
    let mut save_status = use_signal(|| Option::<String>::None);

    let instance_id_sig = use_signal(|| instance_id.clone());

    // Save file content
    let save_file = move |_| {
        let tunnel_name = selected_tunnel.read().clone();
        let path = selected_path.read().clone();
        let content = editor_content.read().clone();
        let mtime = *editor_mtime.read();
        let is_binary = *editor_is_binary.read();
        let iid = instance_id_sig.read().clone();

        if let Some(tunnel_name) = tunnel_name {
            spawn(async move {
                editor_loading.set(true);
                save_status.set(None);
                match file_tunnel_write(iid, tunnel_name, path, content, is_binary, Some(mtime))
                    .await
                {
                    Ok(result) => {
                        if result.status == 200 {
                            if let Some(new_mtime) = result.mtime {
                                editor_mtime.set(new_mtime);
                            }
                            editor_dirty.set(false);
                            save_status.set(Some("Saved".into()));
                        } else if result.status == 409 {
                            save_status.set(Some(
                                "Conflict: file changed on disk. Reload and retry.".into(),
                            ));
                        } else {
                            save_status
                                .set(Some(result.error.unwrap_or_else(|| "Save failed".into())));
                        }
                    }
                    Err(e) => {
                        save_status.set(Some(e.to_string()));
                    }
                }
                editor_loading.set(false);
            });
        }
    };

    rsx! {
        div { class: "mb-6",
            h3 { class: "text-lg font-semibold mb-2", "Configuration Files" }
            div { class: "grid grid-cols-1 lg:grid-cols-3 gap-4",
                // Left panel: file tree
                div { class: "lg:col-span-1 bg-white dark:bg-gray-800 rounded shadow p-4 max-h-96 overflow-y-auto",
                    for ft in &file_tunnels {
                        {
                            let name = ft["name"].as_str().unwrap_or("").to_string();
                            let service = ft["service"].as_str().unwrap_or("").to_string();
                            let description = ft["description"].as_str().unwrap_or("").to_string();
                            let writable = ft["writable"].as_bool().unwrap_or(false);
                            let kind = ft["kind"].as_str().unwrap_or("file").to_string();
                            let is_selected = selected_tunnel.read().as_deref() == Some(&name);

                            let bg = if is_selected {
                                "bg-blue-50 dark:bg-blue-900/30 border-blue-300 dark:border-blue-700"
                            } else {
                                "hover:bg-gray-50 dark:hover:bg-gray-700 border-transparent"
                            };

                            let name_click = name.clone();
                            let kind_click = kind.clone();
                            rsx! {
                                button {
                                    class: "w-full text-left p-2 rounded border mb-1 {bg}",
                                    onclick: move |_| {
                                        let n = name_click.clone();
                                        selected_tunnel.set(Some(n.clone()));
                                        if kind_click == "file" {
                                            selected_path.set(None);
                                            let iid = instance_id_sig.read().clone();
                                            let tn = n;
                                            spawn(async move {
                                                editor_loading.set(true);
                                                editor_error.set(None);
                                                save_status.set(None);
                                                match file_tunnel_read(iid, tn, None).await {
                                                    Ok(result) => {
                                                        editor_content.set(result.content);
                                                        editor_mtime.set(result.mtime);
                                                        editor_is_binary.set(result.is_binary);
                                                        editor_dirty.set(false);
                                                    }
                                                    Err(e) => {
                                                        editor_error.set(Some(e.to_string()));
                                                    }
                                                }
                                                editor_loading.set(false);
                                            });
                                        } else {
                                            selected_path.set(None);
                                            editor_content.set(String::new());
                                            editor_error.set(Some("Select a file from the directory listing".into()));
                                        }
                                    },
                                    div { class: "flex items-center justify-between",
                                        div {
                                            span { class: "font-medium text-sm", "{name}" }
                                            if !writable {
                                                span { class: "ml-2 text-xs px-1 py-0.5 bg-gray-200 dark:bg-gray-600 rounded", "read-only" }
                                            }
                                        }
                                        span { class: "text-xs text-gray-500", "{service}" }
                                    }
                                    if !description.is_empty() {
                                        p { class: "text-xs text-gray-500 mt-1", "{description}" }
                                    }
                                }
                            }
                        }
                    }
                    if file_tunnels.is_empty() {
                        p { class: "text-sm text-gray-500 italic", "No configuration files available" }
                    }
                }

                // Right panel: editor
                div { class: "lg:col-span-2 bg-white dark:bg-gray-800 rounded shadow p-4",
                    if selected_tunnel.read().is_none() {
                        p { class: "text-sm text-gray-500 italic", "Select a file to view or edit" }
                    } else {
                        // Header bar
                        div { class: "flex items-center justify-between mb-2",
                            div {
                                span { class: "font-medium text-sm",
                                    "{selected_tunnel.read().as_deref().unwrap_or(\"\")}"
                                }
                                if let Some(ref p) = *selected_path.read() {
                                    span { class: "text-gray-500 text-xs ml-2", "/{p}" }
                                }
                            }
                            div { class: "flex items-center gap-2",
                                if *editor_dirty.read() {
                                    span { class: "text-xs text-amber-600", "unsaved changes" }
                                }
                                if let Some(ref status) = *save_status.read() {
                                    span { class: "text-xs", "{status}" }
                                }
                                button {
                                    class: "px-3 py-1 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50",
                                    disabled: !*editor_dirty.read() || *editor_loading.read() || *editor_is_binary.read(),
                                    onclick: save_file,
                                    if *editor_loading.read() { "Saving..." } else { "Save" }
                                }
                            }
                        }

                        // Error display
                        if let Some(ref err) = *editor_error.read() {
                            div { class: "mb-2 p-2 bg-red-50 dark:bg-red-900/30 border border-red-200 dark:border-red-800 rounded text-sm text-red-700 dark:text-red-300",
                                "{err}"
                            }
                        }

                        // Editor area
                        if *editor_loading.read() && editor_content.read().is_empty() {
                            div { class: "flex items-center justify-center h-64",
                                span { class: "text-gray-500", "Loading..." }
                            }
                        } else if *editor_is_binary.read() {
                            div { class: "flex items-center justify-center h-64 text-gray-500",
                                "Binary file — download to view"
                            }
                        } else {
                            textarea {
                                class: "w-full h-96 font-mono text-sm p-2 border rounded bg-gray-50 dark:bg-gray-900 dark:border-gray-600 resize-y",
                                spellcheck: false,
                                value: "{editor_content}",
                                oninput: move |e| {
                                    editor_content.set(e.value());
                                    editor_dirty.set(true);
                                    save_status.set(None);
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}
