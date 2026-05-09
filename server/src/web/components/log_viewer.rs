use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::components::ui::{Button, ButtonSize, ButtonVariant, ErrorText, HelpText};
#[cfg(feature = "server")]
use crate::web::user::{current_user, WebUserExt};

// ── Wire types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsContext {
    pub relay_url: String,
    pub proxy_token: String,
    pub instance_prefix: String,
    pub services: Vec<String>,
}

// ── Server function ─────────────────────────────────────────────────────

#[server]
pub async fn get_logs_context(instance_id: String) -> Result<LogsContext, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct HbInfo {
        cluster_id: uuid::Uuid,
        relay_proxy_url: Option<String>,
        services: serde_json::Value,
    }
    let hb: HbInfo = sqlx::query_as(
        "SELECT cluster_id, relay_proxy_url, services FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
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

    let scopes = serde_json::json!(["logs:read"]);
    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at, scopes) \
         VALUES ($1, $2, 'log-viewer', 'proxy', $3, $4)",
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

    let services: Vec<String> = hb
        .services
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.get("name").and_then(|v| v.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(LogsContext {
        relay_url,
        proxy_token: raw_token,
        instance_prefix,
        services,
    })
}

// ── Client-side helpers ──────────────────────────────────────────────────

fn build_relay_logs_url(relay_url: &str, instance_prefix: &str) -> String {
    let scheme = if relay_url.starts_with("https://") {
        "https://"
    } else {
        "http://"
    };
    let relay_host = relay_url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    format!("{scheme}{instance_prefix}.{relay_host}/api/logs")
}

// ── Component ──────────────────────────────────────────────────────────

#[component]
pub fn FleetLogs(instance_id: String) -> Element {
    let iid = instance_id.clone();
    let ctx = use_server_future(move || {
        let iid = iid.clone();
        async move { get_logs_context(iid).await }
    })?;

    match &*ctx.read() {
        Some(Ok(c)) => render_logs(c),
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}

fn render_logs(ctx: &LogsContext) -> Element {
    let logs_url = build_relay_logs_url(&ctx.relay_url, &ctx.instance_prefix);
    let token = ctx.proxy_token.clone();
    let services = ctx.services.clone();

    let mut log_output = use_signal(String::new);
    let mut selected_service = use_signal(|| "".to_string());
    let mut polling = use_signal(|| false);

    rsx! {
        h2 { class: "h-page", {t!("log-title")} }

        div { class: "flex items-center gap-3 mb-4",
            // Service filter
            select { class: "input input-sm w-auto",
                value: "{selected_service}",
                onchange: move |e| selected_service.set(e.value()),
                option { value: "", {t!("log-all-services")} }
                for svc in services.iter() {
                    option { value: "{svc}", "{svc}" }
                }
            }

            // Load / Poll toggle
            button { class: "btn btn-sm btn-primary",
                onclick: {
                    let logs_url = logs_url.clone();
                    let token = token.clone();
                    move |_| {
                        let logs_url = logs_url.clone();
                        let token = token.clone();
                        let is_polling = *polling.read();
                        async move {
                            if is_polling {
                                polling.set(false);
                                return;
                            }
                            polling.set(true);
                            let svc = selected_service.read().clone();
                            let svc_param = if svc.is_empty() { String::new() } else { format!("&service={svc}") };
                            let js = format!(
                                r#"
                                let after = 0;
                                let result = "";
                                // Fetch latest first (500 lines)
                                try {{
                                    const resp = await fetch("{logs_url}?n=500{svc_param}", {{
                                        headers: {{ "X-Proxy-Token": "{token}" }},
                                    }});
                                    const data = await resp.json();
                                    if (data.lines) {{
                                        result = data.lines.join("\n");
                                        after = data.index || 0;
                                    }}
                                }} catch(e) {{
                                    return "fetch error: " + e.message;
                                }}
                                // Auto-scroll helper
                                const scrollToBottom = () => {{
                                    const el = document.querySelector('pre.log-output');
                                    if (el) el.scrollTop = el.scrollHeight;
                                }};
                                scrollToBottom();
                                for (let i = 0; i < 150; i++) {{
                                    await new Promise(r => setTimeout(r, 2000));
                                    try {{
                                        const resp = await fetch("{logs_url}?after=" + after + "{svc_param}", {{
                                            headers: {{ "X-Proxy-Token": "{token}" }},
                                        }});
                                        const data = await resp.json();
                                        if (data.lines && data.lines.length > 0) {{
                                            result += "\n" + data.lines.join("\n");
                                            scrollToBottom();
                                        }}
                                        if (data.index) after = data.index;
                                    }} catch(e) {{ break; }}
                                }}
                                return result;
                                "#,
                            );
                            match document::eval(&js).await {
                                Ok(result) => {
                                    let text = result.as_str().unwrap_or("").to_string();
                                    log_output.set(text);
                                }
                                Err(e) => {
                                    log_output.set(format!("Error: {e}"));
                                }
                            }
                            polling.set(false);
                        }
                    }
                },
                if *polling.read() { {t!("log-stop")} } else { {t!("log-start-tailing")} }
            }

            // Clear
            Button { size: ButtonSize::Sm, variant: ButtonVariant::Secondary,
                onclick: move |_| {
                    log_output.set(String::new());
                },
                {t!("clear")}
            }

            // One-shot fetch
            button { class: "btn btn-sm btn-success-soft",
                onclick: {
                    let logs_url = logs_url.clone();
                    let token = token.clone();
                    move |_| {
                        let logs_url = logs_url.clone();
                        let token = token.clone();
                        let svc = selected_service.read().clone();
                        async move {
                            let svc_param = if svc.is_empty() { String::new() } else { format!("&service={svc}") };
                            let js = format!(
                                r#"
                                try {{
                                    const resp = await fetch("{logs_url}?n=500{svc_param}", {{
                                        headers: {{ "X-Proxy-Token": "{token}" }},
                                    }});
                                    const data = await resp.json();
                                    return (data.lines || []).join("\n");
                                }} catch(e) {{
                                    return "fetch error: " + e.message;
                                }}
                                "#,
                            );
                            match document::eval(&js).await {
                                Ok(result) => {
                                    let text = result.as_str().unwrap_or("").to_string();
                                    log_output.set(text);
                                }
                                Err(e) => {
                                    log_output.set(format!("Error: {e}"));
                                }
                            }
                        }
                    }
                },
                {t!("log-fetch-latest")}
            }
        }

        // Log output
        pre { class: "log-output",
            if log_output.read().is_empty() {
                span { class: "text-fg-faint", {t!("log-empty-hint")} }
            } else {
                "{log_output}"
            }
        }
    }
}
