use std::collections::HashMap;
use std::sync::Arc;

use rocket::response::stream::{Event, EventStream};
use rocket::{get, Shutdown, State};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PushMessage {
    SyncConfig,
    SyncSkills,
    SyncMcpServers,
    SyncSshKeys,
}

/// Per-customer broadcast channels for push notifications.
pub type PushChannels = Arc<RwLock<HashMap<Uuid, broadcast::Sender<PushMessage>>>>;

pub fn new_push_channels() -> PushChannels {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Send a push message to all connected daemons for a customer.
/// No-op if no daemon is connected.
pub async fn notify(channels: &PushChannels, customer_id: Uuid, msg: PushMessage) {
    let map = channels.read().await;
    if let Some(tx) = map.get(&customer_id) {
        let _ = tx.send(msg);
    }
}

/// SSE endpoint for daemon push notifications.
/// Auth via query param since SSE can't carry custom headers from all clients.
#[get("/events?<token>")]
pub async fn sse_events(
    token: &str,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    mut shutdown: Shutdown,
) -> Option<EventStream![]> {
    // Authenticate the token
    let hash = hex::encode(Sha256::digest(token.as_bytes()));

    let result = sqlx::query_as::<_, (Option<Uuid>, String)>(
        "SELECT customer_id, kind FROM tokens WHERE token_hash = $1 AND NOT revoked",
    )
    .bind(&hash)
    .fetch_optional(pool.inner())
    .await
    .ok()??;

    let (customer_id, kind) = result;
    if kind != "sync" {
        return None;
    }
    let customer_id = customer_id?;

    // Get or create broadcast channel for this customer
    let rx = {
        let mut map = channels.write().await;
        let tx = map
            .entry(customer_id)
            .or_insert_with(|| broadcast::channel(64).0);
        tx.subscribe()
    };

    Some(EventStream! {
        let mut rx = rx;
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Ok(push_msg) => {
                            let json = serde_json::to_string(&push_msg).unwrap_or_default();
                            yield Event::data(json);
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("SSE client lagged, skipped {n} messages");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = &mut shutdown => break,
            }
        }
    })
}
