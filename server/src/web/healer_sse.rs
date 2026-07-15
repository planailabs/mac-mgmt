//! Axum SSE endpoints for healer session streaming.
//!
//! These bypass Dioxus's JsonStream (which breaks through nginx) and use
//! standard `text/event-stream` SSE over GET, which nginx handles natively.

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

use super::user::{WebUser, WebUserExt};
use crate::server_state;
use mac_mgmt_common::ChatStreamEvent;

fn event_json(evt: &ChatStreamEvent) -> Event {
    Event::default().data(serde_json::to_string(evt).unwrap_or_default())
}

/// GET /web/healer/stream/:session_id — SSE stream for an existing session.
pub async fn view_session_sse(
    user: Option<Extension<WebUser>>,
    Path(session_id): Path<String>,
) -> Response {
    let Some(Extension(user)) = user else {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(healer) = server_state::healer_state() else {
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Ok(uuid) = session_id.parse::<Uuid>() else {
        return axum::http::StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(pool) = server_state::server_pool() else {
        return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    // Verify user can access this session's cluster
    let Ok(Some((session, existing_messages))) = healer.get_session(uuid).await else {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    };
    if user
        .require_cluster_read(&pool, session.cluster_id)
        .await
        .is_err()
    {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }

    let is_active = session.state.is_active();
    let current_state = session.state.as_str().to_string();
    let current_reason = session
        .state_data
        .get("reason")
        .and_then(|v| v.as_str())
        .map(String::from);

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        use super::components::chat_ui::{
            PinInfo, chat_event_to_stream, extract_pins_from_messages, running_tools_to_wire,
        };
        use super::components::healer_page::staff_pings_to_wire;

        let empty = ChatStreamEvent::default;

        let mut last_seen_at = chrono::DateTime::<chrono::Utc>::MIN_UTC;
        let mut pins = extract_pins_from_messages(
            &existing_messages,
            &plan_ai_chat::tools::PinConfig::healer(),
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

        // Send pins snapshot
        if !pins.is_empty() {
            let _ = tx
                .send(Ok(event_json(&ChatStreamEvent {
                    kind: "pins".to_string(),
                    pins: Some(pins.clone()),
                    ..empty()
                })))
                .await;
        }

        // Send staff pings
        if let Ok(pings) = healer.store().list_session_pings(uuid).await {
            if !pings.is_empty() {
                let _ = tx
                    .send(Ok(event_json(&ChatStreamEvent {
                        kind: "staff_pings".to_string(),
                        staff_pings: Some(staff_pings_to_wire(&pings)),
                        ..empty()
                    })))
                    .await;
            }
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

        // Running tools snapshot
        let tools = healer.running_tools(uuid);
        if !tools.is_empty() {
            let _ = tx
                .send(Ok(event_json(&ChatStreamEvent {
                    kind: "running_tools".to_string(),
                    running_tools: Some(running_tools_to_wire(&tools)),
                    ..empty()
                })))
                .await;
        }

        // Stream live events
        if is_active {
            if let Some(mut rx_bc) = healer.subscribe(uuid) {
                loop {
                    match rx_bc.recv().await {
                        Ok(event) => {
                            if let mac_mgmt_healer::HealerEvent::Message { created_at, .. } = &event
                            {
                                if *created_at > last_seen_at {
                                    last_seen_at = *created_at;
                                }
                            }

                            let (stream_event, is_done) = chat_event_to_stream(&event);
                            let _ = tx.send(Ok(event_json(&stream_event))).await;

                            // Re-send pins/staff_pings on relevant messages
                            if let mac_mgmt_healer::HealerEvent::Message { role, content, .. } =
                                &event
                            {
                                if role == "pin" {
                                    if let Ok(data) =
                                        serde_json::from_str::<serde_json::Value>(content)
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
                                            pins.retain(|p| p.slot != pin.slot);
                                            pins.push(pin);
                                            let _ = tx
                                                .send(Ok(event_json(&ChatStreamEvent {
                                                    kind: "pins".to_string(),
                                                    pins: Some(pins.clone()),
                                                    ..empty()
                                                })))
                                                .await;
                                        }
                                    }
                                }

                                if role == "tool_result" && content.starts_with("staff_ping:") {
                                    if let Ok(pings) = healer.store().list_session_pings(uuid).await
                                    {
                                        let _ = tx
                                            .send(Ok(event_json(&ChatStreamEvent {
                                                kind: "staff_pings".to_string(),
                                                staff_pings: Some(staff_pings_to_wire(&pings)),
                                                ..empty()
                                            })))
                                            .await;
                                    }
                                }
                            }

                            if is_done {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(
                                "healer SSE view lagged, skipped {n} events — replaying from DB"
                            );
                            if let Ok(missed) =
                                healer.store().get_messages_after(uuid, last_seen_at).await
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
/// Mounted in main.rs alongside the other web routes.
pub const SSE_PATH: &str = "/_sse/healer/{session_id}";
