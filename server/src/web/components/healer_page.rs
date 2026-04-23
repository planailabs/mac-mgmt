use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use crate::web::user::current_user;

// ── Wire types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub name: String,
    pub model: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerContext {
    pub instance_id: String,
    pub hostname: String,
    pub cluster_id: String,
    pub services_extended: Vec<serde_json::Value>,
    pub sessions: Vec<SessionSummary>,
    pub models: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub state: String,
    pub created_by: String,
    pub created_at: String,
    pub error_message: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub label: Option<String>,
}

// Re-export wire types from common — single source of truth.
pub use mac_mgmt_common::{
    HealerPin as PinInfo, HealerRunningTool as RunningToolInfo,
    HealerStaffPing as StaffPingSummary, HealerStreamEvent,
};

#[cfg(feature = "server")]
pub fn staff_pings_to_wire(pings: &[mac_mgmt_healer::session::StaffPing]) -> Vec<StaffPingSummary> {
    pings
        .iter()
        .map(|p| StaffPingSummary {
            id: p.id.to_string(),
            category: p.category.clone(),
            message: p.message.clone(),
            resolved: p.resolved,
            created_at: p.created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect()
}

#[cfg(feature = "server")]
pub fn running_tools_to_wire(tools: &[mac_mgmt_healer::session::RunningTool]) -> Vec<RunningToolInfo> {
    tools
        .iter()
        .map(|t| RunningToolInfo {
            name: t.name.clone(),
            args: t.args.clone(),
            started_at: t.started_at.to_rfc3339(),
        })
        .collect()
}

#[cfg(feature = "server")]
pub fn healer_event_to_stream(event: &mac_mgmt_healer::HealerEvent) -> (HealerStreamEvent, bool) {
    use mac_mgmt_healer::HealerEvent;
    let empty = HealerStreamEvent::default();
    match event {
        HealerEvent::Message {
            role,
            content,
            metadata,
            ..
        } => (
            HealerStreamEvent {
                kind: "message".to_string(),
                role: Some(role.clone()),
                content: Some(content.clone()),
                metadata: metadata.clone(),
                ..empty
            },
            false,
        ),
        HealerEvent::RunningTools { tools } => (
            HealerStreamEvent {
                kind: "running_tools".to_string(),
                running_tools: Some(running_tools_to_wire(tools)),
                ..empty
            },
            false,
        ),
        HealerEvent::State { state, state_data } => {
            let reason = state_data
                .get("reason")
                .and_then(|v| v.as_str())
                .map(String::from);
            (
                HealerStreamEvent {
                    kind: "state".to_string(),
                    state: Some(state.clone()),
                    state_reason: reason,
                    ..empty
                },
                false,
            )
        }
        HealerEvent::Status { message } => (
            HealerStreamEvent {
                kind: "status".to_string(),
                status_message: Some(message.clone()),
                ..empty
            },
            false,
        ),
        HealerEvent::Done { state } => (
            HealerStreamEvent {
                kind: "done".to_string(),
                state: Some(state.clone()),
                ..empty
            },
            true,
        ),
    }
}

#[cfg(feature = "server")]
pub fn extract_pins_from_messages(messages: &[mac_mgmt_healer::HealerMessage]) -> Vec<PinInfo> {
    let mut pins = std::collections::HashMap::<String, PinInfo>::new();
    for msg in messages {
        if msg.role == "pin" {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                if let Some(slot) = data.get("slot").and_then(|v| v.as_str()) {
                    pins.insert(
                        slot.to_string(),
                        PinInfo {
                            slot: slot.to_string(),
                            summary: data
                                .get("summary")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            affected_services: data
                                .get("affected_services")
                                .and_then(|v| v.as_array())
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                        },
                    );
                }
            }
        }
    }
    let order = ["diagnosis", "remediation", "final_report"];
    let mut result = Vec::new();
    for slot in &order {
        if let Some(pin) = pins.remove(*slot) {
            result.push(pin);
        }
    }
    result
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
        provider: Option<String>,
        model: Option<String>,
        label: Option<String>,
    }
    let sessions = sqlx::query_as::<_, SessRow>(
        "SELECT id, state, created_by, created_at, error_message, provider, model, label \
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
        provider: r.provider,
        model: r.model,
        label: r.label,
    })
    .collect();

    let healer_cfg = &crate::config::config().healer;
    let model_entries = if healer_cfg.models.is_empty() {
        crate::config::default_healer_models()
    } else {
        healer_cfg.models.clone()
    };
    let models = model_entries
        .into_iter()
        .map(|e| ModelEntry {
            name: e.display_name(),
            model: e.model,
            provider: e.provider,
        })
        .collect();

    Ok(HealerContext {
        instance_id,
        hostname: hb.hostname.unwrap_or_default(),
        cluster_id: hb.cluster_id.to_string(),
        services_extended,
        sessions,
        models,
    })
}

// view_healer_session streaming is handled by the axum SSE endpoint
// at /web/healer/stream/{session_id} — see web/healer_sse.rs

