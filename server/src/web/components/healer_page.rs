use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use crate::web::user::current_user;

// ── Wire types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerContext {
    pub api_base: String,
    pub api_token: String,
    pub cluster_id: String,
    pub instance_id: String,
    pub hostname: String,
    pub services_extended: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerSessionInfo {
    pub id: String,
    pub state: String,
    pub created_at: String,
    pub instance_id: String,
    pub error_message: Option<String>,
}

// ── Server functions ───────────────────────────────────────────────────

#[server]
pub async fn get_healer_context(instance_id: String) -> Result<HealerContext, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct HbInfo {
        cluster_id: uuid::Uuid,
        services_extended: Option<serde_json::Value>,
        hostname: Option<String>,
    }
    let hb: HbInfo = sqlx::query_as(
        "SELECT cluster_id, services_extended, hostname \
         FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
    )
    .bind(&instance_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("instance not found"))?;

    user.require_cluster_write(&pool, hb.cluster_id).await?;

    // Mint a setting-equivalent token for the healer API
    use rand::Rng;
    use sha2::{Digest, Sha256};
    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(6);
    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at) \
         VALUES ($1, $2, 'healer-web', 'setting', $3)",
    )
    .bind(hb.cluster_id)
    .bind(&hash)
    .bind(expires_at)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let cfg = crate::config::config();
    let api_base = cfg.api.external_url.clone();

    let services_extended: Vec<serde_json::Value> = hb
        .services_extended
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    Ok(HealerContext {
        api_base,
        api_token: raw_token,
        cluster_id: hb.cluster_id.to_string(),
        instance_id,
        hostname: hb.hostname.unwrap_or_default(),
        services_extended,
    })
}

// ── Component ──────────────────────────────────────────────────────────

#[component]
pub fn FleetHealer(instance_id: String) -> Element {
    let iid = instance_id.clone();
    let ctx = use_server_future(move || {
        let iid = iid.clone();
        async move { get_healer_context(iid).await }
    })?;

    match &*ctx.read() {
        Some(Ok(c)) => render_healer(c),
        Some(Err(e)) => rsx! {
            p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" }
        },
        None => rsx! {
            p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." }
        },
    }
}

