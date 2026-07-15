//! Interactive fleet chatbot UI: session list + live chat with streaming
//! events, risk-classed tool calls, and inline approval prompts.

use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
use crate::web::components::chat_ui::{
    ChatMsg, ModelEntry, PinInfo, RunningToolInfo, reason_display, render_message,
    simple_md_to_html, state_badge,
};
use crate::web::components::ui::{Badge, BadgeVariant, ErrorText};

#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from};

// ── Wire DTOs ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatContext {
    pub enabled: bool,
    pub models: Vec<ModelEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatSessionSummary {
    pub id: String,
    pub label: Option<String>,
    pub state: String,
    pub model: Option<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub tokens_used: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatSessionMeta {
    pub label: Option<String>,
    pub state: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tokens_used: u64,
    pub token_budget: u64,
    /// Standing "approve everything" grant for the running agent.
    pub auto_approve: bool,
}

/// A pending tool-call approval shown inline in the chat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalInfo {
    pub approval_id: String,
    pub tool_name: String,
    pub tool_args: String,
    pub reason: String,
    pub risk: String,
    pub guard_reasoning: Option<String>,
}

// ── Server functions ────────────────────────────────────────────────────

#[cfg(feature = "server")]
fn chat_state() -> Result<crate::chat::ChatState, ServerFnError> {
    crate::server_state::chat_state().ok_or_else(|| ServerFnError::new("chat is not enabled"))
}

#[cfg(feature = "server")]
async fn chat_user() -> Result<crate::chat::ChatUserCtx, ServerFnError> {
    let user = current_user().await?;
    Ok(crate::chat::ChatUserCtx {
        email: user.email.clone(),
        is_admin: user.is_admin,
        principal: principal_from(&user),
    })
}

#[server]
pub async fn get_chat_context() -> Result<ChatContext, ServerFnError> {
    let _user = current_user().await?;
    let cfg = crate::config::config();
    let enabled = cfg.chat.enabled && crate::server_state::chat_state().is_some();
    let entries = if cfg.chat.models.is_empty() {
        if cfg.healer.models.is_empty() {
            crate::config::default_healer_models()
        } else {
            cfg.healer.models.clone()
        }
    } else {
        cfg.chat.models.clone()
    };
    Ok(ChatContext {
        enabled,
        models: entries
            .iter()
            .map(|m| ModelEntry {
                name: m.name.clone(),
                model: m.model.clone(),
                provider: m.provider.clone(),
            })
            .collect(),
    })
}

#[server]
pub async fn list_chat_sessions() -> Result<Vec<ChatSessionSummary>, ServerFnError> {
    let user = chat_user().await?;
    let chat = chat_state()?;
    let sessions = chat
        .list_sessions(&user.email)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let mut out = Vec::with_capacity(sessions.len());
    for s in sessions {
        let tokens = chat.store().get_token_usage(s.id).await.unwrap_or(0);
        out.push(ChatSessionSummary {
            id: s.id.to_string(),
            label: s.label,
            state: s.state,
            model: s.model,
            updated_at: s.updated_at,
            tokens_used: tokens,
        });
    }
    Ok(out)
}

#[server]
pub async fn start_chat_session(
    provider: Option<String>,
    model: Option<String>,
    message: String,
    page_context: Option<String>,
) -> Result<String, ServerFnError> {
    let user = chat_user().await?;
    let chat = chat_state()?;
    if message.trim().is_empty() {
        return Err(ServerFnError::new("message is empty"));
    }
    let id = chat
        .start_session(&user, provider, model, message, page_context)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(id.to_string())
}

