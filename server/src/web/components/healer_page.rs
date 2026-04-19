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
    pub cluster_id: String,
    pub services_extended: Vec<serde_json::Value>,
    pub sessions: Vec<SessionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub state: String,
    pub created_by: String,
    pub created_at: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerStreamEvent {
    pub kind: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    /// Snapshot of currently executing tools (sent with "running_tools" kind)
    #[serde(default)]
    pub running_tools: Option<Vec<RunningToolInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunningToolInfo {
    pub name: String,
    #[serde(default)]
    pub args: Option<String>,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffPingSummary {
    pub id: String,
    pub category: String,
    pub message: String,
    pub resolved: bool,
    pub created_at: String,
}

#[cfg(feature = "server")]
fn healer_event_to_stream(event: &mac_mgmt_healer::HealerEvent) -> (HealerStreamEvent, bool) {
    use mac_mgmt_healer::HealerEvent;
    match event {
        HealerEvent::Message { role, content, metadata, .. } => (
            HealerStreamEvent {
                kind: "message".to_string(),
                session_id: None, role: Some(role.clone()),
                content: Some(content.clone()), state: None,
                metadata: metadata.clone(), running_tools: None,
            },
            false,
        ),
        HealerEvent::RunningTools { tools } => (
            HealerStreamEvent {
                kind: "running_tools".to_string(),
                session_id: None, role: None, content: None, state: None, metadata: None,
                running_tools: Some(tools.iter().map(|t| RunningToolInfo {
                    name: t.name.clone(),
                    args: t.args.clone(),
                    started_at: t.started_at.to_rfc3339(),
                }).collect()),
            },
            false,
        ),
        HealerEvent::State { state, .. } => (
            HealerStreamEvent {
                kind: "state".to_string(),
                session_id: None, role: None, content: None,
                state: Some(state.clone()), metadata: None, running_tools: None,
            },
            false,
        ),
        HealerEvent::Done { state } => (
            HealerStreamEvent {
                kind: "done".to_string(),
                session_id: None, role: None, content: None,
                state: Some(state.clone()), metadata: None, running_tools: None,
            },
            true,
        ),
    }
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

    #[derive(sqlx::FromRow)]
    struct SessRow {
        id: uuid::Uuid,
        state: String,
        created_by: String,
        created_at: chrono::DateTime<chrono::Utc>,
        error_message: Option<String>,
    }
    let sessions = sqlx::query_as::<_, SessRow>(
        "SELECT id, state, created_by, created_at, error_message \
         FROM healer_sessions \
         WHERE cluster_id = $1 AND instance_id = $2 \
         ORDER BY created_at DESC LIMIT 20",
    )
    .bind(hb.cluster_id)
    .bind(&instance_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| SessionSummary {
        id: r.id.to_string(),
        state: r.state,
        created_by: r.created_by,
        created_at: r.created_at.format("%Y-%m-%d %H:%M").to_string(),
        error_message: r.error_message,
    })
    .collect();

    Ok(HealerContext {
        instance_id,
        hostname: hb.hostname.unwrap_or_default(),
        cluster_id: hb.cluster_id.to_string(),
        services_extended,
        sessions,
    })
}

#[server(output = JsonStream<HealerStreamEvent>)]
pub async fn view_healer_session(
    session_id: String,
) -> Result<JsonStream<HealerStreamEvent>, ServerFnError> {
    let _user = current_user().await?;
    let healer = crate::server_state::healer_state()
        .ok_or_else(|| ServerFnError::new("healer not initialized"))?;

    let uuid: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid session id"))?;

    let (session, existing_messages) = healer
        .get_session(uuid)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("session not found"))?;

    let is_active = session.state.is_active();
    let current_state = session.state.as_str().to_string();

    Ok(JsonStream::spawn(move |tx| async move {
        // Replay persisted messages
        for msg in existing_messages {
            let _ = tx.unbounded_send(HealerStreamEvent {
                kind: "message".to_string(),
                session_id: None, role: Some(msg.role), content: Some(msg.content),
                state: None, metadata: msg.metadata, running_tools: None,
            });
        }

        // Current state
        let _ = tx.unbounded_send(HealerStreamEvent {
            kind: "state".to_string(),
            session_id: None, role: None, content: None,
            state: Some(current_state.clone()), metadata: None, running_tools: None,
        });

        // Send current running tools snapshot (in-memory)
        let tools = healer.running_tools(uuid);
        if !tools.is_empty() {
            let _ = tx.unbounded_send(HealerStreamEvent {
                kind: "running_tools".to_string(),
                session_id: None, role: None, content: None,
                state: None, metadata: None,
                running_tools: Some(tools.iter().map(|t| RunningToolInfo {
                    name: t.name.clone(), args: t.args.clone(),
                    started_at: t.started_at.to_rfc3339(),
                }).collect()),
            });
        }

        // Stream live events
        if is_active {
            if let Some(mut rx) = healer.subscribe(uuid) {
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            let (stream_event, is_done) = healer_event_to_stream(&event);
                            let _ = tx.unbounded_send(stream_event);
                            if is_done { break; }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }

        let _ = tx.unbounded_send(HealerStreamEvent {
            kind: "done".to_string(),
            session_id: None,
            role: None,
            content: None,
            state: Some(current_state),
            running_tools: None,
            metadata: None,
        });
    }))
}

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

    #[derive(sqlx::FromRow)]
    struct InstanceRow { instance_id: String, hostname: Option<String> }
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

    Ok(JsonStream::spawn(move |tx| async move {
        let _ = tx.unbounded_send(HealerStreamEvent {
            kind: "session_created".to_string(),
            session_id: Some(session_id.to_string()),
            role: None, content: None, state: None, metadata: None, running_tools: None,
        });

        let Some(mut rx) = healer.subscribe(session_id) else {
            let _ = tx.unbounded_send(HealerStreamEvent {
                kind: "error".to_string(), session_id: None, role: None,
                content: Some("failed to subscribe".to_string()),
                state: None, metadata: None, running_tools: None,
            });
            return;
        };

        loop {
            match rx.recv().await {
                Ok(event) => {
                    let (evt, is_done) = healer_event_to_stream(&event);
                    let _ = tx.unbounded_send(evt);
                    if is_done { break; }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    }))
}

#[server]
pub async fn cancel_healer_session(session_id: String) -> Result<(), ServerFnError> {
    let _user = current_user().await?;
    let healer = crate::server_state::healer_state()
        .ok_or_else(|| ServerFnError::new("healer not initialized"))?;
    let uuid: uuid::Uuid = session_id.parse().map_err(|_| ServerFnError::new("invalid id"))?;
    healer.cancel_session(uuid).await.map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn resume_healer_session(session_id: String) -> Result<(), ServerFnError> {
    let _user = current_user().await?;
    let healer = crate::server_state::healer_state()
        .ok_or_else(|| ServerFnError::new("healer not initialized"))?;
    let uuid: uuid::Uuid = session_id.parse().map_err(|_| ServerFnError::new("invalid id"))?;
    healer.resume_session(uuid).await.map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn resolve_staff_ping(ping_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = ping_id.parse().map_err(|_| ServerFnError::new("invalid id"))?;
    mac_mgmt_healer::session::store::resolve_staff_ping(&pool, uuid, &user.email)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn get_staff_pings(session_id: String) -> Result<Vec<StaffPingSummary>, ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = session_id.parse().map_err(|_| ServerFnError::new("invalid id"))?;
    let pings = mac_mgmt_healer::session::store::list_session_pings(&pool, uuid)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(pings
        .into_iter()
        .map(|p| StaffPingSummary {
            id: p.id.to_string(),
            category: p.category,
            message: p.message,
            resolved: p.resolved,
            created_at: p.created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect())
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
        Some(Err(e)) => rsx! { p { class: "text-red-600 text-sm", "Error: {e}" } },
        None => rsx! { p { class: "text-gray-500 text-sm", "Loading..." } },
    }
}

fn render_healer(ctx: &HealerContext) -> Element {
    let mut session_id = use_signal::<Option<String>>(|| None);
    let mut messages = use_signal::<Vec<ChatMsg>>(Vec::new);
    let mut active_tools = use_signal::<Vec<RunningToolInfo>>(Vec::new);
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
    let sessions = ctx.sessions.clone();

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

        // New session controls
        if !*running.read() && session_id.read().is_none() {
            div { class: "mb-6 p-4 bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30",
                h3 { class: "text-lg font-semibold mb-3", "New Session" }
                div { class: "mb-3",
                    textarea {
                        class: "w-full px-3 py-2 text-sm border rounded dark:bg-gray-700 dark:border-gray-600 dark:text-gray-200",
                        rows: "2",
                        placeholder: "Optional instructions (leave empty for auto-diagnosis)...",
                        value: "{user_input}",
                        oninput: move |e| user_input.set(e.value()),
                    }
                }
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
                                consume_stream(
                                    start_healer_stream(instance_id, user_msg).await,
                                    &mut session_id, &mut messages, &mut active_tools, &mut state,
                                ).await;
                                running.set(false);
                            }
                        }
                    },
                    "Start Healing"
                }
            }
        }

        // Active session view
        if session_id.read().is_some() || *running.read() {
            div { class: "mb-6",
                div { class: "mb-3 flex items-center gap-3 flex-wrap",
                    {
                        let st = state.read().clone();
                        let (badge_class, label) = state_badge(&st);
                        rsx! {
                            span { class: "inline-block px-2 py-1 text-xs font-medium rounded {badge_class}", "{label}" }
                        }
                    }

                    if *running.read() {
                        button {
                            class: "px-3 py-1 text-xs font-medium bg-red-600 text-white rounded hover:bg-red-700",
                            onclick: move |_| {
                                let sid = session_id.read().clone();
                                async move {
                                    if let Some(sid) = sid { let _ = cancel_healer_session(sid).await; }
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
                                    class: "px-3 py-1 text-xs font-medium bg-yellow-600 text-white rounded hover:bg-yellow-700",
                                    onclick: move |_| {
                                        let sid = session_id.read().clone();
                                        async move {
                                            if let Some(sid) = sid { let _ = resume_healer_session(sid).await; }
                                        }
                                    },
                                    "Resume"
                                }
                            }
                        } else {
                            rsx! {}
                        }
                    }

                    if !*running.read() {
                        button {
                            class: "px-3 py-1 text-xs font-medium bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded hover:bg-gray-300",
                            onclick: move |_| {
                                session_id.set(None);
                                messages.set(Vec::new());
                                state.set("idle".to_string());
                            },
                            "Back to sessions"
                        }
                    }
                }

                // Pinned slots
                { render_pinned_slots(&messages.read()) }

                // Staff pings for this session
                if let Some(sid) = &*session_id.read() {
                    {
                        let sid = sid.clone();
                        rsx! { StaffPingsPanel { session_id: sid } }
                    }
                }

                // Chat messages (filter out pin messages — shown above)
                div { class: "space-y-2 max-h-[70vh] overflow-y-auto",
                    for msg in messages.read().iter().filter(|m| m.role != "pin") {
                        {render_message(msg)}
                    }

                    // Running tools (server-managed, in-memory only)
                    for tool in active_tools.read().iter() {
                        {
                            let name = tool.name.clone();
                            let args_short = tool.args.as_deref()
                                .map(|a| if a.len() > 120 { format!("{}...", &a[..120]) } else { a.to_string() })
                                .unwrap_or_default();
                            rsx! {
                                div { class: "px-3 py-2 rounded bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-200 dark:border-indigo-800 flex items-center gap-2",
                                    span { class: "inline-block w-2 h-2 rounded-full bg-indigo-400 animate-pulse" }
                                    span { class: "text-xs font-mono font-semibold text-indigo-700 dark:text-indigo-300", "{name}" }
                                    span { class: "text-xs text-gray-500 dark:text-gray-400 truncate", "{args_short}" }
                                }
                            }
                        }
                    }

                    if *running.read() && active_tools.read().is_empty() {
                        div { class: "p-3 text-sm text-gray-400 animate-pulse", "Agent is thinking..." }
                    }
                }
            }
        }

        // Previous sessions list
        if session_id.read().is_none() && !*running.read() && !sessions.is_empty() {
            div { class: "mt-6",
                h3 { class: "text-lg font-semibold mb-3", "Previous Sessions" }
                div { class: "space-y-2",
                    for sess in sessions.iter() {
                        {
                            let sid = sess.id.clone();
                            let sess_state = sess.state.clone();
                            let created_at = sess.created_at.clone();
                            let created_by = sess.created_by.clone();
                            let error_msg = sess.error_message.clone();
                            let (badge_class, badge_label) = state_badge(&sess_state);

                            rsx! {
                                div {
                                    class: "flex items-center justify-between p-3 bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 hover:bg-gray-50 dark:hover:bg-gray-750 cursor-pointer",
                                    onclick: {
                                        let sid = sid.clone();
                                        move |_| {
                                            let sid = sid.clone();
                                            session_id.set(Some(sid.clone()));
                                            messages.set(Vec::new());
                                            state.set("loading".to_string());
                                            running.set(true);
                                            async move {
                                                consume_stream(
                                                    view_healer_session(sid).await,
                                                    &mut session_id, &mut messages, &mut active_tools, &mut state,
                                                ).await;
                                                running.set(false);
                                            }
                                        }
                                    },
                                    div { class: "flex items-center gap-3",
                                        span { class: "inline-block px-2 py-0.5 text-xs font-medium rounded {badge_class}", "{badge_label}" }
                                        span { class: "text-sm text-gray-700 dark:text-gray-300", "{created_at}" }
                                        span { class: "text-xs text-gray-500 dark:text-gray-400", "{created_by}" }
                                    }
                                    div { class: "flex items-center gap-2",
                                        if let Some(err) = &error_msg {
                                            span { class: "text-xs text-red-500 max-w-xs truncate", "{err}" }
                                        }
                                        span { class: "text-xs text-gray-400", "View" }
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

// ── Staff pings panel ──────────────────────────────────────────────────

#[component]
fn StaffPingsPanel(session_id: String) -> Element {
    let sid = session_id.clone();
    let pings_res = use_server_future(move || {
        let sid = sid.clone();
        async move { get_staff_pings(sid).await }
    })?;

    let pings = match &*pings_res.read() {
        Some(Ok(p)) if !p.is_empty() => p.clone(),
        _ => return rsx! {},
    };

    rsx! {
        div { class: "mb-4 p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded",
            h4 { class: "text-sm font-semibold text-amber-800 dark:text-amber-300 mb-2", "Staff Pings" }
            div { class: "space-y-2",
                for ping in pings.iter() {
                    {
                        let ping_id = ping.id.clone();
                        let cat_badge = category_badge(&ping.category);
                        rsx! {
                            div { class: "flex items-start justify-between gap-2 text-sm",
                                div { class: "flex-1",
                                    span { class: "inline-block px-1.5 py-0.5 text-xs font-medium rounded mr-2 {cat_badge}", "{ping.category}" }
                                    if ping.resolved {
                                        span { class: "text-green-600 dark:text-green-400 line-through", "{ping.message}" }
                                    } else {
                                        span { class: "text-gray-800 dark:text-gray-200", "{ping.message}" }
                                    }
                                    span { class: "text-xs text-gray-400 ml-2", "{ping.created_at}" }
                                }
                                if !ping.resolved {
                                    button {
                                        class: "px-2 py-0.5 text-xs bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300 rounded hover:bg-green-200",
                                        onclick: move |_| {
                                            let ping_id = ping_id.clone();
                                            async move { let _ = resolve_staff_ping(ping_id).await; }
                                        },
                                        "Resolve"
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

// ── Stream consumer ────────────────────────────────────────────────────

async fn consume_stream(
    result: Result<JsonStream<HealerStreamEvent>, ServerFnError>,
    session_id: &mut Signal<Option<String>>,
    messages: &mut Signal<Vec<ChatMsg>>,
    active_tools: &mut Signal<Vec<RunningToolInfo>>,
    state: &mut Signal<String>,
) {
    match result {
        Ok(mut stream) => {
            while let Some(Ok(evt)) = stream.next().await {
                match evt.kind.as_str() {
                    "session_created" => { session_id.set(evt.session_id); }
                    "message" => {
                        if let (Some(role), Some(content)) = (evt.role, evt.content) {
                            if !content.is_empty() {
                                messages.push(ChatMsg { role, content, metadata: evt.metadata });
                            }
                        }
                    }
                    "running_tools" => {
                        active_tools.set(evt.running_tools.unwrap_or_default());
                    }
                    "state" => { if let Some(s) = evt.state { state.set(s); } }
                    "done" => {
                        active_tools.set(Vec::new());
                        if let Some(s) = evt.state { state.set(s); }
                        break;
                    }
                    "error" => {
                        active_tools.set(Vec::new());
                        messages.push(ChatMsg {
                            role: "system".to_string(),
                            content: evt.content.unwrap_or_else(|| "unknown error".to_string()),
                            metadata: None,
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
                metadata: None,
            });
            state.set("failed".to_string());
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMsg {
    role: String,
    content: String,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

fn state_badge(st: &str) -> (&'static str, &'static str) {
    match st {
        "starting" | "loading" | "created" | "initializing" =>
            ("bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300", "Initializing"),
        "diagnosing" =>
            ("bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300", "Diagnosing"),
        "remediating" =>
            ("bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-300", "Remediating"),
        "verifying" =>
            ("bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-300", "Verifying"),
        "completed" | "done" =>
            ("bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-300", "Done"),
        "failed" =>
            ("bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300", "Failed"),
        "cancelled" =>
            ("bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300", "Cancelled"),
        "paused" =>
            ("bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300", "Paused"),
        "awaiting_retry" =>
            ("bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300", "Awaiting retry"),
        "needs_human_attention" =>
            ("bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300", "Needs Human Attention"),
        _ =>
            ("bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300", "Unknown"),
    }
}

fn category_badge(cat: &str) -> &'static str {
    match cat {
        "hardware" => "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300",
        "network" => "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300",
        "disk_space" => "bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-300",
        "config_error" => "bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300",
        "service_crash" => "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300",
        "model_issue" => "bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-300",
        "permission" => "bg-pink-100 text-pink-800 dark:bg-pink-900 dark:text-pink-300",
        "dependency" => "bg-indigo-100 text-indigo-800 dark:bg-indigo-900 dark:text-indigo-300",
        "security" => "bg-red-200 text-red-900 dark:bg-red-800 dark:text-red-200",
        "performance" => "bg-cyan-100 text-cyan-800 dark:bg-cyan-900 dark:text-cyan-300",
        _ => "bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300",
    }
}

/// Render a chat message. Tool calls/results get special UI.
/// Assistant/system messages are rendered as markdown via dangerous_inner_html.
fn render_message(msg: &ChatMsg) -> Element {
    if msg.role == "tool_result" {
        return render_tool_result(msg);
    }

    let (bg, icon, label) = match msg.role.as_str() {
        "system" => ("bg-gray-50 dark:bg-gray-800 border-l-4 border-gray-400", "S", "System"),
        "assistant" => ("bg-blue-50 dark:bg-blue-900/20 border-l-4 border-blue-400", "A", "Agent"),
        "user" => ("bg-green-50 dark:bg-green-900/20 border-l-4 border-green-400", "U", "User"),
        "summary" => ("bg-purple-50 dark:bg-purple-900/20 border-l-4 border-purple-400", "S", "Summary"),
        _ => ("bg-gray-50 dark:bg-gray-800 border-l-4 border-gray-300", "-", "Other"),
    };

    let html = simple_md_to_html(&msg.content);

    rsx! {
        div { class: "p-3 rounded {bg}",
            div { class: "flex items-center gap-1.5 mb-1",
                span { class: "w-5 h-5 flex items-center justify-center rounded-full bg-gray-200 dark:bg-gray-700 text-xs font-bold text-gray-600 dark:text-gray-300", "{icon}" }
                span { class: "text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider", "{label}" }
            }
            div {
                class: "text-sm text-gray-800 dark:text-gray-200 prose prose-sm dark:prose-invert max-w-none",
                dangerous_inner_html: "{html}",
            }
        }
    }
}

fn render_tool_result(msg: &ChatMsg) -> Element {
    let (tool_name, result) = msg.content.split_once(": ").unwrap_or(("tool", &msg.content));
    let is_error = result.starts_with("Error:");
    let truncated = result.len() > 500;
    let preview = if truncated { &result[..500] } else { result };
    let tool_badge = if is_error {
        "text-xs px-1.5 py-0.5 rounded font-mono bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300"
    } else {
        "text-xs px-1.5 py-0.5 rounded font-mono bg-gray-200 text-gray-700 dark:bg-gray-700 dark:text-gray-300"
    };
    let display = if truncated {
        format!("{preview}\n... (output truncated)")
    } else {
        preview.to_string()
    };

    rsx! {
        div { class: "p-2 rounded bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700",
            details { class: "group",
                summary { class: "flex items-center gap-2 cursor-pointer select-none",
                    span { class: "{tool_badge}", "{tool_name}" }
                    if is_error {
                        span { class: "text-xs text-red-500", "error" }
                    }
                }
                pre { class: "mt-2 p-2 text-xs font-mono bg-gray-900 text-green-400 rounded overflow-x-auto max-h-64 overflow-y-auto whitespace-pre-wrap",
                    "{display}"
                }
            }
        }
    }
}

/// Render pinned slots (diagnosis, remediation, final_report) extracted from messages.
fn render_pinned_slots(messages: &[ChatMsg]) -> Element {
    // Collect the latest pin for each slot
    let mut pins: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();
    for msg in messages {
        if msg.role == "pin" {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                if let Some(slot) = data.get("slot").and_then(|v| v.as_str()) {
                    pins.insert(slot.to_string(), data);
                }
            }
        }
    }

    if pins.is_empty() {
        return rsx! {};
    }

    let slot_order = ["diagnosis", "remediation", "final_report"];

    rsx! {
        div { class: "mb-4 space-y-2",
            for slot_name in slot_order.iter() {
                if let Some(data) = pins.get(*slot_name) {
                    {
                        let summary = data.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let services: Vec<String> = data
                            .get("affected_services")
                            .and_then(|v| v.as_array())
                            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                            .unwrap_or_default();
                        let (icon, label, border) = match *slot_name {
                            "diagnosis" => ("🔍", "Diagnosis", "border-yellow-400 dark:border-yellow-600"),
                            "remediation" => ("🔧", "Remediation Plan", "border-orange-400 dark:border-orange-600"),
                            "final_report" => ("📋", "Final Report", "border-green-400 dark:border-green-600"),
                            _ => ("📌", *slot_name, "border-gray-400"),
                        };
                        let html = simple_md_to_html(&summary);
                        rsx! {
                            div { class: "p-3 bg-white dark:bg-gray-800 rounded border-l-4 {border} shadow-sm",
                                div { class: "flex items-center gap-1.5 mb-1",
                                    span { class: "text-sm", "{icon}" }
                                    span { class: "text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider", "{label}" }
                                }
                                div {
                                    class: "text-sm text-gray-800 dark:text-gray-200 prose prose-sm dark:prose-invert max-w-none",
                                    dangerous_inner_html: "{html}",
                                }
                                if !services.is_empty() {
                                    div { class: "mt-2 flex flex-wrap gap-1",
                                        for svc in services.iter() {
                                            span { class: "px-1.5 py-0.5 text-xs bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 rounded",
                                                "{svc}"
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

/// Render markdown to HTML using pulldown_cmark.
#[cfg(feature = "server")]
pub fn simple_md_to_html(md: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(md, options);
    let mut output = String::with_capacity(md.len() * 2);
    html::push_html(&mut output, parser);
    output
}

/// Client-side fallback: return content as-is (escaped).
#[cfg(not(feature = "server"))]
pub fn simple_md_to_html(md: &str) -> String {
    md.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\n', "<br>")
}