fn render_healer(ctx: &HealerContext) -> Element {
    let mut session_id = use_signal::<Option<String>>(|| None);
    let mut messages = use_signal::<Vec<ChatMsg>>(Vec::new);
    let mut state = use_signal(|| "idle".to_string());
    let mut user_input = use_signal(String::new);
    let mut running = use_signal(|| false);

    // Unhealthy services
    let unhealthy: Vec<String> = ctx
        .services_extended
        .iter()
        .filter(|s| s.get("healthy").and_then(|v| v.as_bool()) == Some(false))
        .filter_map(|s| s.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();

    let api_base = ctx.api_base.clone();
    let api_token = ctx.api_token.clone();
    let instance_id = ctx.instance_id.clone();
    let cluster_id = ctx.cluster_id.clone();

    rsx! {
        h2 { class: "text-2xl font-bold mb-4", "Healer Agent" }
        p { class: "text-sm text-gray-500 dark:text-gray-400 mb-4",
            "Instance: {ctx.instance_id} ({ctx.hostname})"
        }

        // Unhealthy services banner
        if !unhealthy.is_empty() {
            div { class: "mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded",
                p { class: "text-sm font-medium text-red-800 dark:text-red-300",
                    "Unhealthy services: {unhealthy.join(\", \")}"
                }
            }
        }

        // Controls
        div { class: "mb-4 flex flex-wrap gap-2",
            if !*running.read() && session_id.read().is_none() {
                button {
                    class: "px-4 py-2 text-sm font-medium bg-green-600 text-white rounded hover:bg-green-700",
                    onclick: {
                        let api_base = api_base.clone();
                        let api_token = api_token.clone();
                        let instance_id = instance_id.clone();
                        let cluster_id = cluster_id.clone();
                        move |_| {
                            let api_base = api_base.clone();
                            let api_token = api_token.clone();
                            let instance_id = instance_id.clone();
                            let cluster_id = cluster_id.clone();
                            let msg = user_input.read().clone();
                            running.set(true);
                            messages.set(Vec::new());
                            state.set("starting".to_string());
                            async move {
                                start_and_stream(
                                    &api_base, &api_token, &cluster_id, &instance_id,
                                    if msg.is_empty() { None } else { Some(msg) },
                                    &mut session_id, &mut messages, &mut state, &mut running,
                                ).await;
                            }
                        }
                    },
                    "Start Healing"
                }
            }

            if *running.read() {
                button {
                    class: "px-4 py-2 text-sm font-medium bg-red-600 text-white rounded hover:bg-red-700",
                    onclick: {
                        let api_base = api_base.clone();
                        let api_token = api_token.clone();
                        let cluster_id = cluster_id.clone();
                        move |_| {
                            let api_base = api_base.clone();
                            let api_token = api_token.clone();
                            let cluster_id = cluster_id.clone();
                            let sid = session_id.read().clone();
                            async move {
                                if let Some(sid) = sid {
                                    cancel_session(&api_base, &api_token, &cluster_id, &sid).await;
                                }
                            }
                        }
                    },
                    "Cancel"
                }
            }

            {
                let st = state.read().clone();
                if st == "paused" {
                    let api_base = api_base.clone();
                    let api_token = api_token.clone();
                    let cluster_id = cluster_id.clone();
                    rsx! {
                        button {
                            class: "px-4 py-2 text-sm font-medium bg-yellow-600 text-white rounded hover:bg-yellow-700",
                            onclick: {
                                move |_| {
                                    let api_base = api_base.clone();
                                    let api_token = api_token.clone();
                                    let cluster_id = cluster_id.clone();
                                    let sid = session_id.read().clone();
                                    async move {
                                        if let Some(sid) = sid {
                                            resume_session(&api_base, &api_token, &cluster_id, &sid).await;
                                        }
                                    }
                                }
                            },
                            "Resume (budget exceeded)"
                        }
                    }
                } else {
                    rsx! {}
                }
            }
        }

        // Optional initial message input
        if session_id.read().is_none() && !*running.read() {
            div { class: "mb-4",
                label { class: "block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1",
                    "Optional instructions (leave empty for auto-diagnosis)"
                }
                textarea {
                    class: "w-full px-3 py-2 text-sm border rounded dark:bg-gray-700 dark:border-gray-600 dark:text-gray-200",
                    rows: "2",
                    placeholder: "e.g. Focus on the ollama service timeout...",
                    value: "{user_input}",
                    oninput: move |e| user_input.set(e.value()),
                }
            }
        }

        // State badge
        {
            let st = state.read().clone();
            if st != "idle" {
                let (badge_class, label) = match st.as_str() {
                    "starting" => ("bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300", "Starting..."),
                    "diagnosing" => ("bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300", "Diagnosing"),
                    "remediating" => ("bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-300", "Remediating"),
                    "verifying" => ("bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-300", "Verifying"),
                    "completed" => ("bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-300", "Completed"),
                    "failed" => ("bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300", "Failed"),
                    "cancelled" => ("bg-gray-100 text-gray-800 dark:bg-gray-900 dark:text-gray-300", "Cancelled"),
                    "paused" => ("bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300", "Paused (token budget)"),
                    "awaiting_retry" => ("bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300", "Awaiting retry"),
                    _ => ("bg-gray-100 text-gray-800 dark:bg-gray-900 dark:text-gray-300", "Unknown"),
                };
                rsx! {
                    span { class: "inline-block px-2 py-1 text-xs font-medium rounded mb-4 {badge_class}",
                        "{label}"
                    }
                }
            } else {
                rsx! {}
            }
        }

        // Chat messages
        div { class: "space-y-2 max-h-[70vh] overflow-y-auto",
            for msg in messages.read().iter() {
                {render_message(msg)}
            }
        }
    }
}

// ── Chat message rendering ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMsg {
    role: String,
    content: String,
}

fn render_message(msg: &ChatMsg) -> Element {
    let (bg, label) = match msg.role.as_str() {
        "system" => (
            "bg-gray-50 dark:bg-gray-800 border-l-4 border-gray-400",
            "System",
        ),
        "assistant" => (
            "bg-blue-50 dark:bg-blue-900/20 border-l-4 border-blue-400",
            "Agent",
        ),
        "user" => (
            "bg-green-50 dark:bg-green-900/20 border-l-4 border-green-400",
            "User",
        ),
        "tool_result" => (
            "bg-yellow-50 dark:bg-yellow-900/20 border-l-4 border-yellow-400",
            "Tool",
        ),
        "summary" => (
            "bg-purple-50 dark:bg-purple-900/20 border-l-4 border-purple-400",
            "Summary",
        ),
        _ => (
            "bg-gray-50 dark:bg-gray-800 border-l-4 border-gray-300",
            msg.role.as_str(),
        ),
    };

    rsx! {
        div { class: "p-3 rounded {bg}",
            span { class: "text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider",
                "{label}"
            }
            pre { class: "mt-1 text-sm text-gray-800 dark:text-gray-200 whitespace-pre-wrap font-mono overflow-x-auto",
                "{msg.content}"
            }
        }
    }
}

// ── Client-side JS helpers ─────────────────────────────────────────────

