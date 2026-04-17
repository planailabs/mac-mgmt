use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use crate::web::user::current_user;

// ── Wire types ─────────────────────────────────────────────────────────

/// Returned by the single server function — relay URL + short-lived token.
/// The browser uses these to call the relay file API directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEditorContext {
    pub relay_url: String,
    pub proxy_token: String,
    pub instance_id: String,
}

// ── Server function ─────────────────────────────────────────────────────

/// Mint a short-lived proxy token and return the relay URL.
/// This is the ONLY server function — all file I/O goes browser → relay directly.
#[server]
pub async fn get_file_editor_context(
    instance_id: String,
) -> Result<FileEditorContext, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    // Resolve cluster + relay URL from the heartbeat.
    #[derive(sqlx::FromRow)]
    struct HbInfo {
        cluster_id: uuid::Uuid,
        relay_proxy_url: Option<String>,
    }
    let hb: HbInfo = sqlx::query_as(
        "SELECT cluster_id, relay_proxy_url FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
    )
    .bind(&instance_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("instance not found"))?;

    user.require_cluster_read(&pool, hb.cluster_id).await?;

    let relay_url = hb
        .relay_proxy_url
        .ok_or_else(|| ServerFnError::new("daemon has no relay proxy URL"))?;

    // Mint a short-lived proxy token (5 minutes).
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(6);

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at) \
         VALUES ($1, $2, 'file-tunnel', 'proxy', $3)",
    )
    .bind(hb.cluster_id)
    .bind(&hash)
    .bind(expires_at)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(FileEditorContext {
        relay_url,
        proxy_token: raw_token,
        instance_id,
    })
}

// ── Client-side relay calls via JS fetch ────────────────────────────────

/// Call the relay file list API directly from the browser.
async fn relay_file_list(
    relay_url: &str,
    token: &str,
    instance_prefix: &str,
    tunnel_name: &str,
    path: Option<&str>,
) -> Result<serde_json::Value, String> {
    let mut url = format!(
        "https://{instance_prefix}.{relay_host}/api/files/{tunnel_name}",
        relay_host = relay_url.trim_start_matches("https://").trim_start_matches("http://"),
    );
    if let Some(p) = path {
        url = format!("{url}?path={p}");
    }
    let js = format!(
        r#"
        const resp = await fetch("{url}", {{
            headers: {{ "Authorization": "Bearer {token}" }}
        }});
        const body = await resp.text();
        return body;
        "#,
    );
    let result: serde_json::Value = document::eval(&js).await.map_err(|e| format!("{e}"))?;
    let text: String = result.as_str().unwrap_or("").to_string();
    serde_json::from_str(&text).map_err(|e| format!("parse error: {e}"))
}

/// Read a file from the relay directly from the browser.
async fn relay_file_read(
    relay_url: &str,
    token: &str,
    instance_prefix: &str,
    tunnel_name: &str,
    path: Option<&str>,
) -> Result<(String, i64, u64, bool), String> {
    let mut url = format!(
        "https://{instance_prefix}.{relay_host}/api/files/{tunnel_name}/read",
        relay_host = relay_url.trim_start_matches("https://").trim_start_matches("http://"),
    );
    if let Some(p) = path {
        url = format!("{url}?path={p}");
    }
    let js = format!(
        r#"
        const resp = await fetch("{url}", {{
            headers: {{ "Authorization": "Bearer {token}" }}
        }});
        if (!resp.ok) {{
            const err = await resp.text();
            return JSON.stringify({{ error: err, status: resp.status }});
        }}
        const mtime = parseInt(resp.headers.get("x-file-mtime") || "0");
        const size = parseInt(resp.headers.get("x-file-size") || "0");
        const blob = await resp.blob();
        let content;
        let is_binary = false;
        try {{
            content = await blob.text();
            // Check if it's valid UTF-8 by round-tripping
            const encoder = new TextEncoder();
            const decoder = new TextDecoder("utf-8", {{ fatal: true }});
            decoder.decode(encoder.encode(content));
        }} catch(e) {{
            // Binary file — base64 encode
            const buf = await blob.arrayBuffer();
            const bytes = new Uint8Array(buf);
            let binary = "";
            for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
            content = btoa(binary);
            is_binary = true;
        }}
        return JSON.stringify({{ content, mtime, size, is_binary }});
        "#,
    );
    let result: serde_json::Value = document::eval(&js).await.map_err(|e| format!("{e}"))?;
    let text: String = result.as_str().unwrap_or("").to_string();
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("parse: {e}"))?;
    if let Some(err) = v.get("error") {
        return Err(err.as_str().unwrap_or("unknown error").to_string());
    }
    Ok((
        v["content"].as_str().unwrap_or("").to_string(),
        v["mtime"].as_i64().unwrap_or(0),
        v["size"].as_u64().unwrap_or(0),
        v["is_binary"].as_bool().unwrap_or(false),
    ))
}

