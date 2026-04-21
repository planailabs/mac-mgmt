pub mod models;
pub mod store;

pub use models::*;

/// Send a `state_change` message event followed by a `State` event over the SSE broadcast.
/// Call this after `store::transition_state` (which handles the DB-side message).
pub fn emit_state_change(
    events_tx: &tokio::sync::broadcast::Sender<HealerEvent>,
    state: &str,
    state_data: &serde_json::Value,
) {
    let reason = state_data
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let content = serde_json::json!({
        "state": state,
        "reason": reason,
    })
    .to_string();
    let _ = events_tx.send(HealerEvent::Message {
        role: "state_change".to_string(),
        content,
        metadata: Some(state_data.clone()),
        created_at: chrono::Utc::now(),
    });
    let _ = events_tx.send(HealerEvent::State {
        state: state.to_string(),
        state_data: state_data.clone(),
    });
}
