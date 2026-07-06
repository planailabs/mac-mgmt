use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    ActiveSessionCard, Badge, BadgeVariant, Kicker, Pill, PillVariant, TraceStatus, TraceStep,
};
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

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
    pub auto_approve: bool,
    pub fix_provider: Option<String>,
    pub fix_model: Option<String>,
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
pub fn running_tools_to_wire(
    tools: &[mac_mgmt_healer::session::RunningTool],
) -> Vec<RunningToolInfo> {
    tools
        .iter()
        .map(|t| RunningToolInfo {
            name: t.name.clone(),
            args: t.args.clone(),
            started_at: t.started_at.to_rfc3339(),
            validation: t
                .validation
                .as_ref()
                .map(|v| mac_mgmt_common::HealerToolValidation {
                    status: v.status.clone(),
                    reasoning: v.reasoning.clone(),
                    risk: v.risk.clone(),
                }),
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
        state_data: serde_json::Value,
    }
    let sessions = sqlx::query_as::<_, SessRow>(
        "SELECT id, state, created_by, created_at, error_message, provider, model, label, state_data \
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
    .map(|r| {
        let auto_approve = r.state_data.get("auto_approve").and_then(|v| v.as_bool()).unwrap_or(false);
        let fix_provider = r.state_data.get("fix_provider").and_then(|v| v.as_str()).map(String::from);
        let fix_model = r.state_data.get("fix_model").and_then(|v| v.as_str()).map(String::from);
        SessionSummary {
            id: r.id.to_string(),
            state: r.state,
            created_by: r.created_by,
            created_at: r.created_at.format("%Y-%m-%d %H:%M").to_string(),
            error_message: r.error_message,
            provider: r.provider,
            model: r.model,
            label: r.label,
            auto_approve,
            fix_provider,
            fix_model,
        }
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
    validator_provider: Option<String>,
    validator_model: Option<String>,
) -> Result<String, ServerFnError> {
    use mac_mgmt_healer::SpawnRequest;
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
        failure_signals: Option<serde_json::Value>,
        file_tunnels: Option<serde_json::Value>,
        shell_tunnels: Option<serde_json::Value>,
        sample: Option<serde_json::Value>,
        hostname: Option<String>,
    }
    let hb: HbInfo = sqlx::query_as(
        "SELECT cluster_id, relay_proxy_url, services_extended, failure_signals, \
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
    let healer_scopes: &[&str] = &["files:read", "files:write", "shell:exec", "logs:read"];
    let (proxy_token, proxy_expires) = pg_store
        .mint_proxy_token_scoped(hb.cluster_id, None, Some(healer_scopes))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let relay_client = std::sync::Arc::new(mac_mgmt_healer::relay_client::RelayClient::new(
        relay_url.clone(),
        proxy_token,
    ));
    let instance_prefix: String = instance_id.chars().take(12).collect();
    let instance_access: mac_mgmt_healer::DynInstanceAccess =
        std::sync::Arc::new(mac_mgmt_healer::relay_client::RelayInstanceAccess::new(
            relay_client.clone(),
            instance_prefix,
        ));
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
    let failure_signals: Vec<mac_mgmt_common::FailureSignal> = hb
        .failure_signals
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    // Per-cluster healer settings from dedicated table.
    let cluster_healer = {
        #[derive(sqlx::FromRow)]
        struct Row {
            auto_trigger: Option<bool>,
            auto_trigger_provider: Option<String>,
            auto_trigger_model: Option<String>,
            auto_approve: Option<bool>,
            fix_provider: Option<String>,
            fix_model: Option<String>,
        }
        sqlx::query_as::<_, Row>(
            "SELECT auto_trigger, auto_trigger_provider, auto_trigger_model, \
                    auto_approve, fix_provider, fix_model \
             FROM healer_cluster_settings WHERE cluster_id = $1",
        )
        .bind(hb.cluster_id)
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten()
        .map(|r| mac_mgmt_common::HealerClusterConfig {
            auto_trigger: r.auto_trigger,
            auto_trigger_provider: r.auto_trigger_provider,
            auto_trigger_model: r.auto_trigger_model,
            auto_approve: r.auto_approve,
            fix_provider: r.fix_provider,
            fix_model: r.fix_model,
            fine_tuned_model: None,
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
        failure_signals,
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
                models
                    .iter()
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
        validator_provider,
        validator_model,
        ml_hints: None,
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
        .await
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
    healer
        .store()
        .resolve_staff_ping(uuid, &user.email)
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

    // Title shows the hostname (the recognizable handle) and the
    // subtitle disambiguates with the page label, mirroring the
    // design's "Healer Agent · autonomous remediation" pattern.
    let topbar_title = match &*ctx.read() {
        Some(Ok(c)) if !c.hostname.is_empty() => c.hostname.clone(),
        Some(Ok(c)) => c.instance_id.clone(),
        _ => String::new(),
    };
    use_topbar(topbar_title, Some(t!("healer-title").to_string()));

    match &*ctx.read() {
        Some(Ok(c)) => render_healer(c),
        Some(Err(e)) => {
            rsx! { p { class: "text-danger text-sm", {t!("error-message", message: e.to_string())} } }
        }
        None => rsx! { p { class: "text-fg-muted text-sm", {t!("loading")} } },
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
    let mut selected_validator_key = use_signal(|| "none".to_string());
    let mut auto_approve = use_signal(|| false);
    let mut settings_open = use_signal(|| false);
    let models = ctx.models.clone();

    let unhealthy: Vec<String> = ctx
        .services_extended
        .iter()
        .filter(|s| s.get("healthy").and_then(|v| v.as_bool()) == Some(false))
        .filter_map(|s| s.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();

    let instance_id = ctx.instance_id.clone();
    let sessions = ctx.sessions.clone();

    // ── Page hero ─────────────────────────────────────────────────
    // Design language reference shows the healer hero as
    //   "HEALER AGENT · LAST 24H"
    //   "<accent>128</accent> sessions · <ok>97%</ok> auto-resolved"
    // We don't yet aggregate session counts here — that lives in the
    // global healer summary endpoint — so this hero just identifies the
    // instance and lets the running session card carry the live signal.
    let session_count = ctx.sessions.len();
    rsx! {
        div { class: "flex flex-col xl:flex-row xl:items-end xl:justify-between gap-3 mb-5",
            div {
                Kicker { class: "mb-2", {t!("healer-title")} }
                h1 { class: "h-page mb-0",
                    {t!("healer-title")}
                    " "
                    span { class: "text-fg-muted", "·" }
                    " "
                    span { class: "text-brand", "{ctx.hostname}" }
                }
                div { class: "mt-2 text-fg-muted text-sm",
                    {t!("healer-instance", instance_id: ctx.instance_id.clone(), hostname: ctx.hostname.clone())}
                }
            }
            div { class: "flex items-center gap-2 shrink-0",
                Pill { variant: PillVariant::Muted, mono: true,
                    "{session_count} sessions"
                }
                if unhealthy.is_empty() {
                    Pill { variant: PillVariant::Ok, "all healthy" }
                } else {
                    Pill { variant: PillVariant::Bad,
                        "{unhealthy.len()} unhealthy"
                    }
                }
            }
        }

        if !unhealthy.is_empty() {
            div { class: "alert alert-danger mb-4",
                p { class: "text-sm font-medium text-danger-strong",
                    {t!("healer-unhealthy", services: unhealthy.join(", "))}
                }
            }
        }

        // Cluster healer settings (collapsible)
        {
            let cluster_id = ctx.cluster_id.clone();
            rsx! {
                div { class: "mb-4",
                    button {
                        class: "text-sm text-fg-muted hover:text-fg-strong flex items-center gap-1",
                        onclick: move |_| { let v = *settings_open.read(); settings_open.set(!v); },
                        {t!("healer-settings")}
                        span { class: "text-xs", if *settings_open.read() { "\u{25BC}" } else { "\u{25B6}" } }
                    }
                    if *settings_open.read() {
                        div { class: "mt-2 p-4 bg-surface rounded shadow border border-line-soft",
                            super::cluster_healer_settings::ClusterHealerSettings {
                                cluster_id: cluster_id,
                                read_only: false,
                            }
                        }
                    }
                }
            }
        }

        // New session controls
        if !*running.read() && session_id.read().is_none() {
            {
                let ollama_models: Vec<ModelEntry> = models.iter().filter(|m| m.provider == "ollama").cloned().collect();
                let anthropic_models: Vec<ModelEntry> = models.iter().filter(|m| m.provider == "anthropic").cloned().collect();
                let openrouter_models: Vec<ModelEntry> = models.iter().filter(|m| m.provider == "openrouter").cloned().collect();
                // Everything else is a named OpenAI-compatible source.
                let other_models: Vec<ModelEntry> = models.iter().filter(|m| !matches!(m.provider.as_str(), "ollama" | "anthropic" | "openrouter")).cloned().collect();
                // Build option values as "provider:model"
                let first_key = models.first().map(|m| format!("{}:{}", m.provider, m.model)).unwrap_or_default();
                rsx! {
                    div { class: "mb-6 p-4 bg-surface rounded shadow",
                        h3 { class: "text-lg font-semibold mb-3", {t!("healer-new-session")} }

                        // Model selector
                        div { class: "mb-3",
                            label { class: "block text-sm font-medium text-fg mb-1", {t!("healer-model")} }
                            select {
                                class: "w-full px-3 py-2 text-sm border rounded-md ",
                                value: "{selected_model_key}",
                                onchange: move |e| selected_model_key.set(e.value()),
                                if !ollama_models.is_empty() {
                                    optgroup { label: t!("healer-ollama-free"),
                                        for m in ollama_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !anthropic_models.is_empty() {
                                    optgroup { label: t!("healer-anthropic-cloud"),
                                        for m in anthropic_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !openrouter_models.is_empty() {
                                    optgroup { label: t!("healer-openrouter-cloud"),
                                        for m in openrouter_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !other_models.is_empty() {
                                    optgroup { label: "OpenAI-compatible",
                                        for m in other_models.iter() {
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
                                        p { class: "mt-1 text-xs text-fg-muted",
                                            {t!("healer-ollama-hint")}
                                        }
                                    }
                                } else if key.starts_with("openrouter:") {
                                    rsx! {
                                        p { class: "mt-1 text-xs text-fg-muted",
                                            {t!("healer-openrouter-hint")}
                                        }
                                    }
                                } else {
                                    rsx! {
                                        p { class: "mt-1 text-xs text-fg-muted",
                                            {t!("healer-anthropic-hint")}
                                        }
                                    }
                                }
                            }
                        }

                        // Fix-model selector (optional, for remediation phase)
                        div { class: "mb-3",
                            label { class: "block text-sm font-medium text-fg mb-1", {t!("healer-fix-model")} }
                            select {
                                class: "w-full px-3 py-2 text-sm border rounded-md ",
                                value: "{selected_fix_model_key}",
                                onchange: move |e| selected_fix_model_key.set(e.value()),
                                option { value: "none", {t!("healer-same-as-diagnosis")} }
                                if !ollama_models.is_empty() {
                                    optgroup { label: t!("healer-ollama-free"),
                                        for m in ollama_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !anthropic_models.is_empty() {
                                    optgroup { label: t!("healer-anthropic-cloud"),
                                        for m in anthropic_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !openrouter_models.is_empty() {
                                    optgroup { label: t!("healer-openrouter-cloud"),
                                        for m in openrouter_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !other_models.is_empty() {
                                    optgroup { label: "OpenAI-compatible",
                                        for m in other_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                            }
                            p { class: "mt-1 text-xs text-fg-muted",
                                {t!("healer-fix-model-hint")}
                            }
                        }

                        // Validator model selector
                        div { class: "mb-3",
                            label { class: "block text-sm font-medium text-fg mb-1", "Validator Model" }
                            select {
                                class: "w-full px-3 py-2 text-sm border rounded-md ",
                                value: "{selected_validator_key}",
                                onchange: move |e| selected_validator_key.set(e.value()),
                                option { value: "none", "None (static checks only)" }
                                if !ollama_models.is_empty() {
                                    optgroup { label: t!("healer-ollama-free"),
                                        for m in ollama_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !anthropic_models.is_empty() {
                                    optgroup { label: t!("healer-anthropic-cloud"),
                                        for m in anthropic_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !openrouter_models.is_empty() {
                                    optgroup { label: t!("healer-openrouter-cloud"),
                                        for m in openrouter_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                                if !other_models.is_empty() {
                                    optgroup { label: "OpenAI-compatible",
                                        for m in other_models.iter() {
                                            { let key = format!("{}:{}", m.provider, m.model); rsx! {
                                                option { value: "{key}", "{m.name}" }
                                            }}
                                        }
                                    }
                                }
                            }
                            p { class: "mt-1 text-xs text-fg-muted",
                                "Optional second model that validates each tool call before execution."
                            }
                        }

                        div { class: "mb-3",
                            textarea {
                                class: "w-full px-3 py-2 text-sm border rounded-md ",
                                rows: "2",
                                placeholder: t!("healer-instructions-placeholder"),
                                value: "{user_input}",
                                oninput: move |e| user_input.set(e.value()),
                            }
                        }
                        div { class: "mb-3 flex items-center gap-2",
                            input {
                                r#type: "checkbox",
                                id: "auto-approve",
                                class: "rounded border-line",
                                checked: *auto_approve.read(),
                                onchange: move |e| auto_approve.set(e.checked()),
                            }
                            label {
                                r#for: "auto-approve",
                                class: "text-sm text-fg",
                                {t!("healer-auto-approve")}
                            }
                            p { class: "text-xs text-fg-muted",
                                {t!("healer-skip-approval")}
                            }
                        }
                        button {
                            class: "btn btn-md btn-primary",
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
                                    let val_key = selected_validator_key.read().clone();
                                    let (val_provider, val_model) = if val_key != "none" {
                                        if let Some((p, m)) = val_key.split_once(':') {
                                            (Some(p.to_string()), Some(m.to_string()))
                                        } else {
                                            (None, None)
                                        }
                                    } else {
                                        (None, None)
                                    };
                                    async move {
                                        match start_healer_session(instance_id.clone(), user_msg, provider, model, fix_provider, fix_model, approve, val_provider, val_model).await {
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
                            {t!("healer-start")}
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
                        let (badge_variant, label) = state_badge(&st);
                        let reason = state_reason.read().clone();
                        rsx! {
                            Badge { variant: badge_variant, "{label}" }
                            if let Some(reason) = reason {
                                span { class: "text-xs text-fg-muted italic",
                                    "({reason_display(&reason)})"
                                }
                            }
                        }
                    }

                    if *running.read() {
                        button {
                            class: "btn btn-xs btn-warn",
                            onclick: move |_| {
                                let sid = session_id.read().clone();
                                async move {
                                    if let Some(sid) = sid { let _ = pause_healer_session(sid).await; }
                                }
                            },
                            {t!("healer-pause")}
                        }
                        button {
                            class: "btn btn-xs btn-danger",
                            onclick: move |_| {
                                let sid = session_id.read().clone();
                                async move {
                                    if let Some(sid) = sid { let _ = cancel_healer_session(sid).await; }
                                }
                            },
                            {t!("healer-cancel-session")}
                        }
                    }

                    {
                        let st = state.read().clone();
                        if st == "paused" {
                            rsx! {
                                button {
                                    class: "btn btn-xs btn-warn",
                                    onclick: move |_| {
                                        let sid = session_id.read().clone();
                                        async move {
                                            if let Some(sid) = sid { let _ = resume_healer_session(sid).await; }
                                        }
                                    },
                                    {t!("healer-resume")}
                                }
                            }
                        } else {
                            rsx! {}
                        }
                    }

                    if !*running.read() {
                        button {
                            class: "btn btn-xs btn-secondary",
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
                            {t!("healer-back-to-sessions")}
                        }
                    }
                }

                // ── Active session card (design language hero) ──────
                // When the healer is working on a request, surface the
                // current state in a `ActiveSessionCard` so an operator
                // glancing at the page sees what's happening at a
                // glance — title pulled from the most recent user
                // message, trace synthesised from the chat state and
                // any running tools. The chat below stays the source
                // of truth; this is a visual recap.
                if *running.read() {
                    {
                        let messages_snap = messages.read();
                        let last_user = messages_snap.iter()
                            .rev()
                            .find(|m| m.role == "user")
                            .map(|m| {
                                let text = m.content.lines().next().unwrap_or("").trim();
                                if text.len() > 80 {
                                    format!("{}…", &text[..80])
                                } else {
                                    text.to_string()
                                }
                            })
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| t!("healer-thinking").to_string());

                        let svc_count = ctx.services_extended.len();
                        let tools_snap = active_tools.read();
                        let current_tool = tools_snap.first().map(|t| t.name.clone());
                        let model_label = selected_model_key.read().clone();

                        // Four-step abstract trace. The first two are
                        // always done by the time the chat is running;
                        // step 3 reflects whatever tool is in flight (or
                        // "Investigating" while the model is thinking);
                        // step 4 is the post-condition we're moving
                        // towards.
                        let mut trace: Vec<TraceStep> = vec![
                            TraceStep {
                                status: TraceStatus::Done,
                                text: t!(
                                    "healer-instance",
                                    instance_id: ctx.instance_id.clone(),
                                    hostname: ctx.hostname.clone()
                                ).to_string(),
                            },
                            TraceStep {
                                status: TraceStatus::Done,
                                text: format!("loaded service status · {svc_count} services"),
                            },
                        ];
                        let current_step = if let Some(name) = current_tool.as_ref() {
                            trace.push(TraceStep {
                                status: TraceStatus::InProgress,
                                text: format!("running tool · {name}"),
                            });
                            3
                        } else {
                            trace.push(TraceStep {
                                status: TraceStatus::InProgress,
                                text: t!("healer-thinking").to_string(),
                            });
                            3
                        };
                        trace.push(TraceStep {
                            status: TraceStatus::Pending,
                            text: t!("healer-verify-outcome").to_string(),
                        });

                        let subtitle = if model_label.is_empty() {
                            None
                        } else {
                            Some(format!("model · {model_label}"))
                        };

                        rsx! {
                            div { class: "mb-4",
                                ActiveSessionCard {
                                    title: last_user,
                                    subtitle,
                                    trace,
                                    current_step,
                                }
                            }
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
                            // Risk badge
                            let risk_badge = tool.validation.as_ref().map(|v| v.risk.as_str()).unwrap_or("");
                            let badge_class = match risk_badge {
                                "mutating" => Some(("M", "bg-warn text-warn-strong")),
                                "destructive" => Some(("D", "bg-danger text-danger-strong")),
                                _ => None,
                            };
                            let is_validating = tool.validation.as_ref()
                                .is_some_and(|v| v.status == "validating");
                            rsx! {
                                div { class: "px-3 py-2 rounded bg-accent-soft border border-accent flex items-center gap-2",
                                    span { class: "inline-block w-2 h-2 rounded-full bg-accent animate-pulse" }
                                    if let Some((label, cls)) = badge_class {
                                        span { class: "text-[10px] font-bold px-1 rounded {cls}", "{label}" }
                                    }
                                    if is_validating {
                                        span { class: "text-[10px] font-semibold px-1.5 py-0.5 rounded bg-warn-soft text-warn-strong animate-pulse", "Validating..." }
                                    }
                                    span { class: "text-xs font-mono font-semibold text-accent", "{name}" }
                                    if has_args {
                                        span { class: "text-xs text-fg-muted truncate", "{args_short}" }
                                    }
                                }
                            }
                        }
                    }

                    // Ephemeral status (e.g. "Waiting for daemon reconnect...")
                    if let Some(msg) = &*status_msg.read() {
                        div { class: "px-3 py-2 rounded bg-warn-soft border border-warn text-sm text-warn-strong animate-pulse",
                            "{msg}"
                        }
                    }

                    if *running.read() && active_tools.read().is_empty() && status_msg.read().is_none() {
                        div { class: "p-3 text-sm text-fg-faint animate-pulse", {t!("healer-thinking")} }
                    }
                }
            }
        }

        // Previous sessions list
        if session_id.read().is_none() && !*running.read() && !sessions.is_empty() {
            div { class: "mt-6",
                h3 { class: "text-lg font-semibold mb-3", {t!("healer-previous-sessions")} }
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
                            let is_awaiting_approval = sess_state == "awaiting_approval";
                            let has_auto_approve = sess.auto_approve;
                            let fix_model_label = sess.fix_model.clone().unwrap_or_default();
                            let (badge_variant, badge_label) = state_badge(&sess_state);
                            let url = format!("/fleet/{}/healer/{}", instance_id, sid);

                            rsx! {
                                Link {
                                    to: url,
                                    class: "flex items-center justify-between p-3 bg-surface rounded shadow hover:bg-surface-2 cursor-pointer",
                                    div { class: "flex items-center gap-3 flex-wrap",
                                        Badge { variant: badge_variant, "{badge_label}" }
                                        if is_awaiting_approval {
                                            span { class: "badge badge-warn animate-pulse", {t!("healer-approval-pending")} }
                                        }
                                        if is_auto {
                                            span { class: "badge badge-warn", {t!("healer-auto")} }
                                        }
                                        if has_auto_approve {
                                            span { class: "badge badge-success", {t!("healer-auto-approve-label")} }
                                        }
                                        if !session_label.is_empty() {
                                            span { class: "text-sm font-medium text-fg truncate max-w-xs", "{session_label}" }
                                        }
                                        span { class: "text-sm text-fg", "{created_at}" }
                                        if !model_label.is_empty() {
                                            Badge { variant: BadgeVariant::Neutral, class: "font-mono", "{model_label}" }
                                        }
                                        if !fix_model_label.is_empty() {
                                            span { class: "badge badge-accent font-mono", "fix: {fix_model_label}" }
                                        }
                                        if !is_auto {
                                            span { class: "text-xs text-fg-muted", "{created_by}" }
                                        }
                                    }
                                    div { class: "flex items-center gap-2",
                                        if let Some(err) = &error_msg {
                                            span { class: "text-xs text-danger max-w-xs truncate", "{err}" }
                                        }
                                        span { class: "text-xs text-fg-faint", {t!("healer-view")} }
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
    let model_label = meta
        .as_ref()
        .and_then(|m| m.model.clone())
        .unwrap_or_default();
    let session_label = meta
        .as_ref()
        .and_then(|m| m.label.clone())
        .unwrap_or_default();
    let is_auto = meta
        .as_ref()
        .map(|m| m.created_by.starts_with("auto:"))
        .unwrap_or(false);

    rsx! {
        h2 { class: "h-page",
            {t!("healer-session-title")}
            if !session_label.is_empty() {
                span { class: "ml-2 text-lg font-normal text-fg-muted", "— {session_label}" }
            }
        }
        p { class: "text-sm text-fg-muted mb-4",
            {t!("healer-session-subtitle", instance_id: iid.clone(), session_id: session_id.clone())}
            if is_auto {
                Badge { variant: BadgeVariant::Warn, class: "ml-2", {t!("healer-auto-triggered")} }
            }
            if !model_label.is_empty() {
                Badge { variant: BadgeVariant::Neutral, class: "ml-2 font-mono", "{model_label}" }
            }
        }

        div { class: "mb-3 flex items-center gap-3 flex-wrap",
            {
                let st = state.read().clone();
                let (badge_variant, label) = state_badge(&st);
                let reason = state_reason.read().clone();
                rsx! {
                    Badge { variant: badge_variant, "{label}" }
                    if let Some(reason) = reason {
                        span { class: "text-xs text-fg-muted italic",
                            "({reason_display(&reason)})"
                        }
                    }
                }
            }

            if *running.read() {
                button {
                    class: "btn btn-xs btn-warn",
                    onclick: {
                        let sid = session_id.clone();
                        move |_| {
                            let sid = sid.clone();
                            async move { let _ = pause_healer_session(sid).await; }
                        }
                    },
                    {t!("healer-pause")}
                }
                button {
                    class: "btn btn-xs btn-danger",
                    onclick: {
                        let sid = session_id.clone();
                        move |_| {
                            let sid = sid.clone();
                            async move { let _ = cancel_healer_session(sid).await; }
                        }
                    },
                    {t!("healer-cancel-session")}
                }
            }

            {
                let st = state.read().clone();
                let reason = state_reason.read().clone();
                if st == "paused" {
                    let is_budget = reason.as_deref() == Some("token_budget_exceeded");
                    rsx! {
                        button {
                            class: "btn btn-xs btn-warn",
                            onclick: {
                                let sid = session_id.clone();
                                move |_| {
                                    let sid = sid.clone();
                                    async move { let _ = resume_healer_session(sid).await; }
                                }
                            },
                            {t!("healer-resume")}
                        }
                        if is_budget {
                            button {
                                class: "btn btn-xs btn-primary",
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
                                {t!("healer-more-tokens")}
                            }
                        }
                    }
                } else {
                    rsx! {}
                }
            }

            Link {
                to: back_url,
                class: "btn btn-xs btn-secondary",
                {t!("healer-back-to-sessions")}
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
                        div { class: "px-3 py-2 rounded bg-accent-soft border border-accent flex items-center gap-2",
                            span { class: "inline-block w-2 h-2 rounded-full bg-accent animate-pulse" }
                            span { class: "text-xs font-mono font-semibold text-accent", "{name}" }
                            if has_args {
                                span { class: "text-xs text-fg-muted truncate", "{args_short}" }
                            }
                        }
                    }
                }
            }

            // Ephemeral status
            if let Some(msg) = &*status_msg.read() {
                div { class: "px-3 py-2 rounded bg-warn-soft border border-warn text-sm text-warn-strong animate-pulse",
                    "{msg}"
                }
            }

            if *running.read() && active_tools.read().is_empty() && status_msg.read().is_none() {
                div { class: "p-3 text-sm text-fg-faint animate-pulse", "Agent is thinking..." }
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

fn reason_display(reason: &str) -> String {
    match reason {
        "manual_pause" => t!("healer-paused-by-user"),
        "token_budget_exceeded" => t!("healer-token-budget"),
        "proxy_token_expiring" => t!("healer-proxy-expiring"),
        "server_shutdown" => t!("healer-server-shutdown"),
        other => other.to_string(),
    }
}

fn state_badge(st: &str) -> (BadgeVariant, String) {
    match st {
        "starting" | "loading" | "created" | "initializing" => {
            (BadgeVariant::Info, t!("healer-state-initializing"))
        }
        "diagnosing" => (BadgeVariant::Warn, t!("healer-state-diagnosing")),
        "remediating" => (BadgeVariant::Warn, t!("healer-state-remediating")),
        "verifying" => (BadgeVariant::Accent, t!("healer-state-verifying")),
        "completed" | "done" => (BadgeVariant::Success, t!("healer-state-done")),
        "failed" => (BadgeVariant::Danger, t!("healer-state-failed")),
        "cancelled" => (BadgeVariant::Neutral, t!("healer-state-cancelled")),
        "paused" => (BadgeVariant::Warn, t!("healer-state-paused")),
        "awaiting_approval" => (BadgeVariant::Warn, t!("healer-state-awaiting-approval")),
        "awaiting_retry" => (BadgeVariant::Info, t!("healer-state-awaiting-retry")),
        "needs_human_attention" => (BadgeVariant::Danger, t!("healer-state-needs-human")),
        _ => (BadgeVariant::Neutral, t!("healer-state-unknown")),
    }
}

fn category_badge_variant(cat: &str) -> BadgeVariant {
    match cat {
        "hardware" | "service_crash" | "security" => BadgeVariant::Danger,
        "network" | "performance" => BadgeVariant::Info,
        "disk_space" | "config_error" => BadgeVariant::Warn,
        "model_issue" | "permission" | "dependency" => BadgeVariant::Accent,
        _ => BadgeVariant::Neutral,
    }
}

/// Derive a left-border color class from a state name, matching `state_badge` hues.
fn state_border(st: &str) -> &'static str {
    match st {
        "starting" | "loading" | "created" | "initializing" | "awaiting_retry" => "border-info",
        "diagnosing" | "paused" | "remediating" => "border-warn",
        "verifying" => "border-accent",
        "completed" | "done" => "border-success",
        "failed" | "needs_human_attention" => "border-danger",
        "cancelled" | _ => "border-line",
    }
}

/// Render a state_change message in the same style as agent/system messages.
fn render_state_change(msg: &ChatMsg) -> Element {
    let (state, reason) = if let Ok(data) = serde_json::from_str::<serde_json::Value>(&msg.content)
    {
        (
            data.get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            data.get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        )
    } else {
        ("unknown".to_string(), String::new())
    };

    let (badge_variant, label) = state_badge(&state);
    let border = state_border(&state);
    let reason_text = reason_display(&reason);

    rsx! {
        div { class: "p-3 rounded bg-surface-2 border-l-4 {border}",
            div { class: "flex items-center gap-1.5 mb-1",
                span { class: "w-5 h-5 flex items-center justify-center rounded-full bg-surface-3 text-xs font-bold text-fg", "S" }
                span { class: "text-xs font-semibold text-fg-muted uppercase tracking-wider", {t!("healer-event-state-change")} }
            }
            div { class: "flex items-center gap-2",
                Badge { variant: badge_variant, "{label}" }
                if !reason.is_empty() {
                    span { class: "text-sm text-fg", "{reason_text}" }
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
            "bg-surface-2 border-l-4 border-line",
            "S",
            t!("healer-event-system"),
        ),
        "assistant" => (
            "bg-info-soft border-l-4 border-info",
            "A",
            t!("healer-event-agent"),
        ),
        "user" => (
            "bg-success-soft border-l-4 border-success",
            "U",
            t!("healer-event-user"),
        ),
        "summary" => (
            "bg-accent-soft border-l-4 border-accent",
            "S",
            t!("healer-event-summary"),
        ),
        _ => (
            "bg-surface-2 border-l-4 border-line-soft",
            "-",
            t!("healer-event-other"),
        ),
    };

    let html = simple_md_to_html(&msg.content);

    rsx! {
        div { class: "p-3 rounded {bg}",
            div { class: "flex items-center gap-1.5 mb-1",
                span { class: "w-5 h-5 flex items-center justify-center rounded-full bg-surface-2 text-xs font-bold text-fg", "{icon}" }
                span { class: "text-xs font-semibold text-fg-muted uppercase tracking-wider", "{label}" }
            }
            div {
                class: "text-sm text-fg-strong prose prose-sm dark:prose-invert max-w-none",
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
    let is_rejected =
        result.starts_with("[Validation rejected]") || result.starts_with("[Validator rejected]");
    let truncated = result.len() > 500;
    let preview = if truncated { &result[..500] } else { result };
    let tool_badge = if is_rejected {
        "badge badge-danger font-mono"
    } else if is_error {
        "badge badge-danger font-mono"
    } else {
        "badge badge-neutral font-mono"
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

    // Extract validation metadata
    let _validation_status = msg
        .metadata
        .as_ref()
        .and_then(|m| m.get("validation"))
        .and_then(|v| v.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let validation_reasoning = msg
        .metadata
        .as_ref()
        .and_then(|m| m.get("validation"))
        .and_then(|v| v.get("reasoning"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let validation_risk = msg
        .metadata
        .as_ref()
        .and_then(|m| m.get("validation"))
        .and_then(|v| v.get("risk"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let risk_badge = match validation_risk {
        "mutating" => Some(("M", "bg-warn text-warn-strong")),
        "destructive" => Some(("D", "bg-danger text-danger-strong")),
        _ => None,
    };

    rsx! {
        div { class: "p-2 rounded bg-surface-2 border border-line-soft",
            details { class: "group",
                summary { class: "flex items-center gap-2 cursor-pointer select-none",
                    span { class: "{tool_badge}", "{tool_name}" }
                    if let Some((label, cls)) = risk_badge {
                        span { class: "text-[10px] font-bold px-1 rounded {cls}", "{label}" }
                    }
                    if has_args {
                        span { class: "text-xs text-fg-muted truncate max-w-md", "{args_short}" }
                    }
                    if is_rejected {
                        span { class: "text-xs font-semibold text-danger", "REJECTED" }
                    } else if is_error {
                        span { class: "text-xs text-danger", {t!("healer-event-error")} }
                    }
                }
                if has_args {
                    pre { class: "mt-2 p-2 text-xs font-mono bg-surface-3 text-fg-muted rounded overflow-x-auto max-h-32 overflow-y-auto whitespace-pre-wrap",
                        "{args}"
                    }
                }
                if !validation_reasoning.is_empty() {
                    div { class: "mt-1 px-2 py-1 text-xs rounded bg-surface-3 text-fg-muted border-l-2 border-accent",
                        span { class: "font-semibold", "Validation: " }
                        "{validation_reasoning}"
                    }
                }
                pre { class: "log-output mt-1 max-h-64 min-h-0",
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
                        "diagnosis" => (t!("healer-phase-d"), t!("healer-phase-diagnosis"), "border-warn".to_string()),
                        "remediation" => (t!("healer-phase-r"), t!("healer-phase-remediation"), "border-warn".to_string()),
                        "final_report" => (t!("healer-phase-f"), t!("healer-phase-final-report"), "border-success".to_string()),
                        _ => ("P".to_string(), pin.slot.clone(), "border-line".to_string()),
                    };
                    let html = simple_md_to_html(&pin.summary);
                    let services = pin.affected_services.clone();
                    rsx! {
                        div { class: "p-3 bg-surface rounded border-l-4 {border} shadow-sm",
                            div { class: "flex items-center gap-1.5 mb-1",
                                span { class: "w-5 h-5 flex items-center justify-center rounded-full bg-surface-2 text-xs font-bold text-fg", "{icon}" }
                                span { class: "text-xs font-semibold text-fg-muted uppercase tracking-wider", "{label}" }
                            }
                            div {
                                class: "text-sm text-fg-strong prose prose-sm dark:prose-invert max-w-none",
                                dangerous_inner_html: "{html}",
                            }
                            if !services.is_empty() {
                                div { class: "mt-2 flex flex-wrap gap-1",
                                    for svc in services.iter() {
                                        Badge { variant: BadgeVariant::Neutral, "{svc}" }
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
        div { class: "alert alert-warn mb-4",
            h4 { class: "text-sm font-semibold text-warn-strong mb-2", {t!("nav-staff-pings")} }
            div { class: "space-y-2",
                for ping in pings.iter() {
                    {
                        let cat_variant = category_badge_variant(&ping.category);
                        rsx! {
                            div { class: "flex items-start gap-2 text-sm",
                                div { class: "flex-1",
                                    Badge { variant: cat_variant, class: "mr-2", "{ping.category}" }
                                    if ping.resolved {
                                        span { class: "text-success line-through", "{ping.message}" }
                                    } else {
                                        span { class: "text-fg-strong", "{ping.message}" }
                                    }
                                    span { class: "text-xs text-fg-faint ml-2", "{ping.created_at}" }
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
///
/// The result is rendered via `dangerous_inner_html`, so we strip raw HTML
/// events from the parser stream (otherwise `<script>` and friends embedded
/// in the markdown would survive) and rewrite link/image URLs whose scheme
/// isn't on the allowlist (`http`, `https`, `mailto`) to `#`, blocking
/// `javascript:` / `data:` / `vbscript:` payloads.
pub fn simple_md_to_html(md: &str) -> String {
    use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let events = Parser::new_ext(md, options).filter_map(|event| match event {
        Event::Html(_) | Event::InlineHtml(_) => None,
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Some(Event::Start(Tag::Link {
            link_type,
            dest_url: if is_safe_uri(&dest_url) {
                dest_url
            } else {
                CowStr::Borrowed("#")
            },
            title,
            id,
        })),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => Some(Event::Start(Tag::Image {
            link_type,
            dest_url: if is_safe_uri(&dest_url) {
                dest_url
            } else {
                CowStr::Borrowed("#")
            },
            title,
            id,
        })),
        e => Some(e),
    });
    let mut output = String::with_capacity(md.len() * 2);
    html::push_html(&mut output, events);
    output
}

fn is_safe_uri(uri: &str) -> bool {
    let trimmed = uri.trim_start();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.starts_with('/') || trimmed.starts_with('#') || trimmed.starts_with('?') {
        return true;
    }
    match trimmed.find(':') {
        Some(end) => {
            let scheme = trimmed[..end].to_ascii_lowercase();
            matches!(scheme.as_str(), "http" | "https" | "mailto")
        }
        None => true,
    }
}
