//! Axum SSE endpoint for chat session streaming.
//!
//! Same shape as `healer_sse`: replay persisted messages, snapshots (pins,
//! state, running tools, pending approvals), then live broadcast with lag
//! recovery from the store. The wire format is `ChatStreamEvent`, shared
//! with the healer streams.

use dioxus::fullstack::axum::{
    self,
    extract::{Extension, Path},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use std::convert::Infallible;
use uuid::Uuid;

use super::user::WebUser;
use crate::server_state;
use mac_mgmt_common::ChatStreamEvent;

fn event_json(evt: &ChatStreamEvent) -> Event {
    Event::default().data(serde_json::to_string(evt).unwrap_or_default())
}

fn approval_event(p: &plan_ai_chat::PendingApproval) -> ChatStreamEvent {
    ChatStreamEvent {
        kind: "approval_request".to_string(),
        metadata: Some(serde_json::json!({
            "approval_id": p.id,
            "tool_name": p.tool_name,
            "tool_args": p.args,
            "reason": p.reason,
            "risk": plan_ai_chat::validation::risk_to_str(p.risk),
            "guard_reasoning": p.guard_reasoning,
            "requested_at": p.requested_at,
        })),
        ..ChatStreamEvent::default()
    }
}

/// GET /_sse/chat/:session_id — SSE stream for a chat session.
pub async fn view_session_sse(
    user: Option<Extension<WebUser>>,
    Path(session_id): Path<String>,
) -> Response {
    let Some(Extension(user)) = user else {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(chat) = server_state::chat_state() else {
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Ok(uuid) = session_id.parse::<Uuid>() else {
        return axum::http::StatusCode::BAD_REQUEST.into_response();
    };

    let Ok(Some(session)) = chat.store().get_session(uuid).await else {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    };
    if !user.is_admin && session.subject != user.email {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    let Ok(existing_messages) = chat.store().get_messages(uuid).await else {
        return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let manager = chat.manager().clone();
    let store = chat.store().clone();
    let is_running = manager.is_running(uuid);
    let current_state = session.state.clone();
    let current_reason = session
        .state_data
        .get("reason")
        .and_then(|v| v.as_str())
        .map(String::from);

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        use super::components::chat_ui::{
            chat_event_to_stream, extract_pins_from_messages, running_tools_to_wire,
        };

        let empty = ChatStreamEvent::default;

        let mut last_seen_at = chrono::DateTime::<chrono::Utc>::MIN_UTC;
        let pins = extract_pins_from_messages(
            &existing_messages,
            &plan_ai_chat::tools::PinConfig::chat(),
        );

        // Replay persisted messages
        for msg in &existing_messages {
            if msg.created_at > last_seen_at {
                last_seen_at = msg.created_at;
            }
            let _ = tx
                .send(Ok(event_json(&ChatStreamEvent {
                    kind: "message".to_string(),
                    role: Some(msg.role.clone()),
                    content: Some(msg.content.clone()),
                    metadata: msg.metadata.clone(),
                    ..empty()
                })))
                .await;
        }

        // Pins snapshot
        if !pins.is_empty() {
            let _ = tx
                .send(Ok(event_json(&ChatStreamEvent {
                    kind: "pins".to_string(),
                    pins: Some(pins.clone()),
                    ..empty()
                })))
                .await;
        }

        // Current state
        let _ = tx
            .send(Ok(event_json(&ChatStreamEvent {
                kind: "state".to_string(),
                state: Some(current_state.clone()),
                state_reason: current_reason.clone(),
                ..empty()
            })))
            .await;

        // Running tools + pending approvals snapshots
        let tools = manager.running_tools(uuid);
        if !tools.is_empty() {
            let _ = tx
                .send(Ok(event_json(&ChatStreamEvent {
                    kind: "running_tools".to_string(),
                    running_tools: Some(running_tools_to_wire(&tools)),
                    ..empty()
                })))
                .await;
        }
        for pending in manager.pending_approvals(uuid) {
            let _ = tx.send(Ok(event_json(&approval_event(&pending)))).await;
        }

        // Live events
        if is_running {
            if let Some(mut rx_bc) = manager.subscribe(uuid) {
                loop {
                    match rx_bc.recv().await {
                        Ok(event) => {
                            if let plan_ai_chat::ChatEvent::Message { created_at, .. } = &event {
                                if *created_at > last_seen_at {
                                    last_seen_at = *created_at;
                                }
                            }
                            let (stream_event, is_done) = chat_event_to_stream(&event);
                            let _ = tx.send(Ok(event_json(&stream_event))).await;
                            if is_done {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(
                                "chat SSE lagged, skipped {n} events — replaying from DB"
                            );
                            if let Ok(missed) = store.get_messages_after(uuid, last_seen_at).await
                            {
                                for msg in missed {
                                    if msg.created_at > last_seen_at {
                                        last_seen_at = msg.created_at;
                                    }
                                    let _ = tx
                                        .send(Ok(event_json(&ChatStreamEvent {
                                            kind: "message".to_string(),
                                            role: Some(msg.role.clone()),
                                            content: Some(msg.content.clone()),
                                            metadata: msg.metadata.clone(),
                                            ..empty()
                                        })))
                                        .await;
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }

        // Final done event
        let _ = tx
            .send(Ok(event_json(&ChatStreamEvent {
                kind: "done".to_string(),
                state: Some(current_state),
                state_reason: current_reason,
                ..empty()
            })))
            .await;
    });

    Sse::new(tokio_stream::wrappers::ReceiverStream::new(rx))
        .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(30)))
        .into_response()
}

/// SSE endpoint path (must not collide with Dioxus client-side routes).
pub const SSE_PATH: &str = "/_sse/chat/{session_id}";
