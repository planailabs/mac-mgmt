use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use crate::web::user::current_user;

// ── Wire types ─────────────────────────────────────────────────────────

/// Returned by the server function — relay context + available file tunnels.
/// The token is only minted when this page is opened.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEditorContext {
    pub relay_url: String,
    pub proxy_token: String,
    pub instance_id: String,
    pub instance_prefix: String,
    pub file_tunnels: Vec<serde_json::Value>,
}

// ── Server function ─────────────────────────────────────────────────────

/// Mint a proxy token and return relay URL + file tunnel metadata.
/// Called only when the files page is opened.
#[server]
pub async fn get_file_editor_context(
    instance_id: String,
) -> Result<FileEditorContext, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct HbInfo {
        cluster_id: uuid::Uuid,
        relay_proxy_url: Option<String>,
        file_tunnels: serde_json::Value,
    }
    let hb: HbInfo = sqlx::query_as(
        "SELECT cluster_id, relay_proxy_url, file_tunnels FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
    )
    .bind(&instance_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("instance not found"))?;

    user.require_cluster_write(&pool, hb.cluster_id).await?;

    let relay_url = hb
        .relay_proxy_url
        .ok_or_else(|| ServerFnError::new("daemon has no relay proxy URL"))?;

    // Mint a 6-hour proxy token.
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(6);

    let scopes = serde_json::json!(["files:read", "files:write"]);
    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at, scopes) \
         VALUES ($1, $2, 'file-tunnel', 'proxy', $3, $4)",
    )
    .bind(hb.cluster_id)
    .bind(&hash)
    .bind(expires_at)
    .bind(&scopes)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let instance_prefix = if instance_id.len() >= 12 {
        instance_id[..12].to_string()
    } else {
        instance_id.clone()
    };

    let file_tunnels = hb.file_tunnels.as_array().cloned().unwrap_or_default();

    Ok(FileEditorContext {
        relay_url,
        proxy_token: raw_token,
        instance_id,
        instance_prefix,
        file_tunnels,
    })
}

// ── Client-side relay calls via JS fetch ────────────────────────────────

fn build_relay_file_url(
    relay_url: &str,
    instance_prefix: &str,
    tunnel_name: &str,
    suffix: &str,
) -> String {
    let scheme = if relay_url.starts_with("https://") {
        "https://"
    } else {
        "http://"
    };
    let relay_host = relay_url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    format!("{scheme}{instance_prefix}.{relay_host}/api/files/{tunnel_name}{suffix}")
}

async fn relay_file_list(
    relay_url: &str,
    token: &str,
    instance_prefix: &str,
    tunnel_name: &str,
    path: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    let mut url = build_relay_file_url(relay_url, instance_prefix, tunnel_name, "");
    if let Some(p) = path {
        url = format!("{url}?path={p}");
    }
    let js = format!(
        r#"
        const resp = await fetch("{url}", {{
            headers: {{ "X-Proxy-Token": "{token}" }}
        }});
        const body = await resp.text();
        return body;
        "#,
    );
    let result: serde_json::Value = document::eval(&js).await.map_err(|e| format!("{e}"))?;
    let text: String = result.as_str().unwrap_or("").to_string();
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("parse: {e}"))?;
    Ok(v["entries"].as_array().cloned().unwrap_or_default())
}