/// Spawn a new healer session. Returns the session ID (no streaming — the
/// client connects to the axum SSE endpoint at `/web/healer/stream/{id}`).
#[server]
pub async fn start_healer_session(
    instance_id: String,
    user_message: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    fix_provider: Option<String>,
    fix_model: Option<String>,
    auto_approve: bool,
) -> Result<String, ServerFnError> {
    use mac_mgmt_healer::agent::InstanceInfo;
    use mac_mgmt_healer::SpawnRequest;

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

    let pg_store = crate::server_state::pg_healer_store()?;
    let (proxy_token, proxy_expires) = pg_store
        .mint_proxy_token(hb.cluster_id, None)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let relay_client = std::sync::Arc::new(
        mac_mgmt_healer::relay_client::RelayClient::new(relay_url.clone(), proxy_token),
    );
    let instance_prefix: String = instance_id.chars().take(12).collect();
    let instance_access: mac_mgmt_healer::DynInstanceAccess = std::sync::Arc::new(
        mac_mgmt_healer::relay_client::RelayInstanceAccess::new(
            relay_client.clone(),
            instance_prefix,
        ),
    );
    let cluster_access: Option<mac_mgmt_healer::DynClusterAccess> = Some(std::sync::Arc::new(
        mac_mgmt_healer::relay_client::RelayClusterAccess::new(relay_client),
    ));
    let metrics_url = Some(format!("{}/metrics", relay_url));

    let cluster_name: String = sqlx::query_scalar("SELECT name FROM clusters WHERE id = $1")
        .bind(hb.cluster_id)
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| hb.cluster_id.to_string());

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

    // Per-cluster healer settings from dedicated table.
    let cluster_healer = {
        #[derive(sqlx::FromRow)]
        struct Row {
            auto_trigger: Option<bool>,
            auto_approve: Option<bool>,
            fix_provider: Option<String>,
            fix_model: Option<String>,
        }
        sqlx::query_as::<_, Row>(
            "SELECT auto_trigger, auto_approve, fix_provider, fix_model \
             FROM healer_cluster_settings WHERE cluster_id = $1",
        )
        .bind(hb.cluster_id)
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten()
        .map(|r| mac_mgmt_common::HealerClusterConfig {
            auto_trigger: r.auto_trigger,
            auto_approve: r.auto_approve,
            fix_provider: r.fix_provider,
            fix_model: r.fix_model,
        })
        .unwrap_or_default()
    };
    let server_cfg = crate::config::load();

    let req = SpawnRequest {
        cluster_id: hb.cluster_id,
        instance_id: instance_id.clone(),
        created_by: format!("web:{}", user.email),
        user_message,
        instance_access,
        cluster_access,
        metrics_url,
        services_extended,
        sample: hb.sample,
        file_tunnels: hb.file_tunnels.unwrap_or_default(),
        shell_tunnels: hb.shell_tunnels.unwrap_or_default(),
        cluster_instances,
        cluster_name,
        hostname: hb.hostname.unwrap_or_default(),
        skip_cooldown: user.is_admin,
        provider: provider.clone(),
        model: model.clone(),
        label: None,
        token_budget: {
            if let (Some(p), Some(m)) = (&provider, &model) {
                let models = if server_cfg.healer.models.is_empty() {
                    crate::config::default_healer_models()
                } else {
                    server_cfg.healer.models.clone()
                };
                models.iter()
                    .find(|entry| entry.provider == *p && entry.model == *m)
                    .and_then(|entry| entry.token_budget)
            } else {
                None
            }
        },
        proxy_expires: Some(proxy_expires),
        // Priority: request param > cluster config > server global
        auto_approve: auto_approve || cluster_healer.auto_approve.unwrap_or(false),
        fix_provider: fix_provider
            .or(cluster_healer.fix_provider)
            .or_else(|| server_cfg.healer.fix_provider.clone()),
        fix_model: fix_model
            .or(cluster_healer.fix_model)
            .or_else(|| server_cfg.healer.fix_model.clone()),
    };

    let session_id = healer
        .spawn_session(req)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(session_id.to_string())
}

