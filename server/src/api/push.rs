use std::collections::HashMap;
use std::sync::Arc;

use rocket::response::stream::{Event, EventStream};
use rocket::{get, Shutdown, State};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use uuid::Uuid;

pub use mac_mgmt_common::PushEvent as PushMessage;

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

/// Send a push message using the global PushChannels (for Dioxus server functions).
/// No-op if push channels are not initialized or no daemon is connected.
#[cfg(feature = "webui")]
pub async fn notify_global(customer_id: Uuid, msg: PushMessage) {
    if let Ok(channels) = crate::push_channels() {
        notify(&channels, customer_id, msg).await;
    }
}

/// Send a push message to a specific set of customers.
async fn notify_customers(channels: &PushChannels, customer_ids: &[Uuid], msg: PushMessage) {
    if customer_ids.is_empty() {
        return;
    }
    let map = channels.read().await;
    for cid in customer_ids {
        if let Some(tx) = map.get(cid) {
            let _ = tx.send(msg.clone());
        }
    }
}

/// Find every customer that uses any of the given skill channels — either
/// through a direct `customer_skills` assignment or transitively via a
/// `customer_bundles` membership whose bundle contains one of the channels.
pub async fn notify_skill_channel_customers(
    channels: &PushChannels,
    pool: &PgPool,
    channel_ids: &[Uuid],
) {
    if channel_ids.is_empty() {
        return;
    }
    let customer_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT customer_id FROM ( \
           SELECT customer_id FROM customer_skills WHERE skill_channel_id = ANY($1) \
           UNION \
           SELECT cb.customer_id FROM customer_bundles cb \
             JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
             WHERE bi.skill_channel_id = ANY($1) \
         ) u",
    )
    .bind(channel_ids)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    notify_customers(channels, &customer_ids, PushMessage::SyncSkills).await;
}

/// Dioxus server function variant: notify customers affected by changes to
/// the given skill channels.
#[cfg(feature = "webui")]
pub async fn notify_skill_channels_global(channel_ids: &[Uuid]) {
    if let (Ok(channels), Ok(pool)) = (crate::push_channels(), crate::server_pool()) {
        notify_skill_channel_customers(&channels, &pool, channel_ids).await;
    }
}

/// Find every customer that uses the given MCP server — either through a
/// direct `customer_mcp_servers` assignment or transitively via a
/// `customer_mcp_bundles` membership whose bundle contains the server.
pub async fn notify_mcp_server_customers(
    channels: &PushChannels,
    pool: &PgPool,
    mcp_server_id: Uuid,
) {
    let customer_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT customer_id FROM ( \
           SELECT customer_id FROM customer_mcp_servers WHERE mcp_server_id = $1 \
           UNION \
           SELECT cmb.customer_id FROM customer_mcp_bundles cmb \
             JOIN mcp_server_bundle_items mbi ON mbi.bundle_id = cmb.bundle_id \
             WHERE mbi.mcp_server_id = $1 \
         ) u",
    )
    .bind(mcp_server_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    notify_customers(channels, &customer_ids, PushMessage::SyncMcpServers).await;
}

/// Dioxus server function variant: notify customers affected by changes to
/// the given MCP server.
#[cfg(feature = "webui")]
pub async fn notify_mcp_server_global(mcp_server_id: Uuid) {
    if let (Ok(channels), Ok(pool)) = (crate::push_channels(), crate::server_pool()) {
        notify_mcp_server_customers(&channels, &pool, mcp_server_id).await;
    }
}

/// Notify all customers targeted by a rollout's currently-rolling stages.
pub async fn notify_rollout_customers(channels: &PushChannels, pool: &PgPool, rollout_id: Uuid, msg: PushMessage) {
    let customer_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT rgm.customer_id FROM rollout_stages rs \
         JOIN LATERAL ( \
           SELECT customer_id FROM rollout_group_members WHERE group_id = rs.group_id \
           UNION ALL \
           SELECT id FROM customers WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
         ) rgm ON true \
         WHERE rs.rollout_id = $1 AND rs.status = 'rolling'",
    )
    .bind(rollout_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let map = channels.read().await;
    for cid in customer_ids {
        if let Some(tx) = map.get(&cid) {
            let _ = tx.send(msg.clone());
        }
    }
}

/// Same as notify_rollout_customers but for all stages (used on complete).
pub async fn notify_all_rollout_customers(channels: &PushChannels, pool: &PgPool, rollout_id: Uuid, msg: PushMessage) {
    let customer_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT rgm.customer_id FROM rollout_stages rs \
         JOIN LATERAL ( \
           SELECT customer_id FROM rollout_group_members WHERE group_id = rs.group_id \
           UNION ALL \
           SELECT id FROM customers WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
         ) rgm ON true \
         WHERE rs.rollout_id = $1",
    )
    .bind(rollout_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let map = channels.read().await;
    for cid in customer_ids {
        if let Some(tx) = map.get(&cid) {
            let _ = tx.send(msg.clone());
        }
    }
}

/// Dioxus server function variant: notify rollout customers using global state.
#[cfg(feature = "webui")]
pub async fn notify_rollout_global(rollout_id: Uuid, msg: PushMessage) {
    if let (Ok(channels), Ok(pool)) = (crate::push_channels(), crate::server_pool()) {
        notify_rollout_customers(&channels, &pool, rollout_id, msg).await;
    }
}

/// Dioxus server function variant: notify ALL customers in a rollout.
#[cfg(feature = "webui")]
pub async fn notify_all_rollout_global(rollout_id: Uuid, msg: PushMessage) {
    if let (Ok(channels), Ok(pool)) = (crate::push_channels(), crate::server_pool()) {
        notify_all_rollout_customers(&channels, &pool, rollout_id, msg).await;
    }
}

/// Notify all customers that have a given bundle assigned via `assignment_table`
/// (which must have `customer_id` and `bundle_id` columns).
pub async fn notify_bundle_customers(
    channels: &PushChannels,
    pool: &PgPool,
    assignment_table: &str,
    bundle_id: Uuid,
    msg: PushMessage,
) {
    let query = format!(
        "SELECT DISTINCT customer_id FROM {assignment_table} WHERE bundle_id = $1"
    );
    let customer_ids: Vec<Uuid> = sqlx::query_scalar(&query)
        .bind(bundle_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    let map = channels.read().await;
    for cid in customer_ids {
        if let Some(tx) = map.get(&cid) {
            let _ = tx.send(msg.clone());
        }
    }
}

/// Dioxus server function variant: notify all customers using a skill bundle.
#[cfg(feature = "webui")]
pub async fn notify_skill_bundle_global(bundle_id: Uuid) {
    if let (Ok(channels), Ok(pool)) = (crate::push_channels(), crate::server_pool()) {
        notify_bundle_customers(
            &channels,
            &pool,
            "customer_bundles",
            bundle_id,
            PushMessage::SyncSkills,
        )
        .await;
    }
}

/// Dioxus server function variant: notify all customers using an MCP bundle.
#[cfg(feature = "webui")]
pub async fn notify_mcp_bundle_global(bundle_id: Uuid) {
    if let (Ok(channels), Ok(pool)) = (crate::push_channels(), crate::server_pool()) {
        notify_bundle_customers(
            &channels,
            &pool,
            "customer_mcp_bundles",
            bundle_id,
            PushMessage::SyncMcpServers,
        )
        .await;
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
        let mut keepalive = tokio::time::interval(std::time::Duration::from_secs(30));
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
                _ = keepalive.tick() => {
                    yield Event::comment("");
                }
                _ = &mut shutdown => break,
            }
        }
    })
}