async fn relay_file_read(
    relay_url: &str,
    token: &str,
    instance_prefix: &str,
    tunnel_name: &str,
    path: Option<&str>,
) -> Result<(String, i64, u64, bool), String> {
    let mut url = build_relay_file_url(relay_url, instance_prefix, tunnel_name, "/read");
    if let Some(p) = path {
        url = format!("{url}?path={p}");
    }
    let js = format!(
        r#"
        const resp = await fetch("{url}", {{
            headers: {{ "X-Proxy-Token": "{token}" }}
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
            const encoder = new TextEncoder();
            const decoder = new TextDecoder("utf-8", {{ fatal: true }});
            decoder.decode(encoder.encode(content));
        }} catch(e) {{
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

async fn relay_file_write(
    relay_url: &str,
    token: &str,
    instance_prefix: &str,
    tunnel_name: &str,
    path: Option<&str>,
    content: &str,
    expected_mtime: Option<i64>,
) -> Result<serde_json::Value, String> {
    let mut url = build_relay_file_url(relay_url, instance_prefix, tunnel_name, "/write");
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
    let escaped = content
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('$', "\\$");
    let js = format!(
        r#"
        const resp = await fetch("{url}", {{
            method: "POST",
            headers: {{
                "X-Proxy-Token": "{token}",
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

// ── Page component ─────────────────────────────────────────────────────

/// Standalone page for editing configuration files on a daemon instance.
/// Route: /fleet/:instance_id/files
/// Token is minted only when this page loads.
#[component]
pub fn FleetFiles(instance_id: String) -> Element {
    let mut selected_tunnel = use_signal(|| Option::<String>::None);
    let mut selected_path = use_signal(|| Option::<String>::None);
    let mut editor_content = use_signal(|| String::new());
    let mut editor_mtime = use_signal(|| 0i64);
    let mut editor_dirty = use_signal(|| false);
    let mut editor_loading = use_signal(|| false);
    let mut editor_error = use_signal(|| Option::<String>::None);
    let mut editor_is_binary = use_signal(|| false);
    let mut save_status = use_signal(|| Option::<String>::None);
    // Directory expansion: tunnel_name → list of file entries
    let mut dir_entries: Signal<std::collections::HashMap<String, Vec<serde_json::Value>>> =
        use_signal(|| std::collections::HashMap::new());
    let mut dir_loading = use_signal(|| Option::<String>::None);

    let ctx = use_server_future(move || {
        let iid = instance_id.clone();
        async move { get_file_editor_context(iid).await }
    })?;

    let ctx_data = match &*ctx.read() {
        Some(Ok(c)) => Some(c.clone()),
        Some(Err(e)) => {
            return rsx! {
                div { class: "max-w-6xl mx-auto px-4 py-6",
                    div { class: "p-4 bg-red-50 dark:bg-red-900/30 rounded text-sm text-red-700 dark:text-red-300",
                        "File editor unavailable: {e}"
                    }
                }
            };
        }
        None => None,
    };

    let Some(ctx_data) = ctx_data else {
        return rsx! {
            div { class: "max-w-6xl mx-auto px-4 py-6",
                p { class: "text-sm text-gray-500", "Loading file editor..." }
            }
        };
    };

    let relay_url = use_signal(|| ctx_data.relay_url.clone());
    let proxy_token = use_signal(|| ctx_data.proxy_token.clone());
    let instance_prefix = use_signal(|| ctx_data.instance_prefix.clone());
    let file_tunnels = ctx_data.file_tunnels.clone();
    let back_url = format!("/fleet/{}", ctx_data.instance_id);

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
                    &ru,
                    &tok,
                    &prefix,
                    &tunnel_name,
                    path.as_deref(),
                    &content,
                    Some(mtime),
                )
                .await
                {
                    Ok(result) => {
                        let status = result["status"].as_u64().unwrap_or(500);
                        if status == 200 {
                            if let Some(new_mtime) = result["mtime"].as_i64() {
                                editor_mtime.set(new_mtime);
                            }
                            editor_dirty.set(false);
                            save_status.set(Some("Saved".into()));
                        } else if status == 409 {
                            save_status.set(Some(
                                "Conflict: file changed on disk. Reload and retry.".into(),
                            ));
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
        div { class: "max-w-6xl mx-auto px-4 py-6",
            // Header with back link
            div { class: "flex items-center gap-3 mb-4",
                Link {
                    to: back_url,
                    class: "text-sm text-blue-600 hover:text-blue-800 dark:text-blue-400",
                    "Back to instance"
                }
                h2 { class: "text-xl font-semibold", "Configuration Files" }
            }

            div { class: "mb-4 p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded text-sm text-amber-800 dark:text-amber-200",
                "Changes here apply to this instance only and are not synced across the cluster. "
                "Use the cluster configuration for settings that should be consistent across all instances."
            }

            div { class: "grid grid-cols-1 lg:grid-cols-3 gap-4",
                // Left panel: file tree
                div { class: "lg:col-span-1 bg-white dark:bg-gray-800 rounded shadow p-4 max-h-[calc(100vh-12rem)] overflow-y-auto",
                    for ft in &file_tunnels {
                        {
                            let name = ft["name"].as_str().unwrap_or("").to_string();
                            let service = ft["service"].as_str().unwrap_or("").to_string();
                            let description = ft["description"].as_str().unwrap_or("").to_string();
                            let writable = ft["writable"].as_bool().unwrap_or(false);
                            let kind = ft["kind"].as_str().unwrap_or("file").to_string();
                            let is_dir = kind == "directory";
                            let is_expanded = dir_entries.read().contains_key(&name);
                            let is_selected = selected_tunnel.read().as_deref() == Some(&name)
                                && selected_path.read().is_none();

                            let bg = if is_selected {
                                "bg-blue-50 dark:bg-blue-900/30 border-blue-300 dark:border-blue-700"
                            } else {
                                "hover:bg-gray-50 dark:hover:bg-gray-700 border-transparent"
                            };

                            let name_click = name.clone();
                            let name_entries = name.clone();
                            rsx! {
                                button {
                                    class: "w-full text-left p-2 rounded border mb-1 {bg}",
                                    onclick: move |_| {
                                        let n = name_click.clone();
                                        if is_dir {
                                            // Toggle directory expansion
                                            if dir_entries.read().contains_key(&n) {
                                                dir_entries.write().remove(&n);
                                            } else {
                                                let ru = relay_url.read().clone();
                                                let tok = proxy_token.read().clone();
                                                let prefix = instance_prefix.read().clone();
                                                let tn = n.clone();
                                                dir_loading.set(Some(tn.clone()));
                                                spawn(async move {
                                                    match relay_file_list(&ru, &tok, &prefix, &tn, None).await {
                                                        Ok(entries) => { dir_entries.write().insert(tn, entries); }
                                                        Err(e) => editor_error.set(Some(e)),
                                                    }
                                                    dir_loading.set(None);
                                                });
                                            }
                                        } else {
                                            // Single file tunnel — load directly
                                            selected_tunnel.set(Some(n.clone()));
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
                                        }
                                    },
                                    div { class: "flex items-center justify-between",
                                        div {
                                            if is_dir {
                                                span { class: "mr-1 text-xs",
                                                    if is_expanded { "v" } else { ">" }
                                                }
                                            }
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
                                // Expanded directory entries
                                if is_dir {
                                    if dir_loading.read().as_deref() == Some(&*name_entries) {
                                        div { class: "ml-4 py-1 text-xs text-gray-500", "Loading..." }
                                    }
                                    if let Some(entries) = dir_entries.read().get(&name_entries) {
                                        for entry in entries.iter() {
                                            {
                                                let fname = entry["name"].as_str().unwrap_or("").to_string();
                                                let fkind = entry["kind"].as_str().unwrap_or("file");
                                                let fsize = entry["size"].as_u64().unwrap_or(0);
                                                let tunnel_for_file = name_entries.clone();
                                                let fname_click = fname.clone();
                                                let is_file_selected = selected_tunnel.read().as_deref() == Some(&*tunnel_for_file)
                                                    && selected_path.read().as_deref() == Some(&*fname);
                                                let file_bg = if is_file_selected {
                                                    "bg-blue-50 dark:bg-blue-900/30"
                                                } else {
                                                    "hover:bg-gray-50 dark:hover:bg-gray-700"
                                                };
                                                if fkind == "file" {
                                                    rsx! {
                                                        button {
                                                            class: "w-full text-left ml-4 pl-2 py-1 rounded text-sm {file_bg}",
                                                            onclick: move |_| {
                                                                let tn = tunnel_for_file.clone();
                                                                let fp = fname_click.clone();
                                                                selected_tunnel.set(Some(tn.clone()));
                                                                selected_path.set(Some(fp.clone()));
                                                                let ru = relay_url.read().clone();
                                                                let tok = proxy_token.read().clone();
                                                                let prefix = instance_prefix.read().clone();
                                                                spawn(async move {
                                                                    editor_loading.set(true);
                                                                    editor_error.set(None);
                                                                    save_status.set(None);
                                                                    match relay_file_read(&ru, &tok, &prefix, &tn, Some(&fp)).await {
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
                                                            },
                                                            span { class: "text-gray-700 dark:text-gray-300", "{fname}" }
                                                            span { class: "ml-2 text-xs text-gray-400", "{fsize}B" }
                                                        }
                                                    }
                                                } else {
                                                    rsx! {
                                                        div { class: "ml-4 pl-2 py-1 text-sm text-gray-500",
                                                            "{fname}/"
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
                                class: "w-full h-[calc(100vh-16rem)] font-mono text-sm p-2 border rounded bg-gray-50 dark:bg-gray-900 dark:border-gray-600 resize-y",
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