#[server]
pub async fn cancel_healer_session(session_id: String) -> Result<(), ServerFnError> {
    let _user = current_user().await?;
    let healer = crate::server_state::healer_state()
        .ok_or_else(|| ServerFnError::new("healer not initialized"))?;
    let uuid: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    healer
        .cancel_session(uuid)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn pause_healer_session(session_id: String) -> Result<(), ServerFnError> {
    let _user = current_user().await?;
    let healer = crate::server_state::healer_state()
        .ok_or_else(|| ServerFnError::new("healer not initialized"))?;
    let uuid: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    healer
        .pause_session(uuid)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn extend_healer_budget(session_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("admin access required"));
    }
    let healer = crate::server_state::healer_state()
        .ok_or_else(|| ServerFnError::new("healer not initialized"))?;
    let uuid: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    healer
        .extend_budget(uuid)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn resume_healer_session(session_id: String) -> Result<(), ServerFnError> {
    let _user = current_user().await?;
    let healer = crate::server_state::healer_state()
        .ok_or_else(|| ServerFnError::new("healer not initialized"))?;
    let uuid: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    healer
        .resume_session(uuid)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn resolve_staff_ping(ping_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let healer = crate::server_state::healer_state()
        .ok_or_else(|| ServerFnError::new("healer not initialized"))?;
    let uuid: uuid::Uuid = ping_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    healer.store().resolve_staff_ping(uuid, &user.email)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Session metadata returned by get_session_meta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub label: Option<String>,
    pub created_by: String,
}

/// Get metadata for a session (provider, model, label, created_by).
#[server]
pub async fn get_session_meta(session_id: String) -> Result<SessionMeta, ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    #[derive(sqlx::FromRow)]
    struct Row {
        provider: Option<String>,
        model: Option<String>,
        label: Option<String>,
        created_by: String,
    }
    let row = sqlx::query_as::<_, Row>(
        "SELECT provider, model, label, created_by FROM healer_sessions WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("session not found"))?;
    Ok(SessionMeta {
        provider: row.provider,
        model: row.model,
        label: row.label,
        created_by: row.created_by,
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
        Some(Err(e)) => rsx! { p { class: "text-red-600 text-sm", "Error: {e}" } },
        None => rsx! { p { class: "text-gray-500 text-sm", "Loading..." } },
    }
}

fn render_healer(ctx: &HealerContext) -> Element {
    let mut session_id = use_signal::<Option<String>>(|| None);
    let mut messages = use_signal::<Vec<ChatMsg>>(Vec::new);
    let mut active_tools = use_signal::<Vec<RunningToolInfo>>(Vec::new);
    let mut pins = use_signal::<Vec<PinInfo>>(Vec::new);
    let mut staff_pings = use_signal::<Vec<StaffPingSummary>>(Vec::new);
    let mut status_msg = use_signal::<Option<String>>(|| None);
    let mut state = use_signal(|| "idle".to_string());
    let mut state_reason = use_signal::<Option<String>>(|| None);
    let mut user_input = use_signal(String::new);
    let mut running = use_signal(|| false);
    // Encodes "provider:model" or empty for first entry
    let mut selected_model_key = use_signal(String::new);
    let mut selected_fix_model_key = use_signal(|| "none".to_string());
    let mut auto_approve = use_signal(|| false);
    let models = ctx.models.clone();

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
            {
                let ollama_models: Vec<ModelEntry> = models.iter().filter(|m| m.provider == "ollama").cloned().collect();
                let anthropic_models: Vec<ModelEntry> = models.iter().filter(|m| m.provider == "anthropic").cloned().collect();
                let openrouter_models: Vec<ModelEntry> = models.iter().filter(|m| m.provider == "openrouter").cloned().collect();
                // Build option values as "provider:model"
                let first_key = models.first().map(|m| format!("{}:{}", m.provider, m.model)).unwrap_or_default();
                rsx! {
                    div { class: "mb-6 p-4 bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30",
                        h3 { class: "text-lg font-semibold mb-3", "New Session" }

                        // Model selector
                        div { class: "mb-3",
                            label { class: "block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1", "Model" }
                            select {
                                class: "w-full px-3 py-2 text-sm border rounded dark:bg-gray-700 dark:border-gray-600 dark:text-gray-200",
                                value: "{selected_model_key}",
                                onchange: move |e| selected_model_key.set(e.value()),
                                if !ollama_models.is_empty() {
                                    optgroup { label: "Ollama (local, free)",
                                        for m in ollama_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !anthropic_models.is_empty() {
                                    optgroup { label: "Anthropic (cloud)",
                                        for m in anthropic_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !openrouter_models.is_empty() {
                                    optgroup { label: "OpenRouter (cloud)",
                                        for m in openrouter_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                            }
                            {
                                let key = selected_model_key.read().clone();
                                let key = if key.is_empty() { first_key.clone() } else { key };
                                let is_ollama = key.starts_with("ollama:");
                                if is_ollama {
                                    rsx! {
                                        p { class: "mt-1 text-xs text-gray-500 dark:text-gray-400",
                                            "Free to run, but local models are less capable than cloud models."
                                        }
                                    }
                                } else if key.starts_with("openrouter:") {
                                    rsx! {
                                        p { class: "mt-1 text-xs text-gray-500 dark:text-gray-400",
                                            "Uses OpenRouter API credits. Subject to token budget."
                                        }
                                    }
                                } else {
                                    rsx! {
                                        p { class: "mt-1 text-xs text-gray-500 dark:text-gray-400",
                                            "Uses Anthropic API credits. More capable, subject to token budget."
                                        }
                                    }
                                }
                            }
                        }

                        // Fix-model selector (optional, for remediation phase)
                        div { class: "mb-3",
                            label { class: "block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1", "Fix Model (remediation)" }
                            select {
                                class: "w-full px-3 py-2 text-sm border rounded dark:bg-gray-700 dark:border-gray-600 dark:text-gray-200",
                                value: "{selected_fix_model_key}",
                                onchange: move |e| selected_fix_model_key.set(e.value()),
                                option { value: "none", "Same as diagnosis model" }
                                if !ollama_models.is_empty() {
                                    optgroup { label: "Ollama (local, free)",
                                        for m in ollama_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !anthropic_models.is_empty() {
                                    optgroup { label: "Anthropic (cloud)",
                                        for m in anthropic_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !openrouter_models.is_empty() {
                                    optgroup { label: "OpenRouter (cloud)",
                                        for m in openrouter_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                            }
                            p { class: "mt-1 text-xs text-gray-500 dark:text-gray-400",
                                "Optional: use a different model for the remediation phase after diagnosis."
                            }
                        }

                        div { class: "mb-3",
                            textarea {
                                class: "w-full px-3 py-2 text-sm border rounded dark:bg-gray-700 dark:border-gray-600 dark:text-gray-200",
                                rows: "2",
                                placeholder: "Optional instructions (leave empty for auto-diagnosis)...",
                                value: "{user_input}",
                                oninput: move |e| user_input.set(e.value()),
                            }
                        }
                        div { class: "mb-3 flex items-center gap-2",
                            input {
                                r#type: "checkbox",
                                id: "auto-approve",
                                class: "rounded border-gray-300 dark:border-gray-600 dark:bg-gray-700",
                                checked: *auto_approve.read(),
                                onchange: move |e| auto_approve.set(e.checked()),
                            }
                            label {
                                r#for: "auto-approve",
                                class: "text-sm text-gray-700 dark:text-gray-300",
                                "Auto-approve remediation"
                            }
                            p { class: "text-xs text-gray-500 dark:text-gray-400",
                                "(skip approval gate between diagnosis and fix)"
                            }
                        }
                        button {
                            class: "px-4 py-2 text-sm font-medium bg-green-600 text-white rounded hover:bg-green-700",
                            onclick: {
                                let instance_id = instance_id.clone();
                                let first_key = first_key.clone();
                                move |_| {
                                    let instance_id = instance_id.clone();
                                    let msg = user_input.read().clone();
                                    let user_msg = if msg.is_empty() { None } else { Some(msg) };
                                    let key = selected_model_key.read().clone();
                                    let key = if key.is_empty() { first_key.clone() } else { key };
                                    let (provider, model) = if let Some((p, m)) = key.split_once(':') {
                                        (Some(p.to_string()), Some(m.to_string()))
                                    } else {
                                        (None, None)
                                    };
                                    let fix_key = selected_fix_model_key.read().clone();
                                    let (fix_provider, fix_model) = if fix_key != "none" {
                                        if let Some((p, m)) = fix_key.split_once(':') {
                                            (Some(p.to_string()), Some(m.to_string()))
                                        } else {
                                            (None, None)
                                        }
                                    } else {
                                        (None, None)
                                    };
                                    let approve = *auto_approve.read();
                                    async move {
                                        match start_healer_session(instance_id.clone(), user_msg, provider, model, fix_provider, fix_model, approve).await {
                                            Ok(sid) => {
                                                navigator().push(format!("/fleet/{}/healer/{}", instance_id, sid));
                                            }
                                            Err(e) => {
                                                tracing::error!("failed to start healer session: {e}");
                                            }
                                        }
                                    }
                                }
                            },
                            "Start Healing"
                        }
                    }
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
                        let reason = state_reason.read().clone();
                        rsx! {
                            span { class: "inline-block px-2 py-1 text-xs font-medium rounded {badge_class}", "{label}" }
                            if let Some(reason) = reason {
                                span { class: "text-xs text-gray-500 dark:text-gray-400 italic",
                                    "({reason_display(&reason)})"
                                }
                            }
                        }
                    }

                    if *running.read() {
                        button {
                            class: "px-3 py-1 text-xs font-medium bg-yellow-600 text-white rounded hover:bg-yellow-700",
                            onclick: move |_| {
                                let sid = session_id.read().clone();
                                async move {
                                    if let Some(sid) = sid { let _ = pause_healer_session(sid).await; }
                                }
                            },
                            "Pause"
                        }
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
                                active_tools.set(Vec::new());
                                pins.set(Vec::new());
                                staff_pings.set(Vec::new());
                                status_msg.set(None);
                                state.set("idle".to_string());
                                state_reason.set(None);
                            },
                            "Back to sessions"
                        }
                    }
                }

                // Pinned slots (from stream)
                { render_pinned_slots_from_signal(&pins.read()) }

                // Staff pings (from stream)
                { render_staff_pings_inline(&staff_pings.read()) }

                // Chat messages (filter out pin messages — shown above)
                div { class: "space-y-2",
                    for msg in messages.read().iter().filter(|m| m.role != "pin") {
                        {render_message(msg)}
                    }

                    // Running tools (server-managed, in-memory only)
                    for tool in active_tools.read().iter() {
                        {
                            let name = tool.name.clone();
                            let args_short = tool.args.as_deref()
                                .filter(|a| *a != "{}")
                                .map(|a| if a.len() > 120 { format!("{}...", &a[..120]) } else { a.to_string() })
                                .unwrap_or_default();
                            let has_args = !args_short.is_empty();
                            rsx! {
                                div { class: "px-3 py-2 rounded bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-200 dark:border-indigo-800 flex items-center gap-2",
                                    span { class: "inline-block w-2 h-2 rounded-full bg-indigo-400 animate-pulse" }
                                    span { class: "text-xs font-mono font-semibold text-indigo-700 dark:text-indigo-300", "{name}" }
                                    if has_args {
                                        span { class: "text-xs text-gray-500 dark:text-gray-400 truncate", "{args_short}" }
                                    }
                                }
                            }
                        }
                    }

                    // Ephemeral status (e.g. "Waiting for daemon reconnect...")
                    if let Some(msg) = &*status_msg.read() {
                        div { class: "px-3 py-2 rounded bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 text-sm text-amber-800 dark:text-amber-300 animate-pulse",
                            "{msg}"
                        }
                    }

                    if *running.read() && active_tools.read().is_empty() && status_msg.read().is_none() {
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
                            let model_label = sess.model.clone().unwrap_or_default();
                            let session_label = sess.label.clone().unwrap_or_default();
                            let is_auto = created_by.starts_with("auto:");
                            let (badge_class, badge_label) = state_badge(&sess_state);
                            let url = format!("/fleet/{}/healer/{}", instance_id, sid);

                            rsx! {
                                Link {
                                    to: url,
                                    class: "flex items-center justify-between p-3 bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 hover:bg-gray-50 dark:hover:bg-gray-750 cursor-pointer",
                                    div { class: "flex items-center gap-3",
                                        span { class: "inline-block px-2 py-0.5 text-xs font-medium rounded {badge_class}", "{badge_label}" }
                                        if is_auto {
                                            span { class: "px-1.5 py-0.5 text-xs bg-amber-100 dark:bg-amber-900 text-amber-700 dark:text-amber-300 rounded", "auto" }
                                        }
                                        if !session_label.is_empty() {
                                            span { class: "text-sm font-medium text-gray-700 dark:text-gray-300 truncate max-w-xs", "{session_label}" }
                                        }
                                        span { class: "text-sm text-gray-700 dark:text-gray-300", "{created_at}" }
                                        if !model_label.is_empty() {
                                            span { class: "px-1.5 py-0.5 text-xs font-mono bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 rounded", "{model_label}" }
                                        }
                                        if !is_auto {
                                            span { class: "text-xs text-gray-500 dark:text-gray-400", "{created_by}" }
                                        }
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

// ── Session detail view (routed at /fleet/:instance_id/healer/:session_id) ──

#[component]
pub fn FleetHealerSession(instance_id: String, session_id: String) -> Element {
    let mut messages = use_signal::<Vec<ChatMsg>>(Vec::new);
    let mut active_tools = use_signal::<Vec<RunningToolInfo>>(Vec::new);
    let mut pins = use_signal::<Vec<PinInfo>>(Vec::new);
    let mut staff_pings_sig = use_signal::<Vec<StaffPingSummary>>(Vec::new);
    let mut status_msg = use_signal::<Option<String>>(|| None);
    let mut state = use_signal(|| "loading".to_string());
    let mut state_reason = use_signal::<Option<String>>(|| None);
    let mut running = use_signal(|| true);

    let iid = instance_id.clone();
    let sid_for_meta = session_id.clone();
    let session_meta = use_server_future(move || {
        let sid = sid_for_meta.clone();
        async move { get_session_meta(sid).await }
    })?;
    let sid_for_sse = session_id.clone();

    // Connect to SSE via JS eval — runs only in the browser, no SSR issues.
    // eval() returns a channel we can recv() events from.
    let _sse_task = use_future(move || {
        let sid = sid_for_sse.clone();
        async move {
            let mut ev = document::eval(&format!(
                r#"
                const es = new EventSource("/_sse/healer/{sid}");
                es.onmessage = (e) => dioxus.send(e.data);
                es.onerror = () => {{
                    if (es.readyState === EventSource.CLOSED) {{
                        dioxus.send('{{"kind":"_closed"}}');
                    }}
                }};
                // Keep alive until Dioxus drops the eval
                await new Promise(() => {{}});
                "#
            ));

            while let Ok(val) = ev.recv::<serde_json::Value>().await {
                let val_str = val.as_str().unwrap_or_default();
                let Ok(evt) = serde_json::from_str::<HealerStreamEvent>(val_str) else {
                    continue;
                };
                match evt.kind.as_str() {
                    "message" => {
                        if let (Some(role), Some(content)) = (evt.role, evt.content) {
                            if !content.is_empty() {
                                messages.push(ChatMsg {
                                    role,
                                    content,
                                    metadata: evt.metadata,
                                });
                            }
                        }
                    }
                    "running_tools" => {
                        active_tools.set(evt.running_tools.unwrap_or_default());
                    }
                    "pins" => {
                        pins.set(evt.pins.unwrap_or_default());
                    }
                    "staff_pings" => {
                        staff_pings_sig.set(evt.staff_pings.unwrap_or_default());
                    }
                    "status" => {
                        status_msg.set(evt.status_message);
                    }
                    "state" => {
                        if let Some(s) = evt.state {
                            state.set(s);
                        }
                        if let Some(r) = evt.state_reason {
                            state_reason.set(Some(r));
                        }
                    }
                    "done" => {
                        active_tools.set(Vec::new());
                        status_msg.set(None);
                        if let Some(s) = evt.state {
                            state.set(s);
                        }
                        if let Some(r) = evt.state_reason {
                            state_reason.set(Some(r));
                        }
                        running.set(false);
                        break;
                    }
                    "_closed" | "error" => {
                        if *running.peek() {
                            running.set(false);
                        }
                        break;
                    }
                    _ => {} // ping, unknown
                }
            }
        }
    });

    // Auto-scroll: when new messages arrive and the user is near the bottom,
    // scroll down automatically. Uses a MutationObserver on the messages div.
    use_effect(move || {
        document::eval(
            r#"
            (function() {
                const el = document.getElementById('healer-messages');
                if (!el) return;
                const observer = new MutationObserver(() => {
                    const threshold = 200;
                    const distFromBottom = document.documentElement.scrollHeight
                        - window.scrollY - window.innerHeight;
                    if (distFromBottom < threshold) {
                        window.scrollTo({ top: document.documentElement.scrollHeight, behavior: 'smooth' });
                    }
                });
                observer.observe(el, { childList: true, subtree: true });
            })();
            "#,
        );
    });

    let back_url = format!("/fleet/{}/healer", instance_id);

    let meta = session_meta
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let model_label = meta.as_ref().and_then(|m| m.model.clone()).unwrap_or_default();
    let session_label = meta.as_ref().and_then(|m| m.label.clone()).unwrap_or_default();
    let is_auto = meta.as_ref().map(|m| m.created_by.starts_with("auto:")).unwrap_or(false);

    rsx! {
        h2 { class: "text-2xl font-bold mb-4",
            "Healer Session"
            if !session_label.is_empty() {
                span { class: "ml-2 text-lg font-normal text-gray-500 dark:text-gray-400", "— {session_label}" }
            }
        }
        p { class: "text-sm text-gray-500 dark:text-gray-400 mb-4",
            "Instance: {iid} — Session: {session_id}"
            if is_auto {
                span { class: "ml-2 px-1.5 py-0.5 text-xs bg-amber-100 dark:bg-amber-900 text-amber-700 dark:text-amber-300 rounded",
                    "auto-triggered"
                }
            }
            if !model_label.is_empty() {
                span { class: "ml-2 px-1.5 py-0.5 text-xs font-mono bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 rounded",
                    "{model_label}"
                }
            }
        }

        div { class: "mb-3 flex items-center gap-3 flex-wrap",
            {
                let st = state.read().clone();
                let (badge_class, label) = state_badge(&st);
                let reason = state_reason.read().clone();
                rsx! {
                    span { class: "inline-block px-2 py-1 text-xs font-medium rounded {badge_class}", "{label}" }
                    if let Some(reason) = reason {
                        span { class: "text-xs text-gray-500 dark:text-gray-400 italic",
                            "({reason_display(&reason)})"
                        }
                    }
                }
            }

            if *running.read() {
                button {
                    class: "px-3 py-1 text-xs font-medium bg-yellow-600 text-white rounded hover:bg-yellow-700",
                    onclick: {
                        let sid = session_id.clone();
                        move |_| {
                            let sid = sid.clone();
                            async move { let _ = pause_healer_session(sid).await; }
                        }
                    },
                    "Pause"
                }
                button {
                    class: "px-3 py-1 text-xs font-medium bg-red-600 text-white rounded hover:bg-red-700",
                    onclick: {
                        let sid = session_id.clone();
                        move |_| {
                            let sid = sid.clone();
                            async move { let _ = cancel_healer_session(sid).await; }
                        }
                    },
                    "Cancel"
                }
            }

            {
                let st = state.read().clone();
                let reason = state_reason.read().clone();
                if st == "paused" {
                    let is_budget = reason.as_deref() == Some("token_budget_exceeded");
                    rsx! {
                        button {
                            class: "px-3 py-1 text-xs font-medium bg-yellow-600 text-white rounded hover:bg-yellow-700",
                            onclick: {
                                let sid = session_id.clone();
                                move |_| {
                                    let sid = sid.clone();
                                    async move { let _ = resume_healer_session(sid).await; }
                                }
                            },
                            "Resume"
                        }
                        if is_budget {
                            button {
                                class: "px-3 py-1 text-xs font-medium bg-emerald-600 text-white rounded hover:bg-emerald-700",
                                onclick: {
                                    let sid = session_id.clone();
                                    move |_| {
                                        let sid = sid.clone();
                                        async move {
                                            let _ = extend_healer_budget(sid.clone()).await;
                                            let _ = resume_healer_session(sid).await;
                                        }
                                    }
                                },
                                "More Tokens (1M)"
                            }
                        }
                    }
                } else {
                    rsx! {}
                }
            }

            Link {
                to: back_url,
                class: "px-3 py-1 text-xs font-medium bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded hover:bg-gray-300",
                "Back to sessions"
            }
        }

        // Pinned slots (from stream)
        { render_pinned_slots_from_signal(&pins.read()) }

        // Chat messages (filter out pin messages — shown above)
        div { id: "healer-messages", class: "space-y-2",
            for msg in messages.read().iter().filter(|m| m.role != "pin") {
                {render_message(msg)}
            }

            // Running tools
            for tool in active_tools.read().iter() {
                {
                    let name = tool.name.clone();
                    let args_short = tool.args.as_deref()
                        .filter(|a| *a != "{}")
                        .map(|a| if a.len() > 120 { format!("{}...", &a[..120]) } else { a.to_string() })
                        .unwrap_or_default();
                    let has_args = !args_short.is_empty();
                    rsx! {
                        div { class: "px-3 py-2 rounded bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-200 dark:border-indigo-800 flex items-center gap-2",
                            span { class: "inline-block w-2 h-2 rounded-full bg-indigo-400 animate-pulse" }
                            span { class: "text-xs font-mono font-semibold text-indigo-700 dark:text-indigo-300", "{name}" }
                            if has_args {
                                span { class: "text-xs text-gray-500 dark:text-gray-400 truncate", "{args_short}" }
                            }
                        }
                    }
                }
            }

            // Ephemeral status
            if let Some(msg) = &*status_msg.read() {
                div { class: "px-3 py-2 rounded bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 text-sm text-amber-800 dark:text-amber-300 animate-pulse",
                    "{msg}"
                }
            }

            if *running.read() && active_tools.read().is_empty() && status_msg.read().is_none() {
                div { class: "p-3 text-sm text-gray-400 animate-pulse", "Agent is thinking..." }
            }
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

fn reason_display(reason: &str) -> &str {
    match reason {
        "manual_pause" => "paused by user",
        "token_budget_exceeded" => "token budget exceeded",
        "proxy_token_expiring" => "proxy token expiring",
        "server_shutdown" => "server shutdown",
        other => other,
    }
}

fn state_badge(st: &str) -> (&'static str, &'static str) {
    match st {
        "starting" | "loading" | "created" | "initializing" => (
            "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300",
            "Initializing",
        ),
        "diagnosing" => (
            "bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300",
            "Diagnosing",
        ),
        "remediating" => (
            "bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-300",
            "Remediating",
        ),
        "verifying" => (
            "bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-300",
            "Verifying",
        ),
        "completed" | "done" => (
            "bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-300",
            "Done",
        ),
        "failed" => (
            "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300",
            "Failed",
        ),
        "cancelled" => (
            "bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300",
            "Cancelled",
        ),
        "paused" => (
            "bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300",
            "Paused",
        ),
        "awaiting_retry" => (
            "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300",
            "Awaiting retry",
        ),
        "needs_human_attention" => (
            "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300",
            "Needs Human Attention",
        ),
        _ => (
            "bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300",
            "Unknown",
        ),
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

/// Derive a left-border color class from a state name, matching `state_badge` hues.
fn state_border(st: &str) -> &'static str {
    match st {
        "starting" | "loading" | "created" | "initializing" | "awaiting_retry" => {
            "border-blue-300 dark:border-blue-700"
        }
        "diagnosing" | "paused" => "border-yellow-300 dark:border-yellow-700",
        "remediating" => "border-orange-300 dark:border-orange-700",
        "verifying" => "border-purple-300 dark:border-purple-700",
        "completed" | "done" => "border-green-300 dark:border-green-700",
        "failed" | "needs_human_attention" => "border-red-300 dark:border-red-700",
        "cancelled" | _ => "border-gray-300 dark:border-gray-700",
    }
}

/// Render a state_change message in the same style as agent/system messages.
fn render_state_change(msg: &ChatMsg) -> Element {
    let (state, reason) = if let Ok(data) = serde_json::from_str::<serde_json::Value>(&msg.content) {
        (
            data.get("state").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
            data.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        )
    } else {
        ("unknown".to_string(), String::new())
    };

    let (badge_bg, label) = state_badge(&state);
    let border = state_border(&state);
    let reason_text = reason_display(&reason);

    rsx! {
        div { class: "p-3 rounded bg-gray-50/50 dark:bg-gray-800/50 border-l-4 {border}",
            div { class: "flex items-center gap-1.5 mb-1",
                span { class: "w-5 h-5 flex items-center justify-center rounded-full bg-gray-200 dark:bg-gray-700 text-xs font-bold text-gray-600 dark:text-gray-300", "S" }
                span { class: "text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider", "State Change" }
            }
            div { class: "flex items-center gap-2",
                span { class: "text-xs font-semibold px-2 py-0.5 rounded-full {badge_bg}", "{label}" }
                if !reason.is_empty() {
                    span { class: "text-sm text-gray-600 dark:text-gray-300", "{reason_text}" }
                }
            }
        }
    }
}

/// Render a chat message. Tool calls/results get special UI.
/// Assistant/system messages are rendered as markdown via dangerous_inner_html.
fn render_message(msg: &ChatMsg) -> Element {
    if msg.role == "tool_result" {
        return render_tool_result(msg);
    }

    if msg.role == "state_change" {
        return render_state_change(msg);
    }

    let (bg, icon, label) = match msg.role.as_str() {
        "system" => (
            "bg-gray-50 dark:bg-gray-800 border-l-4 border-gray-400",
            "S",
            "System",
        ),
        "assistant" => (
            "bg-blue-50 dark:bg-blue-900/20 border-l-4 border-blue-400",
            "A",
            "Agent",
        ),
        "user" => (
            "bg-green-50 dark:bg-green-900/20 border-l-4 border-green-400",
            "U",
            "User",
        ),
        "summary" => (
            "bg-purple-50 dark:bg-purple-900/20 border-l-4 border-purple-400",
            "S",
            "Summary",
        ),
        _ => (
            "bg-gray-50 dark:bg-gray-800 border-l-4 border-gray-300",
            "-",
            "Other",
        ),
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
    let (tool_name, result) = msg
        .content
        .split_once(": ")
        .unwrap_or(("tool", &msg.content));
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
    let args = msg
        .metadata
        .as_ref()
        .and_then(|m| m.get("tool_args"))
        .and_then(|v| v.as_str())
        .filter(|a| *a != "{}")
        .unwrap_or("");
    let args_short = if args.len() > 120 {
        format!("{}...", &args[..120])
    } else {
        args.to_string()
    };
    let has_args = !args.is_empty();

    rsx! {
        div { class: "p-2 rounded bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700",
            details { class: "group",
                summary { class: "flex items-center gap-2 cursor-pointer select-none",
                    span { class: "{tool_badge}", "{tool_name}" }
                    if has_args {
                        span { class: "text-xs text-gray-500 dark:text-gray-400 truncate max-w-md", "{args_short}" }
                    }
                    if is_error {
                        span { class: "text-xs text-red-500", "error" }
                    }
                }
                if has_args {
                    pre { class: "mt-2 p-2 text-xs font-mono bg-gray-100 dark:bg-gray-900 text-gray-600 dark:text-gray-400 rounded overflow-x-auto max-h-32 overflow-y-auto whitespace-pre-wrap",
                        "{args}"
                    }
                }
                pre { class: "mt-1 p-2 text-xs font-mono bg-gray-900 text-green-400 rounded overflow-x-auto max-h-64 overflow-y-auto whitespace-pre-wrap",
                    "{display}"
                }
            }
        }
    }
}

fn render_pinned_slots_from_signal(pins: &[PinInfo]) -> Element {
    if pins.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "mb-4 space-y-2",
            for pin in pins.iter() {
                {
                    let (icon, label, border) = match pin.slot.as_str() {
                        "diagnosis" => ("D", "Diagnosis", "border-yellow-400 dark:border-yellow-600"),
                        "remediation" => ("R", "Remediation Plan", "border-orange-400 dark:border-orange-600"),
                        "final_report" => ("F", "Final Report", "border-green-400 dark:border-green-600"),
                        _ => ("P", pin.slot.as_str(), "border-gray-400"),
                    };
                    let html = simple_md_to_html(&pin.summary);
                    let services = pin.affected_services.clone();
                    rsx! {
                        div { class: "p-3 bg-white dark:bg-gray-800 rounded border-l-4 {border} shadow-sm",
                            div { class: "flex items-center gap-1.5 mb-1",
                                span { class: "w-5 h-5 flex items-center justify-center rounded-full bg-gray-200 dark:bg-gray-700 text-xs font-bold text-gray-600 dark:text-gray-300", "{icon}" }
                                span { class: "text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider", "{label}" }
                            }
                            div {
                                class: "text-sm text-gray-800 dark:text-gray-200 prose prose-sm dark:prose-invert max-w-none",
                                dangerous_inner_html: "{html}",
                            }
                            if !services.is_empty() {
                                div { class: "mt-2 flex flex-wrap gap-1",
                                    for svc in services.iter() {
                                        span { class: "px-1.5 py-0.5 text-xs bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 rounded", "{svc}" }
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

fn render_staff_pings_inline(pings: &[StaffPingSummary]) -> Element {
    if pings.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "mb-4 p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded",
            h4 { class: "text-sm font-semibold text-amber-800 dark:text-amber-300 mb-2", "Staff Pings" }
            div { class: "space-y-2",
                for ping in pings.iter() {
                    {
                        let cat_badge = category_badge(&ping.category);
                        rsx! {
                            div { class: "flex items-start gap-2 text-sm",
                                div { class: "flex-1",
                                    span { class: "inline-block px-1.5 py-0.5 text-xs font-medium rounded mr-2 {cat_badge}", "{ping.category}" }
                                    if ping.resolved {
                                        span { class: "text-green-600 dark:text-green-400 line-through", "{ping.message}" }
                                    } else {
                                        span { class: "text-gray-800 dark:text-gray-200", "{ping.message}" }
                                    }
                                    span { class: "text-xs text-gray-400 ml-2", "{ping.created_at}" }
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
pub fn simple_md_to_html(md: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(md, options);
    let mut output = String::with_capacity(md.len() * 2);
    html::push_html(&mut output, parser);
    output
}
