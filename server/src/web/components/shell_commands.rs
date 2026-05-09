use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::components::ui::{ErrorText, HelpText};
#[cfg(feature = "server")]
use crate::web::user::{current_user, WebUserExt};

// ── Wire types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellContext {
    pub relay_url: String,
    pub proxy_token: String,
    pub instance_prefix: String,
    pub commands: Vec<serde_json::Value>,
}

// ── Server function ─────────────────────────────────────────────────────

#[server]
pub async fn get_shell_context(instance_id: String) -> Result<ShellContext, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct HbInfo {
        cluster_id: uuid::Uuid,
        relay_proxy_url: Option<String>,
        shell_tunnels: serde_json::Value,
    }
    let hb: HbInfo = sqlx::query_as(
        "SELECT cluster_id, relay_proxy_url, shell_tunnels FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
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

    // Mint a 6-hour proxy token.
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(6);

    let scopes = serde_json::json!(["shell:exec"]);
    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at, scopes) \
         VALUES ($1, $2, 'shell-tunnel', 'proxy', $3, $4)",
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

    let commands = hb.shell_tunnels.as_array().cloned().unwrap_or_default();

    Ok(ShellContext {
        relay_url,
        proxy_token: raw_token,
        instance_prefix,
        commands,
    })
}

// ── Client-side helpers ──────────────────────────────────────────────────

fn build_relay_shell_url(relay_url: &str, instance_prefix: &str, command_name: &str) -> String {
    let scheme = if relay_url.starts_with("https://") {
        "https://"
    } else {
        "http://"
    };
    let relay_host = relay_url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    format!("{scheme}{instance_prefix}.{relay_host}/api/shell/{command_name}/exec")
}

// ── Component ──────────────────────────────────────────────────────────