#[server]
pub async fn send_chat_message(
    session_id: String,
    message: String,
    page_context: Option<String>,
) -> Result<(), ServerFnError> {
    let user = chat_user().await?;
    let chat = chat_state()?;
    let id: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    if message.trim().is_empty() {
        return Err(ServerFnError::new("message is empty"));
    }
    chat.send_message(&user, id, message, page_context)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// decision: "approve" | "approve_all" | "deny"
#[server]
pub async fn chat_approve(
    session_id: String,
    approval_id: String,
    decision: String,
    reason: Option<String>,
) -> Result<(), ServerFnError> {
    let user = chat_user().await?;
    let chat = chat_state()?;
    let sid: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid session id"))?;
    let aid: uuid::Uuid = approval_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid approval id"))?;
    let decision = match decision.as_str() {
        "approve" => plan_ai_chat::ApprovalDecision::Approve,
        "approve_all" => plan_ai_chat::ApprovalDecision::ApproveAllForSession,
        _ => plan_ai_chat::ApprovalDecision::Deny { reason },
    };
    chat.resolve_approval(&user, sid, aid, decision)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn pause_chat_session(session_id: String) -> Result<(), ServerFnError> {
    let user = chat_user().await?;
    let chat = chat_state()?;
    let id: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    chat.pause(&user, id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn cancel_chat_session(session_id: String) -> Result<(), ServerFnError> {
    let user = chat_user().await?;
    let chat = chat_state()?;
    let id: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    chat.cancel(&user, id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn extend_chat_budget(session_id: String) -> Result<(), ServerFnError> {
    let user = chat_user().await?;
    let chat = chat_state()?;
    let id: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    chat.extend_budget(&user, id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Toggle the session-level "auto-approve everything" grant.
#[server]
pub async fn set_chat_auto_approve(session_id: String, value: bool) -> Result<(), ServerFnError> {
    let user = chat_user().await?;
    let chat = chat_state()?;
    let id: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    chat.set_auto_approve(&user, id, value)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn get_chat_session_meta(session_id: String) -> Result<ChatSessionMeta, ServerFnError> {
    let user = chat_user().await?;
    let chat = chat_state()?;
    let id: uuid::Uuid = session_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid id"))?;
    let sess = chat
        .get_session_checked(&user, id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let tokens_used = chat.store().get_token_usage(id).await.unwrap_or(0);
    let token_budget = chat.store().get_token_budget(id).await.unwrap_or(0);
    Ok(ChatSessionMeta {
        label: sess.label,
        state: sess.state,
        provider: sess.provider,
        model: sess.model,
        tokens_used,
        token_budget,
        auto_approve: chat.auto_approve(id),
    })
}

/// User turns are persisted with the page-context prefix the agent sees
/// ("[context: user is viewing /x]\n\n..."); hide it in the UI.
fn strip_context_prefix(content: &str) -> &str {
    if let Some(rest) = content.strip_prefix("[context: ") {
        if let Some(idx) = rest.find("]\n\n") {
            return &rest[idx + 3..];
        }
    }
    content
}

// ── Components ──────────────────────────────────────────────────────────
//
// The chat lives in a context-aware sidebar available on every page: a
// floating brand bubble (bottom-right) opens it; the current route is sent
// along with every message so the agent knows what the user is looking at.

/// Floating chat bubble + slide-in sidebar. Mounted once in the Layout.
#[component]
pub fn ChatSidebar() -> Element {
    // use_resource (not use_server_future): the sidebar lives OUTSIDE the
    // router's SuspenseBoundary, so suspending here would bubble to the app
    // root and replace the whole page with the loading state.
    let ctx = use_resource(get_chat_context);
    let mut open = use_signal(|| false);
    // None = session list; Some(id) = active conversation.
    let mut active: Signal<Option<String>> = use_signal(|| None);

    let enabled = matches!(&*ctx.read(), Some(Ok(c)) if c.enabled);

    // Width drag: DOM-driven for smoothness (no per-move round trip through
    // the VDOM); width persists in localStorage. Re-runs on every open so a
    // remounted panel gets rewired.
    use_effect(move || {
        if !*open.read() {
            return;
        }
        document::eval(
            r#"
            (function() {
                const sb = document.getElementById('chat-sidebar');
                const handle = document.getElementById('chat-resize');
                if (!sb || !handle || handle.dataset.wired) return;
                handle.dataset.wired = '1';
                const clamp = (w) => Math.min(Math.max(w, 320), window.innerWidth - 80);
                const saved = parseInt(localStorage.getItem('chat.sidebar.width') || '');
                if (saved) sb.style.width = clamp(saved) + 'px';
                let dragging = false;
                handle.addEventListener('pointerdown', (e) => {
                    dragging = true;
                    handle.setPointerCapture(e.pointerId);
                    document.body.style.userSelect = 'none';
                    e.preventDefault();
                });
                handle.addEventListener('pointermove', (e) => {
                    if (!dragging) return;
                    sb.style.width = clamp(window.innerWidth - e.clientX) + 'px';
                });
                const end = () => {
                    if (!dragging) return;
                    dragging = false;
                    document.body.style.userSelect = '';
                    try {
                        localStorage.setItem('chat.sidebar.width', parseInt(sb.style.width) || '');
                    } catch (_) {}
                };
                handle.addEventListener('pointerup', end);
                handle.addEventListener('pointercancel', end);
            })();
            "#,
        );
    });

    if !enabled {
        return rsx! {};
    }
    let models = match &*ctx.read() {
        Some(Ok(c)) => c.models.clone(),
        _ => Vec::new(),
    };

    rsx! {
        // Floating open button
        if !*open.read() {
            button {
                r#type: "button",
                class: "fixed bottom-5 right-5 z-40 w-12 h-12 rounded-full bg-brand text-white shadow-lg flex items-center justify-center hover:opacity-90 focus:outline-none focus:ring-2 focus:ring-brand transition-opacity",
                "aria-label": t!("chat-open"),
                onclick: move |_| open.set(true),
                ChatBubbleIcon {}
            }
        }

        // Sidebar panel
        if *open.read() {
            div {
                id: "chat-sidebar",
                class: "fixed inset-y-0 right-0 z-50 w-[26rem] max-w-full flex flex-col bg-surface border-l border-line shadow-2xl",
                // Drag handle over the left border: resizes the sidebar.
                div {
                    id: "chat-resize",
                    class: "absolute left-0 inset-y-0 w-1.5 -ml-0.5 cursor-ew-resize hover:bg-brand/40 z-10",
                    "aria-hidden": "true",
                }
                // Header: [logo] chat
                div { class: "shrink-0 flex items-center gap-2 px-3 py-2 border-b border-line",
                    crate::web::components::navbar::LogoMark {}
                    span { class: "font-semibold text-sm tracking-tight", {t!("chat-title")} }
                    div { class: "flex-1" }
                    if active.read().is_some() {
                        button {
                            class: "btn btn-xs btn-ghost",
                            title: t!("chat-back-to-list").to_string(),
                            onclick: move |_| active.set(None),
                            "←"
                        }
                    }
                    if let Some(sid) = active.read().clone() {
                        ChatMenu { key: "{sid}", session_id: sid }
                    }
                    button {
                        class: "btn btn-xs btn-ghost",
                        "aria-label": t!("chat-close"),
                        onclick: move |_| open.set(false),
                        "✕"
                    }
                }

                {match active.read().clone() {
                    Some(sid) => rsx! {
                        ChatConversation { key: "{sid}", session_id: sid, active }
                    },
                    None => rsx! {
                        ChatSessionList { models: models.clone(), active }
                    },
                }}
            }
        }
    }
}

/// Three-dot menu for the active session: auto-approve toggle + session actions.
#[component]
fn ChatMenu(session_id: String) -> Element {
    let mut menu_open = use_signal(|| false);
    let mut auto_approve = use_signal(|| false);

    // Load the current auto-approve state once per session.
    let sid_load = session_id.clone();
    use_future(move || {
        let sid = sid_load.clone();
        async move {
            if let Ok(meta) = get_chat_session_meta(sid).await {
                auto_approve.set(meta.auto_approve);
            }
        }
    });

    let sid_toggle = session_id.clone();
    let sid_pause = session_id.clone();
    let sid_cancel = session_id.clone();

    rsx! {
        div { class: "relative",
            button {
                class: "btn btn-xs btn-ghost",
                "aria-label": t!("chat-menu"),
                onclick: move |_| { let v = *menu_open.read(); menu_open.set(!v); },
                "⋮"
            }
            if *menu_open.read() {
                div { class: "absolute right-0 top-7 z-10 card p-1 shadow-lg min-w-44 flex flex-col",
                    button {
                        class: "flex items-center gap-2 text-sm px-2 py-1.5 rounded hover:bg-surface-3 text-left",
                        onclick: move |_| {
                            let next = !*auto_approve.read();
                            auto_approve.set(next);
                            let sid = sid_toggle.clone();
                            spawn(async move {
                                if set_chat_auto_approve(sid, next).await.is_err() {
                                    auto_approve.set(!next);
                                }
                            });
                        },
                        input {
                            r#type: "checkbox",
                            checked: *auto_approve.read(),
                            class: "pointer-events-none",
                        }
                        {t!("chat-auto-approve")}
                    }
                    div { class: "border-t border-line my-1" }
                    button {
                        class: "text-sm px-2 py-1.5 rounded hover:bg-surface-3 text-left",
                        onclick: move |_| {
                            menu_open.set(false);
                            let sid = sid_pause.clone();
                            spawn(async move { let _ = pause_chat_session(sid).await; });
                        },
                        {t!("chat-pause")}
                    }
                    button {
                        class: "text-sm px-2 py-1.5 rounded hover:bg-surface-3 text-left text-danger",
                        onclick: move |_| {
                            menu_open.set(false);
                            let sid = sid_cancel.clone();
                            spawn(async move { let _ = cancel_chat_session(sid).await; });
                        },
                        {t!("chat-cancel")}
                    }
                }
            }
        }
    }
}

/// Up-arrow glyph for the in-box send button.
#[component]
fn SendArrowIcon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 24 24",
            fill: "none",
            path {
                d: "M12 19V5M5 12l7-7 7 7",
                stroke: "currentColor",
                "stroke-width": "2.2",
                "stroke-linecap": "round",
                "stroke-linejoin": "round",
            }
        }
    }
}

/// Three bouncing dots shown while the agent is processing a turn.
#[component]
fn TypingIndicator() -> Element {
    rsx! {
        div { class: "flex items-center gap-1 mt-2 px-3 py-2 rounded-xl bg-surface-2 w-fit",
            span { class: "w-1.5 h-1.5 rounded-full bg-fg-muted animate-bounce" }
            span { class: "w-1.5 h-1.5 rounded-full bg-fg-muted animate-bounce [animation-delay:150ms]" }
            span { class: "w-1.5 h-1.5 rounded-full bg-fg-muted animate-bounce [animation-delay:300ms]" }
        }
    }
}

#[component]
fn ChatBubbleIcon() -> Element {
    rsx! {
        svg {
            width: "22",
            height: "22",
            view_box: "0 0 24 24",
            fill: "none",
            path {
                d: "M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z",
                stroke: "currentColor",
                "stroke-width": "2",
                "stroke-linecap": "round",
                "stroke-linejoin": "round",
            }
        }
    }
}

/// Session list + new-chat composer (sidebar start view).
#[component]
fn ChatSessionList(models: Vec<ModelEntry>, active: Signal<Option<String>>) -> Element {
    // Non-suspending: see ChatSidebar.
    let mut sessions = use_resource(list_chat_sessions);
    let mut selected_model = use_signal(String::new);
    let mut first_message = use_signal(String::new);
    let mut error = use_signal::<Option<String>>(|| None);
    let mut starting = use_signal(|| false);

    let route = use_route::<Route>();
    let page_ctx = route.to_string();

    let models_for_start = models.clone();
    let start = move |_| {
        let msg = first_message.read().clone();
        if msg.trim().is_empty() || *starting.read() {
            return;
        }
        let sel = selected_model.read().clone();
        let entry = models_for_start
            .iter()
            .find(|m| m.name == sel)
            .or(models_for_start.first());
        let (provider, model) = match entry {
            Some(m) => (Some(m.provider.clone()), Some(m.model.clone())),
            None => (None, None),
        };
        let page_ctx = page_ctx.clone();
        starting.set(true);
        spawn(async move {
            match start_chat_session(provider, model, msg, Some(page_ctx)).await {
                Ok(id) => {
                    first_message.set(String::new());
                    starting.set(false);
                    active.set(Some(id));
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                    starting.set(false);
                }
            }
        });
    };

    rsx! {
        div { class: "flex-1 overflow-y-auto p-3 flex flex-col gap-2",
            {match &*sessions.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-fg-muted text-sm", {t!("chat-no-sessions")} }
                },
                Some(Ok(list)) => rsx! {
                    for s in list.iter() {
                        {
                            let sid = s.id.clone();
                            let (variant, label) = state_badge(&s.state);
                            let title = s.label.clone().unwrap_or_else(|| t!("chat-untitled"));
                            let updated = s.updated_at.format("%m-%d %H:%M").to_string();
                            rsx! {
                                button {
                                    key: "{s.id}",
                                    class: "card p-2 flex items-center justify-between gap-2 text-left hover:border-brand w-full",
                                    onclick: move |_| active.set(Some(sid.clone())),
                                    div { class: "min-w-0",
                                        div { class: "font-medium text-sm truncate", "{title}" }
                                        div { class: "text-xs text-fg-muted", "{updated} · {s.tokens_used} tok" }
                                    }
                                    Badge { variant, "{label}" }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                None => rsx! { p { class: "text-fg-muted text-sm", {t!("loading")} } },
            }}
        }

        // New chat composer (pinned at the bottom of the list view)
        div { class: "shrink-0 border-t border-line p-3 flex flex-col gap-2",
            if let Some(err) = error.read().as_ref() {
                ErrorText { {t!("error-message", message: err.clone())} }
            }
            div { class: "relative",
                textarea {
                    class: "input w-full min-h-16 text-sm rounded-xl pr-12",
                    placeholder: t!("chat-first-message-placeholder").to_string(),
                    value: "{first_message}",
                    oninput: move |e| first_message.set(e.value()),
                }
                button {
                    r#type: "button",
                    class: "absolute right-2 bottom-2 w-8 h-8 rounded-full bg-surface-3 text-fg-muted hover:bg-surface-2 hover:text-fg-strong flex items-center justify-center transition-colors disabled:opacity-40",
                    "aria-label": t!("chat-start"),
                    disabled: *starting.read(),
                    onclick: start,
                    SendArrowIcon {}
                }
            }
            div { class: "flex items-center gap-2",
                select {
                    class: "input input-sm flex-1",
                    onchange: move |e| selected_model.set(e.value()),
                    for m in models.iter() {
                        option { value: "{m.name}", "{m.name}" }
                    }
                }
                button {
                    class: "btn btn-sm btn-ghost",
                    title: t!("chat-refresh").to_string(),
                    onclick: move |_| { sessions.restart(); },
                    "⟳"
                }
            }
        }
    }
}

/// One live conversation: SSE stream, messages (healer-style rendering),
/// running tools, inline approvals, pins, composer.
#[component]
fn ChatConversation(session_id: String, active: Signal<Option<String>>) -> Element {
    let mut messages = use_signal::<Vec<ChatMsg>>(Vec::new);
    let mut active_tools = use_signal::<Vec<RunningToolInfo>>(Vec::new);
    let mut pins = use_signal::<Vec<PinInfo>>(Vec::new);
    let mut approvals = use_signal::<Vec<ApprovalInfo>>(Vec::new);
    let mut state = use_signal(|| "running".to_string());
    let mut state_reason = use_signal::<Option<String>>(|| None);
    // Agent is processing (between a user turn and the next idle/approval).
    // Set by send() and by live running-tool events, so replaying an old
    // session never shows the indicator.
    let mut busy = use_signal(|| false);
    // Pinned panel is expanded by default; the header button collapses it.
    let mut show_pins = use_signal(|| true);
    // Optimistic local echoes of sent messages, pending their server copy
    // (the loop persists user turns at pickup, which can lag the submit).
    let mut pending_echoes = use_signal::<Vec<String>>(Vec::new);
    let mut input = use_signal(String::new);
    let mut send_error = use_signal::<Option<String>>(|| None);
    let mut meta_refresh = use_signal(|| 0u32);

    let route = use_route::<Route>();
    let page_ctx = route.to_string();

    let sid_meta = session_id.clone();
    let meta = use_resource(move || {
        let sid = sid_meta.clone();
        let _tick = meta_refresh();
        async move { get_chat_session_meta(sid).await }
    });

    // SSE consumer. Restartable: the stream ends (done/_closed) for parked
    // sessions; sending a message resumes the session server-side and bumps
    // the epoch so we reconnect and replay the fresh state.
    let mut stream_epoch = use_signal(|| 0u32);
    let mut stream_ended = use_signal(|| false);
    let sid_for_sse = session_id.clone();
    let _sse = use_resource(move || {
        let sid = sid_for_sse.clone();
        let _epoch = stream_epoch();
        async move {
            // Reconnects replay the whole session — start from a clean slate.
            messages.set(Vec::new());
            approvals.set(Vec::new());
            active_tools.set(Vec::new());
            pending_echoes.set(Vec::new());
            stream_ended.set(false);
            let mut ev = document::eval(&format!(
                r#"
                const es = new EventSource("/_sse/chat/{sid}");
                es.onmessage = (e) => dioxus.send(e.data);
                es.onerror = () => {{
                    if (es.readyState === EventSource.CLOSED) {{
                        dioxus.send('{{"kind":"_closed"}}');
                    }}
                }};
                await new Promise(() => {{}});
                "#
            ));

            while let Ok(val) = ev.recv::<serde_json::Value>().await {
                let val_str = val.as_str().unwrap_or_default();
                let Ok(evt) =
                    serde_json::from_str::<mac_mgmt_common::ChatStreamEvent>(val_str)
                else {
                    continue;
                };
                match evt.kind.as_str() {
                    "message" => {
                        if let (Some(role), Some(content)) = (evt.role, evt.content) {
                            // Pins belong in the pinned panel, not the
                            // transcript (the content is a JSON envelope).
                            if role == "pin" {
                                if let Ok(data) =
                                    serde_json::from_str::<serde_json::Value>(&content)
                                {
                                    if let Some(slot) =
                                        data.get("slot").and_then(|v| v.as_str())
                                    {
                                        let pin = PinInfo {
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
                                                        .filter_map(|v| {
                                                            v.as_str().map(String::from)
                                                        })
                                                        .collect()
                                                })
                                                .unwrap_or_default(),
                                        };
                                        let mut cur = pins.write();
                                        cur.retain(|p| p.slot != pin.slot);
                                        cur.push(pin);
                                    }
                                }
                                continue;
                            }
                            if !content.is_empty()
                                && role != "approval_request"
                                && role != "approval_decision"
                            {
                                let content = if role == "user" {
                                    strip_context_prefix(&content).to_string()
                                } else {
                                    content
                                };
                                // Server copy of an optimistically echoed send?
                                if role == "user" {
                                    let mut echoes = pending_echoes.write();
                                    if echoes.first().is_some_and(|e| *e == content) {
                                        echoes.remove(0);
                                        continue;
                                    }
                                }
                                messages.push(ChatMsg {
                                    role,
                                    content,
                                    metadata: evt.metadata,
                                });
                            }
                        }
                    }
                    "running_tools" => {
                        let tools = evt.running_tools.unwrap_or_default();
                        if !tools.is_empty() {
                            busy.set(true);
                        }
                        active_tools.set(tools);
                    }
                    "pins" => {
                        pins.set(evt.pins.unwrap_or_default());
                    }
                    "state" => {
                        if let Some(s) = evt.state {
                            state.set(s);
                        }
                        state_reason.set(evt.state_reason);
                    }
                    "approval_request" => {
                        busy.set(false);
                        if let Some(m) = evt.metadata {
                            let info = ApprovalInfo {
                                approval_id: m
                                    .get("approval_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                tool_name: m
                                    .get("tool_name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                tool_args: m
                                    .get("tool_args")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                reason: m
                                    .get("reason")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                risk: m
                                    .get("risk")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                guard_reasoning: m
                                    .get("guard_reasoning")
                                    .and_then(|v| v.as_str())
                                    .map(String::from),
                            };
                            let mut cur = approvals.write();
                            cur.retain(|a| a.approval_id != info.approval_id);
                            cur.push(info);
                        }
                    }
                    "approval_resolved" => {
                        busy.set(true);
                        if let Some(id) = evt
                            .metadata
                            .as_ref()
                            .and_then(|m| m.get("approval_id"))
                            .and_then(|v| v.as_str())
                        {
                            approvals.write().retain(|a| a.approval_id != id);
                        }
                    }
                    "idle" => {
                        busy.set(false);
                        active_tools.set(Vec::new());
                        meta_refresh += 1;
                    }
                    "done" => {
                        busy.set(false);
                        active_tools.set(Vec::new());
                        approvals.set(Vec::new());
                        if let Some(s) = evt.state {
                            state.set(s);
                        }
                        state_reason.set(evt.state_reason);
                        meta_refresh += 1;
                        break;
                    }
                    "_closed" | "error" => break,
                    _ => {}
                }
            }
            stream_ended.set(true);
        }
    });

    // Auto-scroll the sidebar message pane
    use_effect(move || {
        document::eval(
            r#"
            (function() {
                const el = document.getElementById('chat-messages');
                if (!el) return;
                const pane = document.getElementById('chat-scroll');
                const observer = new MutationObserver(() => {
                    if (!pane) return;
                    const dist = pane.scrollHeight - pane.scrollTop - pane.clientHeight;
                    if (dist < 300) {
                        pane.scrollTo({ top: pane.scrollHeight, behavior: 'smooth' });
                    }
                });
                observer.observe(el, { childList: true, subtree: true });
            })();
            "#,
        );
    });

    let sid_send = session_id.clone();
    let page_ctx_send = page_ctx.clone();
    let mut send = move |_| {
        let text = input.read().trim().to_string();
        if text.is_empty() {
            return;
        }
        let sid = sid_send.clone();
        let ctx = page_ctx_send.clone();
        input.set(String::new());
        send_error.set(None);
        busy.set(true);
        // Optimistic echo — the server copy arrives when the loop picks the
        // turn up and is de-duplicated against this entry.
        messages.push(ChatMsg {
            role: "user".to_string(),
            content: text.clone(),
            metadata: None,
        });
        pending_echoes.push(text.clone());
        spawn(async move {
            if let Err(e) = send_chat_message(sid, text.clone(), Some(ctx)).await {
                send_error.set(Some(e.to_string()));
                busy.set(false);
                // Roll back the echo.
                pending_echoes.write().retain(|m| m != &text);
                let mut msgs = messages.write();
                if let Some(pos) = msgs
                    .iter()
                    .rposition(|m| m.role == "user" && m.content == text)
                {
                    msgs.remove(pos);
                }
            } else if *stream_ended.peek() {
                // The session was parked — it just respawned; reconnect.
                stream_epoch += 1;
            }
        });
    };

    let meta_val = meta
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let (badge_variant, badge_label) = state_badge(&state.read());
    let cur_state = state.read().clone();
    let is_terminal = matches!(cur_state.as_str(), "failed" | "cancelled");
    let budget_exhausted = state_reason
        .read()
        .as_deref()
        .map(|r| r == "token_budget_exceeded")
        .unwrap_or(false);
    let has_pins = !pins.read().is_empty();

    let sid_extend = session_id.clone();

    rsx! {
        // Session header strip
        div { class: "shrink-0 flex items-center gap-2 px-3 py-1.5 border-b border-line text-xs text-fg-muted flex-wrap",
            Badge { variant: badge_variant, "{badge_label}" }
            if let Some(reason) = state_reason.read().as_ref() {
                span { {reason_display(reason)} }
            }
            if let Some(m) = &meta_val {
                span { class: "font-mono", {m.model.clone().unwrap_or_default()} }
                span {
                    "{m.tokens_used}"
                    if m.token_budget > 0 { "/{m.token_budget}" }
                    " tok"
                }
            }
            div { class: "flex-1" }
            if budget_exhausted {
                button {
                    class: "btn btn-xs btn-primary",
                    onclick: move |_| {
                        let sid = sid_extend.clone();
                        spawn(async move { let _ = extend_chat_budget(sid).await; });
                    },
                    {t!("chat-extend-budget")}
                }
            }
            if has_pins {
                button {
                    class: "btn btn-xs btn-ghost",
                    onclick: move |_| { let v = *show_pins.read(); show_pins.set(!v); },
                    {t!("chat-pinned")}
                }
            }
        }

        // Pins (collapsible)
        if *show_pins.read() && has_pins {
            div { class: "shrink-0 border-b border-line p-2 max-h-48 overflow-y-auto flex flex-col gap-2",
                for pin in pins.read().iter() {
                    div { class: "card p-2",
                        div { class: "text-[10px] uppercase text-fg-muted", "{pin.slot}" }
                        div {
                            class: "text-xs",
                            dangerous_inner_html: simple_md_to_html(&pin.summary),
                        }
                    }
                }
            }
        }

        // Messages
        div { id: "chat-scroll", class: "flex-1 overflow-y-auto p-3",
            div { id: "chat-messages", class: "flex flex-col gap-2",
                for (i, msg) in messages.read().iter().enumerate() {
                    div { key: "{i}", {render_message(msg)} }
                }
            }

            if *busy.read() && approvals.read().is_empty() {
                TypingIndicator {}
            }

            if !active_tools.read().is_empty() {
                div { class: "flex flex-wrap gap-1 mt-2",
                    for tool in active_tools.read().iter() {
                        span { class: "badge badge-info animate-pulse text-[10px] font-mono",
                            "{tool.name}"
                            if let Some(v) = &tool.validation {
                                " · {v.status}"
                            }
                        }
                    }
                }
            }

            for approval in approvals.read().iter() {
                ApprovalCard { session_id: session_id.clone(), approval: approval.clone() }
            }
        }

        // Composer
        div { class: "shrink-0 border-t border-line p-2 flex flex-col gap-1",
            if let Some(err) = send_error.read().as_ref() {
                ErrorText { {t!("error-message", message: err.clone())} }
            }
            div { class: "relative",
                textarea {
                    class: "input w-full min-h-14 text-sm rounded-xl pr-12",
                    placeholder: if is_terminal {
                        t!("chat-session-over").to_string()
                    } else if *busy.read() {
                        t!("chat-input-queued-placeholder").to_string()
                    } else {
                        t!("chat-input-placeholder").to_string()
                    },
                    disabled: is_terminal,
                    value: "{input}",
                    oninput: move |e| input.set(e.value()),
                    onkeydown: {
                        let mut send = send.clone();
                        move |e: KeyboardEvent| {
                            if e.key() == Key::Enter && !e.modifiers().shift() {
                                e.prevent_default();
                                send(());
                            }
                        }
                    },
                }
                button {
                    r#type: "button",
                    class: "absolute right-2 bottom-2 w-8 h-8 rounded-full bg-surface-3 text-fg-muted hover:bg-surface-2 hover:text-fg-strong flex items-center justify-center transition-colors disabled:opacity-40",
                    "aria-label": t!("chat-send"),
                    disabled: is_terminal,
                    onclick: move |_| send(()),
                    SendArrowIcon {}
                }
            }
        }
    }
}

#[component]
fn ApprovalCard(session_id: String, approval: ApprovalInfo) -> Element {
    let mut deny_reason = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal::<Option<String>>(|| None);

    let risk_variant = match approval.risk.as_str() {
        "destructive" => BadgeVariant::Danger,
        "mutating" => BadgeVariant::Warn,
        _ => BadgeVariant::Info,
    };

    let pretty_args = serde_json::from_str::<serde_json::Value>(&approval.tool_args)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| approval.tool_args.clone());

    let decide = {
        let session_id = session_id.clone();
        let approval_id = approval.approval_id.clone();
        move |decision: &'static str, reason: Option<String>| {
            let sid = session_id.clone();
            let aid = approval_id.clone();
            busy.set(true);
            spawn(async move {
                if let Err(e) = chat_approve(sid, aid, decision.to_string(), reason).await {
                    error.set(Some(e.to_string()));
                    busy.set(false);
                }
            });
        }
    };

    let mut approve = {
        let mut decide = decide.clone();
        move |_| decide("approve", None)
    };
    let mut approve_all = {
        let mut decide = decide.clone();
        move |_| decide("approve_all", None)
    };
    let mut deny = {
        let mut decide = decide.clone();
        move |_| {
            let r = deny_reason.read().trim().to_string();
            decide("deny", if r.is_empty() { None } else { Some(r) })
        }
    };

    rsx! {
        div { class: "card p-3 mt-2 border-2 border-warn",
            div { class: "flex items-center gap-2 mb-1 flex-wrap",
                Badge { variant: risk_variant, "{approval.risk}" }
                span { class: "font-mono font-medium text-sm", "{approval.tool_name}" }
                span { class: "text-xs text-fg-muted", {t!("chat-approval-needed")} }
            }
            if !approval.reason.is_empty() {
                p { class: "text-xs mb-1", em { "{approval.reason}" } }
            }
            pre { class: "bg-surface-2 rounded p-2 text-[10px] overflow-x-auto mb-1 max-h-32", "{pretty_args}" }
            if let Some(guard) = &approval.guard_reasoning {
                p { class: "text-[10px] text-fg-muted mb-1", {t!("chat-guard-verdict")} ": {guard}" }
            }
            if let Some(err) = error.read().as_ref() {
                ErrorText { {t!("error-message", message: err.clone())} }
            }
            div { class: "flex flex-wrap items-center gap-1",
                button {
                    class: "btn btn-xs btn-primary",
                    disabled: *busy.read(),
                    onclick: move |e| approve(e),
                    {t!("chat-approve-once")}
                }
                button {
                    class: "btn btn-xs btn-ghost",
                    disabled: *busy.read(),
                    onclick: move |e| approve_all(e),
                    {t!("chat-approve-all")}
                }
                input {
                    class: "input input-sm flex-1 min-w-24 text-xs",
                    placeholder: t!("chat-deny-reason-placeholder").to_string(),
                    value: "{deny_reason}",
                    oninput: move |e| deny_reason.set(e.value()),
                }
                button {
                    class: "btn btn-xs btn-ghost text-danger",
                    disabled: *busy.read(),
                    onclick: move |e| deny(e),
                    {t!("chat-deny")}
                }
            }
        }
    }
}
