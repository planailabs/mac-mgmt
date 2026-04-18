use dioxus::prelude::*;
use dioxus::fullstack::JsonStream;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use crate::web::user::current_user;

// ── Wire types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerContext {
    pub instance_id: String,
    pub hostname: String,
    pub services_extended: Vec<serde_json::Value>,
}

/// Event streamed from server to client during a healer session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerStreamEvent {
    /// "session_created", "message", "state", "done", "error"
    pub kind: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
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

    let services_extended: Vec<serde_json::Value> = hb
        .services_extended
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    Ok(HealerContext {
        instance_id,
        hostname: hb.hostname.unwrap_or_default(),
        services_extended,
    })
}

/// Start a healer session and stream events back as they arrive.
#[server(output = JsonStream<HealerStreamEvent>)]
pub async fn start_healer_stream(
    instance_id: String,
    user_message: Option<String>,
) -> Result<JsonStream<HealerStreamEvent>, ServerFnError> {
    use mac_mgmt_healer::{HealerEvent, SpawnRequest};
    use mac_mgmt_healer::agent::InstanceInfo;

    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let healer = crate::server_state::healer_state()
        .ok_or_else(|| ServerFnError::new("healer not initialized"))?;

    // Look up instance
    #[derive(sqlx::FromRow)]
    struct HbInfo {
        cluster_id: uuid::Uuid,
        relay_proxy_url: Option<String>,
        services_extended: Option<serde_json::Value>,
        file_tunnels: Option<serde_json::Value>,
        shell_tunnels: Option<serde_json::Value>,
        sample: Option<serde_json::Value>,
        hostname: Option<String>,
    }
    let hb: HbInfo = sqlx::query_as(
        "SELECT cluster_id, relay_proxy_url, services_extended, \
                file_tunnels, shell_tunnels, sample, hostname \
         FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
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

    let cluster_name: String =
        sqlx::query_scalar("SELECT name FROM clusters WHERE id = $1")
            .bind(hb.cluster_id)
            .fetch_optional(&pool)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| hb.cluster_id.to_string());

    // Other instances in the cluster
    #[derive(sqlx::FromRow)]
    struct InstanceRow {
        instance_id: String,
        hostname: Option<String>,
    }
    let cluster_instances: Vec<InstanceInfo> = sqlx::query_as::<_, InstanceRow>(
        "SELECT instance_id, hostname FROM daemon_heartbeats \
         WHERE cluster_id = $1 AND instance_id != $2 \
         AND reported_at > now() - interval '5 minutes'",
    )
    .bind(hb.cluster_id)
    .bind(&instance_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| InstanceInfo {
        instance_prefix: r.instance_id.chars().take(12).collect(),
        hostname: r.hostname.unwrap_or_default(),
        healthy: true,
    })
    .collect();

    let services_extended: Vec<mac_mgmt_common::ServiceExtState> = hb
        .services_extended
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let req = SpawnRequest {
        cluster_id: hb.cluster_id,
        instance_id: instance_id.clone(),
        created_by: format!("web:{}", user.email),
        user_message,
        relay_url,
        services_extended,
        sample: hb.sample,
        file_tunnels: hb.file_tunnels.unwrap_or_default(),
        shell_tunnels: hb.shell_tunnels.unwrap_or_default(),
        cluster_instances,
        cluster_name,
        hostname: hb.hostname.unwrap_or_default(),
    };

    let session_id = healer
        .spawn_session(req)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Stream events via JsonStream using Streaming::spawn
    Ok(JsonStream::spawn(move |tx| async move {
        // Send session_created event
        let _ = tx.unbounded_send(HealerStreamEvent {
            kind: "session_created".to_string(),
            session_id: Some(session_id.to_string()),
            role: None,
            content: None,
            state: None,
        });

        // Subscribe to live events
        let Some(mut rx) = healer.subscribe(session_id) else {
            let _ = tx.unbounded_send(HealerStreamEvent {
                kind: "error".to_string(),
                session_id: None,
                role: None,
                content: Some("failed to subscribe to session events".to_string()),
                state: None,
            });
            return;
        };

        loop {
            match rx.recv().await {
                Ok(event) => {
                    let stream_event = match &event {
                        HealerEvent::Message { role, content, .. } => HealerStreamEvent {
                            kind: "message".to_string(),
                            session_id: None,
                            role: Some(role.clone()),
                            content: Some(content.clone()),
                            state: None,
                        },
                        HealerEvent::State { state, .. } => HealerStreamEvent {
                            kind: "state".to_string(),
                            session_id: None,
                            role: None,
                            content: None,
                            state: Some(state.clone()),
                        },
                        HealerEvent::Done { state } => HealerStreamEvent {
                            kind: "done".to_string(),
                            session_id: None,
                            role: None,
                            content: None,
                            state: Some(state.clone()),
                        },
                    };
                    let is_done = matches!(&event, HealerEvent::Done { .. });
                    let _ = tx.unbounded_send(stream_event);
                    if is_done {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    }))
}

/// Cancel a running healer session.
#[server]
pub async fn cancel_healer_session(session_id: String) -> Result<(), ServerFnError> {
    let _user = current_user().await?;
    let healer = crate::server_state::healer_state()
        .ok_or_else(|| ServerFnError::new("healer not initialized"))?;
    let uuid: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid session id"))?;
    healer
        .cancel_session(uuid)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Resume a paused healer session.
#[server]
pub async fn resume_healer_session(session_id: String) -> Result<(), ServerFnError> {
    let _user = current_user().await?;
    let healer = crate::server_state::healer_state()
        .ok_or_else(|| ServerFnError::new("healer not initialized"))?;
    let uuid: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid session id"))?;
    healer
        .resume_session(uuid)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
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

    let unhealthy: Vec<String> = ctx
        .services_extended
        .iter()
        .filter(|s| s.get("healthy").and_then(|v| v.as_bool()) == Some(false))
        .filter_map(|s| s.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();

    let instance_id = ctx.instance_id.clone();

    rsx! {
        h2 { class: "text-2xl font-bold mb-4", "Healer Agent" }
        p { class: "text-sm text-gray-500 dark:text-gray-400 mb-4",
            "Instance: {ctx.instance_id} ({ctx.hostname})"
        }

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
                        let instance_id = instance_id.clone();
                        move |_| {
                            let instance_id = instance_id.clone();
                            let msg = user_input.read().clone();
                            let user_msg = if msg.is_empty() { None } else { Some(msg) };
                            running.set(true);
                            messages.set(Vec::new());
                            state.set("starting".to_string());
                            async move {
                                match start_healer_stream(instance_id, user_msg).await {
                                    Ok(mut stream) => {
                                        while let Some(Ok(evt)) = stream.next().await {
                                            match evt.kind.as_str() {
                                                "session_created" => {
                                                    session_id.set(evt.session_id);
                                                }
                                                "message" => {
                                                    if let (Some(role), Some(content)) = (evt.role, evt.content) {
                                                        if !content.is_empty() {
                                                            messages.push(ChatMsg { role, content });
                                                        }
                                                    }
                                                }
                                                "state" => {
                                                    if let Some(s) = evt.state {
                                                        state.set(s);
                                                    }
                                                }
                                                "done" => {
                                                    if let Some(s) = evt.state {
                                                        state.set(s);
                                                    }
                                                    break;
                                                }
                                                "error" => {
                                                    messages.push(ChatMsg {
                                                        role: "system".to_string(),
                                                        content: evt.content.unwrap_or_else(|| "unknown error".to_string()),
                                                    });
                                                    state.set("failed".to_string());
                                                    break;
                                                }
                                                _ => {}
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
                        }
                    },
                    "Start Healing"
                }
            }

            if *running.read() {
                button {
                    class: "px-4 py-2 text-sm font-medium bg-red-600 text-white rounded hover:bg-red-700",
                    onclick: {
                        move |_| {
                            let sid = session_id.read().clone();
                            async move {
                                if let Some(sid) = sid {
                                    let _ = cancel_healer_session(sid).await;
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
                    rsx! {
                        button {
                            class: "px-4 py-2 text-sm font-medium bg-yellow-600 text-white rounded hover:bg-yellow-700",
                            onclick: {
                                move |_| {
                                    let sid = session_id.read().clone();
                                    async move {
                                        if let Some(sid) = sid {
                                            let _ = resume_healer_session(sid).await;
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
                let (badge_class, label) = state_badge(&st);
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

// ── Helpers ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMsg {
    role: String,
    content: String,
}

fn state_badge(st: &str) -> (&'static str, &'static str) {
    match st {
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
    }
}

fn render_message(msg: &ChatMsg) -> Element {
    let (bg, label) = match msg.role.as_str() {
        "system" => ("bg-gray-50 dark:bg-gray-800 border-l-4 border-gray-400", "System"),
        "assistant" => ("bg-blue-50 dark:bg-blue-900/20 border-l-4 border-blue-400", "Agent"),
        "user" => ("bg-green-50 dark:bg-green-900/20 border-l-4 border-green-400", "User"),
        "tool_result" => ("bg-yellow-50 dark:bg-yellow-900/20 border-l-4 border-yellow-400", "Tool"),
        "summary" => ("bg-purple-50 dark:bg-purple-900/20 border-l-4 border-purple-400", "Summary"),
        _ => ("bg-gray-50 dark:bg-gray-800 border-l-4 border-gray-300", "Other"),
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