#[component]
pub fn FleetShell(instance_id: String) -> Element {
    let iid = instance_id.clone();
    let ctx = use_server_future(move || {
        let iid = iid.clone();
        async move { get_shell_context(iid).await }
    })?;

    match &*ctx.read() {
        Some(Ok(c)) => render_shell(c),
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}

fn render_shell(ctx: &ShellContext) -> Element {
    // Group commands by service
    let mut by_service: std::collections::BTreeMap<String, Vec<&serde_json::Value>> =
        std::collections::BTreeMap::new();
    for cmd in &ctx.commands {
        let svc = cmd
            .get("service")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        by_service.entry(svc).or_default().push(cmd);
    }

    let relay_url = ctx.relay_url.clone();
    let prefix = ctx.instance_prefix.clone();
    let token = ctx.proxy_token.clone();

    rsx! {
        h2 { class: "h-page", {t!("shell-title")} }

        for (service, cmds) in by_service.iter() {
            div { class: "mb-6",
                h3 { class: "h-section text-fg-strong",
                    "{service}"
                }
                div { class: "space-y-3",
                    for cmd in cmds.iter() {
                        {
                            let name = cmd.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let description = cmd.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let requires_arg = cmd.get("requires_arg").and_then(|v| v.as_bool()).unwrap_or(false);
                            let arg_label = cmd.get("arg_label").and_then(|v| v.as_str()).unwrap_or("Argument").to_string();
                            let arg_placeholder = cmd.get("arg_placeholder").and_then(|v| v.as_str()).unwrap_or("").to_string();

                            let url = build_relay_shell_url(&relay_url, &prefix, &name);
                            let tok = token.clone();

                            rsx! {
                                ShellCommandCard {
                                    name: name,
                                    description: description,
                                    requires_arg: requires_arg,
                                    arg_label: arg_label,
                                    arg_placeholder: arg_placeholder,
                                    exec_url: url,
                                    token: tok,
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ShellCommandCard(
    name: String,
    description: String,
    requires_arg: bool,
    arg_label: String,
    arg_placeholder: String,
    exec_url: String,
    token: String,
) -> Element {
    let mut output = use_signal(String::new);
    let mut running = use_signal(|| false);
    let mut arg_value = use_signal(String::new);

    let run_name = name.clone();

    let mut show_modal = use_signal(|| false);

    rsx! {
        div { class: "card p-3",
            div { class: "flex items-center gap-3 mb-2",
                button { class: "btn btn-sm btn-primary",
                    disabled: *running.read(),
                    onclick: {
                        let exec_url = exec_url.clone();
                        let token = token.clone();
                        move |_| {
                            let exec_url = exec_url.clone();
                            let token = token.clone();
                            let arg = arg_value.read().clone();
                            running.set(true);
                            output.set(String::new());
                            show_modal.set(true);
                            async move {
                                let user_arg = if arg.is_empty() { "null".to_string() } else { format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\"")) };
                                let body = format!("{{\"user_arg\":{user_arg}}}");
                                // Use SSE EventSource for real-time streaming.
                                // POST isn't supported by EventSource, so we use fetch + ReadableStream.
                                let js = format!(
                                    r#"
                                    try {{
                                        const resp = await fetch("{exec_url}", {{
                                            method: "POST",
                                            headers: {{
                                                "X-Proxy-Token": "{token}",
                                                "Content-Type": "application/json",
                                            }},
                                            body: '{body}',
                                        }});
                                        const reader = resp.body.getReader();
                                        const decoder = new TextDecoder();
                                        let result = "";
                                        let buf = "";
                                        while (true) {{
                                            const {{done, value}} = await reader.read();
                                            if (done) break;
                                            buf += decoder.decode(value, {{stream: true}});
                                            while (true) {{
                                                const idx = buf.indexOf("\n");
                                                if (idx === -1) break;
                                                const line = buf.slice(0, idx);
                                                buf = buf.slice(idx + 1);
                                                if (line.startsWith("data: ")) {{
                                                    try {{
                                                        const obj = JSON.parse(line.slice(6));
                                                        if (obj.data !== undefined) {{
                                                            result += obj.data + "\n";
                                                        }} else if (obj.exit_code !== undefined) {{
                                                            result += "\n[exit code: " + obj.exit_code + "]\n";
                                                        }} else if (obj.error !== undefined) {{
                                                            result += "\n[error: " + obj.error + "]\n";
                                                        }}
                                                    }} catch(e) {{}}
                                                }}
                                            }}
                                            // Update the modal output element in real-time.
                                            const el = document.getElementById("shell-output");
                                            if (el) {{
                                                el.textContent = result;
                                                el.scrollTop = el.scrollHeight;
                                            }}
                                        }}
                                        return result;
                                    }} catch(e) {{
                                        return "fetch error: " + e.message;
                                    }}
                                    "#,
                                );
                                match document::eval(&js).await {
                                    Ok(result) => {
                                        let text = result.as_str().unwrap_or("").to_string();
                                        output.set(text);
                                    }
                                    Err(e) => {
                                        output.set(format!("Error: {e}"));
                                    }
                                }
                                running.set(false);
                            }
                        }
                    },
                    if *running.read() { {t!("shell-running")} } else { {t!("shell-run")} }
                }
                span { class: "text-sm font-mono text-fg",
                    "{run_name}"
                }
                span { class: "text-xs text-fg-muted",
                    "{description}"
                }
            }
            if requires_arg {
                div { class: "flex items-center gap-2 mb-2",
                    label { class: "label", {t!("shell-arg-label", label: arg_label.clone())} }
                    input { class: "input input-sm w-auto",
                        r#type: "text",
                        placeholder: "{arg_placeholder}",
                        value: "{arg_value}",
                        oninput: move |e| arg_value.set(e.value()),
                    }
                }
            }
        }

        // Output modal
        if *show_modal.read() {
            div {
                class: "fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4",
                onclick: move |_| {
                    if !*running.read() {
                        show_modal.set(false);
                    }
                },
                div {
                    class: "bg-surface rounded-lg shadow-xl w-full max-w-3xl max-h-[80vh] flex flex-col",
                    onclick: move |e| e.stop_propagation(),
                    // Header
                    div { class: "flex items-center justify-between px-4 py-3 border-b border-line-soft",
                        h3 { class: "text-sm font-semibold text-fg-strong",
                            "{name}"
                            if *running.read() {
                                span { class: "ml-2 text-warn animate-pulse", "running..." }
                            }
                        }
                        button {
                            class: "text-fg-faint hover:text-fg-strong text-lg cursor-pointer",
                            disabled: *running.read(),
                            onclick: move |_| show_modal.set(false),
                            "x"
                        }
                    }
                    // Output
                    pre {
                        id: "shell-output",
                        class: "log-output flex-1 max-h-none rounded-none",
                        "{output}"
                    }
                }
            }
        }
    }
}