/// Write a file to the relay directly from the browser.
async fn relay_file_write(
    relay_url: &str,
    token: &str,
    instance_prefix: &str,
    tunnel_name: &str,
    path: Option<&str>,
    content: &str,
    expected_mtime: Option<i64>,
) -> Result<serde_json::Value, String> {
    let mut url = format!(
        "https://{instance_prefix}.{relay_host}/api/files/{tunnel_name}/write",
        relay_host = relay_url.trim_start_matches("https://").trim_start_matches("http://"),
    );
    let mut query_parts = Vec::new();
    if let Some(p) = path {
        query_parts.push(format!("path={p}"));
    }
    if let Some(mt) = expected_mtime {
        query_parts.push(format!("expected_mtime={mt}"));
    }
    if !query_parts.is_empty() {
        url = format!("{url}?{}", query_parts.join("&"));
    }
    // Escape content for JS string embedding
    let escaped = content.replace('\\', "\\\\").replace('`', "\\`").replace('$', "\\$");
    let js = format!(
        r#"
        const resp = await fetch("{url}", {{
            method: "POST",
            headers: {{
                "Authorization": "Bearer {token}",
                "Content-Type": "application/octet-stream"
            }},
            body: `{escaped}`
        }});
        const body = await resp.text();
        return body;
        "#,
    );
    let result: serde_json::Value = document::eval(&js).await.map_err(|e| format!("{e}"))?;
    let text: String = result.as_str().unwrap_or("").to_string();
    serde_json::from_str(&text).map_err(|e| format!("parse: {e}"))
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

    // Fetch relay context (URL + token) once on mount
    let ctx = use_server_future(move || {
        let iid = instance_id.clone();
        async move { get_file_editor_context(iid).await }
    })?;

    let ctx_data = match &*ctx.read() {
        Some(Ok(c)) => Some(c.clone()),
        Some(Err(e)) => {
            return rsx! {
                div { class: "mb-6 p-3 bg-red-50 dark:bg-red-900/30 rounded text-sm text-red-700",
                    "File editor unavailable: {e}"
                }
            };
        }
        None => None,
    };

    let Some(ctx_data) = ctx_data else {
        return rsx! {
            div { class: "mb-6 text-sm text-gray-500", "Loading file editor..." }
        };
    };

    let relay_url = use_signal(|| ctx_data.relay_url.clone());
    let proxy_token = use_signal(|| ctx_data.proxy_token.clone());
    let instance_prefix = use_signal(|| {
        let iid = &ctx_data.instance_id;
        if iid.len() >= 12 { iid[..12].to_string() } else { iid.clone() }
    });

    // Save handler
    let save_file = move |_| {
        let tunnel_name = selected_tunnel.read().clone();
        let path = selected_path.read().clone();
        let content = editor_content.read().clone();
        let mtime = *editor_mtime.read();
        let ru = relay_url.read().clone();
        let tok = proxy_token.read().clone();
        let prefix = instance_prefix.read().clone();

        if let Some(tunnel_name) = tunnel_name {
            spawn(async move {
                editor_loading.set(true);
                save_status.set(None);
                match relay_file_write(
                    &ru, &tok, &prefix, &tunnel_name,
                    path.as_deref(), &content, Some(mtime),
                ).await {
                    Ok(result) => {
                        let status = result["status"].as_u64().unwrap_or(500);
                        if status == 200 {
                            if let Some(new_mtime) = result["mtime"].as_i64() {
                                editor_mtime.set(new_mtime);
                            }
                            editor_dirty.set(false);
                            save_status.set(Some("Saved".into()));
                        } else if status == 409 {
                            save_status.set(Some("Conflict: file changed on disk. Reload and retry.".into()));
                        } else {
                            let err = result["error"].as_str().unwrap_or("Save failed");
                            save_status.set(Some(err.to_string()));
                        }
                    }
                    Err(e) => save_status.set(Some(e)),
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
                                            let ru = relay_url.read().clone();
                                            let tok = proxy_token.read().clone();
                                            let prefix = instance_prefix.read().clone();
                                            let tn = n;
                                            spawn(async move {
                                                editor_loading.set(true);
                                                editor_error.set(None);
                                                save_status.set(None);
                                                match relay_file_read(&ru, &tok, &prefix, &tn, None).await {
                                                    Ok((content, mtime, _size, is_binary)) => {
                                                        editor_content.set(content);
                                                        editor_mtime.set(mtime);
                                                        editor_is_binary.set(is_binary);
                                                        editor_dirty.set(false);
                                                    }
                                                    Err(e) => editor_error.set(Some(e)),
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

                        if let Some(ref err) = *editor_error.read() {
                            div { class: "mb-2 p-2 bg-red-50 dark:bg-red-900/30 border border-red-200 dark:border-red-800 rounded text-sm text-red-700 dark:text-red-300",
                                "{err}"
                            }
                        }

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