async fn start_and_stream(
    api_base: &str,
    api_token: &str,
    cluster_id: &str,
    instance_id: &str,
    user_message: Option<String>,
    session_id: &mut Signal<Option<String>>,
    messages: &mut Signal<Vec<ChatMsg>>,
    state: &mut Signal<String>,
    running: &mut Signal<bool>,
) {
    let msg_json = match &user_message {
        Some(m) => format!(r#","user_message":"{}""#, m.replace('\\', "\\\\").replace('"', "\\\"")),
        None => String::new(),
    };

    let js = format!(
        r#"
        try {{
            // 1. Create session
            const createResp = await fetch("{api_base}/api/healer/sessions", {{
                method: "POST",
                headers: {{
                    "Authorization": "Bearer {api_token}",
                    "Content-Type": "application/json",
                    "X-Cluster-Id": "{cluster_id}",
                }},
                body: JSON.stringify({{ instance_id: "{instance_id}"{msg_json} }}),
            }});
            if (!createResp.ok) {{
                return JSON.stringify({{ error: "Failed to create session: " + createResp.status }});
            }}
            const created = await createResp.json();
            const sessionId = created.session_id;

            // 2. Stream SSE events
            const streamResp = await fetch("{api_base}/api/healer/sessions/" + sessionId + "/stream", {{
                headers: {{
                    "Authorization": "Bearer {api_token}",
                    "X-Cluster-Id": "{cluster_id}",
                }},
            }});
            const reader = streamResp.body.getReader();
            const decoder = new TextDecoder();
            let events = [];
            let finalState = "running";

            while (true) {{
                const {{done, value}} = await reader.read();
                if (done) break;
                const text = decoder.decode(value, {{stream: true}});
                for (const line of text.split("\\n")) {{
                    if (line.startsWith("data: ")) {{
                        try {{
                            const evt = JSON.parse(line.slice(6));
                            events.push(evt);
                            if (evt.type === "done") {{
                                finalState = evt.state || "completed";
                            }} else if (evt.type === "state") {{
                                finalState = evt.state || finalState;
                            }}
                        }} catch(e) {{}}
                    }}
                }}
            }}
            return JSON.stringify({{ session_id: sessionId, events: events, state: finalState }});
        }} catch(e) {{
            return JSON.stringify({{ error: e.message }});
        }}
        "#,
    );

    match document::eval(&js).await {
        Ok(result) => {
            let text = result.as_str().unwrap_or("{}");
            if let Ok(resp) = serde_json::from_str::<serde_json::Value>(text) {
                if let Some(err) = resp.get("error").and_then(|v| v.as_str()) {
                    messages.push(ChatMsg {
                        role: "system".to_string(),
                        content: format!("Error: {err}"),
                    });
                    state.set("failed".to_string());
                } else {
                    if let Some(sid) = resp.get("session_id").and_then(|v| v.as_str()) {
                        session_id.set(Some(sid.to_string()));
                    }
                    if let Some(events) = resp.get("events").and_then(|v| v.as_array()) {
                        let mut msgs: Vec<ChatMsg> = Vec::new();
                        for evt in events {
                            let evt_type = evt.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            match evt_type {
                                "message" => {
                                    let role = evt.get("role").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                                    let content = evt.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    if !content.is_empty() {
                                        msgs.push(ChatMsg { role, content });
                                    }
                                }
                                "state" => {
                                    if let Some(s) = evt.get("state").and_then(|v| v.as_str()) {
                                        state.set(s.to_string());
                                    }
                                }
                                "done" => {
                                    if let Some(s) = evt.get("state").and_then(|v| v.as_str()) {
                                        state.set(s.to_string());
                                    }
                                }
                                _ => {}
                            }
                        }
                        messages.set(msgs);
                    }
                    if let Some(s) = resp.get("state").and_then(|v| v.as_str()) {
                        state.set(s.to_string());
                    }
                }
            }
        }
        Err(e) => {
            messages.push(ChatMsg {
                role: "system".to_string(),
                content: format!("Error: {e}"),
            });
            state.set("failed".to_string());
        }
    }
    running.set(false);
}

async fn cancel_session(api_base: &str, api_token: &str, cluster_id: &str, session_id: &str) {
    let js = format!(
        r#"
        await fetch("{api_base}/api/healer/sessions/{session_id}/cancel", {{
            method: "POST",
            headers: {{
                "Authorization": "Bearer {api_token}",
                "X-Cluster-Id": "{cluster_id}",
            }},
        }});
        return "ok";
        "#,
    );
    let _ = document::eval(&js).await;
}

async fn resume_session(api_base: &str, api_token: &str, cluster_id: &str, session_id: &str) {
    let js = format!(
        r#"
        await fetch("{api_base}/api/healer/sessions/{session_id}/resume", {{
            method: "POST",
            headers: {{
                "Authorization": "Bearer {api_token}",
                "X-Cluster-Id": "{cluster_id}",
            }},
        }});
        return "ok";
        "#,
    );
    let _ = document::eval(&js).await;
}
