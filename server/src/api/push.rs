use std::collections::HashMap;
use std::sync::Arc;

use rocket::response::stream::{Event, EventStream};
use rocket::{Shutdown, State, get};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use uuid::Uuid;

pub use mac_mgmt_common::FederationEvent;
pub use mac_mgmt_common::PushEvent as PushMessage;

/// Per-cluster broadcast channels for push notifications.
pub type PushChannels = Arc<RwLock<HashMap<Uuid, broadcast::Sender<PushMessage>>>>;

/// Single broadcast channel for federation events (skill center → mgmt server).
pub type FederationPushChannel = broadcast::Sender<FederationEvent>;

pub fn new_push_channels() -> PushChannels {
    Arc::new(RwLock::new(HashMap::new()))
}

pub fn new_federation_channel() -> FederationPushChannel {
    broadcast::channel(64).0
}

/// Notify all federation subscribers that the catalog has changed.
pub fn notify_federation(channel: &FederationPushChannel) {
    let _ = channel.send(FederationEvent::CatalogChanged);
}

/// Notify federation subscribers using the global channel (for Dioxus server functions).
#[cfg(feature = "webui")]
pub fn notify_federation_global() {
    if let Ok(channel) = crate::federation_channel() {
        notify_federation(&channel);
    }
}

/// Send a push message to all connected daemons for a cluster.
/// No-op if no daemon is connected.
pub async fn notify(channels: &PushChannels, cluster_id: Uuid, msg: PushMessage) {
    let _ = notify_counted(channels, cluster_id, msg).await;
}

/// Like [`notify`] but returns the number of connected daemon receivers the
/// message was delivered to (0 if none are connected). Used by the admin push
/// endpoint to report reach back to the caller.
pub async fn notify_counted(channels: &PushChannels, cluster_id: Uuid, msg: PushMessage) -> usize {
    let map = channels.read().await;
    match map.get(&cluster_id) {
        Some(tx) => tx.send(msg).unwrap_or(0),
        None => 0,
    }
}

/// Send a push message using the global PushChannels (for Dioxus server functions).
/// No-op if push channels are not initialized or no daemon is connected.
#[cfg(feature = "webui")]
pub async fn notify_global(cluster_id: Uuid, msg: PushMessage) {
    if let Ok(channels) = crate::push_channels() {
        notify(&channels, cluster_id, msg).await;
    }
}

/// Send a push message to a specific set of clusters.
async fn notify_clusters(channels: &PushChannels, cluster_ids: &[Uuid], msg: PushMessage) {
    if cluster_ids.is_empty() {
        return;
    }
    let map = channels.read().await;
    for cid in cluster_ids {
        if let Some(tx) = map.get(cid) {
            let _ = tx.send(msg.clone());
        }
    }
}

/// Find every cluster that uses any of the given skill channels — either
/// through a direct `cluster_skills` assignment or transitively via a
/// `cluster_bundles` membership whose bundle contains one of the channels.
pub async fn notify_skill_channel_clusters(
    channels: &PushChannels,
    pool: &PgPool,
    channel_ids: &[Uuid],
) {
    if channel_ids.is_empty() {
        return;
    }
    let cluster_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT cluster_id FROM ( \
           SELECT cluster_id FROM cluster_skills WHERE skill_channel_id = ANY($1) \
           UNION \
           SELECT cb.cluster_id FROM cluster_bundles cb \
             JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
             WHERE bi.skill_channel_id = ANY($1) \
         ) u",
    )
    .bind(channel_ids)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    notify_clusters(channels, &cluster_ids, PushMessage::SyncSkills).await;
}

/// Dioxus server function variant: notify clusters affected by changes to
/// the given skill channels.
#[cfg(feature = "webui")]
pub async fn notify_skill_channels_global(channel_ids: &[Uuid]) {
    if let (Ok(channels), Ok(pool)) = (crate::push_channels(), crate::server_pool()) {
        notify_skill_channel_clusters(&channels, &pool, channel_ids).await;
    }
}

/// Find every cluster that uses the given MCP server — either through a
/// direct `cluster_mcp_servers` assignment or transitively via a
/// `cluster_mcp_bundles` membership whose bundle contains the server.
pub async fn notify_mcp_server_clusters(
    channels: &PushChannels,
    pool: &PgPool,
    mcp_server_id: Uuid,
) {
    let cluster_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT cluster_id FROM ( \
           SELECT cluster_id FROM cluster_mcp_servers WHERE mcp_server_id = $1 \
           UNION \
           SELECT cmb.cluster_id FROM cluster_mcp_bundles cmb \
             JOIN mcp_server_bundle_items mbi ON mbi.bundle_id = cmb.bundle_id \
             WHERE mbi.mcp_server_id = $1 \
         ) u",
    )
    .bind(mcp_server_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    notify_clusters(channels, &cluster_ids, PushMessage::SyncMcpServers).await;
}

/// Dioxus server function variant: notify clusters affected by changes to
/// the given MCP server.
#[cfg(feature = "webui")]
pub async fn notify_mcp_server_global(mcp_server_id: Uuid) {
    if let (Ok(channels), Ok(pool)) = (crate::push_channels(), crate::server_pool()) {
        notify_mcp_server_clusters(&channels, &pool, mcp_server_id).await;
    }
}

/// Notify all clusters targeted by a rollout's currently-rolling stages.
pub async fn notify_rollout_clusters(
    channels: &PushChannels,
    pool: &PgPool,
    rollout_id: Uuid,
    msg: PushMessage,
) {
    let cluster_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT rgm.cluster_id FROM rollout_stages rs \
         JOIN LATERAL ( \
           SELECT cluster_id FROM rollout_group_members WHERE group_id = rs.group_id \
           UNION ALL \
           SELECT id FROM clusters WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
         ) rgm ON true \
         WHERE rs.rollout_id = $1 AND rs.status = 'rolling'",
    )
    .bind(rollout_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let map = channels.read().await;
    for cid in cluster_ids {
        if let Some(tx) = map.get(&cid) {
            let _ = tx.send(msg.clone());
        }
    }
}

/// Same as notify_rollout_clusters but for all stages (used on complete).
pub async fn notify_all_rollout_clusters(
    channels: &PushChannels,
    pool: &PgPool,
    rollout_id: Uuid,
    msg: PushMessage,
) {
    let cluster_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT rgm.cluster_id FROM rollout_stages rs \
         JOIN LATERAL ( \
           SELECT cluster_id FROM rollout_group_members WHERE group_id = rs.group_id \
           UNION ALL \
           SELECT id FROM clusters WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
         ) rgm ON true \
         WHERE rs.rollout_id = $1",
    )
    .bind(rollout_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let map = channels.read().await;
    for cid in cluster_ids {
        if let Some(tx) = map.get(&cid) {
            let _ = tx.send(msg.clone());
        }
    }
}

/// Dioxus server function variant: notify rollout clusters using global state.
#[cfg(feature = "webui")]
pub async fn notify_rollout_global(rollout_id: Uuid, msg: PushMessage) {
    if let (Ok(channels), Ok(pool)) = (crate::push_channels(), crate::server_pool()) {
        notify_rollout_clusters(&channels, &pool, rollout_id, msg).await;
    }
}

/// Dioxus server function variant: notify ALL clusters in a rollout.
#[cfg(feature = "webui")]
pub async fn notify_all_rollout_global(rollout_id: Uuid, msg: PushMessage) {
    if let (Ok(channels), Ok(pool)) = (crate::push_channels(), crate::server_pool()) {
        notify_all_rollout_clusters(&channels, &pool, rollout_id, msg).await;
    }
}

/// Bundle assignment join table; mapped to a literal SQL identifier rather
/// than letting callers pass a raw &str so a future caller can't accidentally
/// flow user input into a `format!` SQL query.
#[derive(Copy, Clone)]
pub enum BundleAssignmentTable {
    Cluster,
    Mcp,
}

impl BundleAssignmentTable {
    fn as_sql(self) -> &'static str {
        match self {
            BundleAssignmentTable::Cluster => "cluster_bundles",
            BundleAssignmentTable::Mcp => "cluster_mcp_bundles",
        }
    }
}

/// Notify all clusters that have a given bundle assigned via the named
/// assignment table (which must have `cluster_id` and `bundle_id` columns).
pub async fn notify_bundle_clusters(
    channels: &PushChannels,
    pool: &PgPool,
    assignment_table: BundleAssignmentTable,
    bundle_id: Uuid,
    msg: PushMessage,
) {
    let query = format!(
        "SELECT DISTINCT cluster_id FROM {} WHERE bundle_id = $1",
        assignment_table.as_sql()
    );
    let cluster_ids: Vec<Uuid> = sqlx::query_scalar(&query)
        .bind(bundle_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    let map = channels.read().await;
    for cid in cluster_ids {
        if let Some(tx) = map.get(&cid) {
            let _ = tx.send(msg.clone());
        }
    }
}

/// Dioxus server function variant: notify all clusters using a skill bundle.
#[cfg(feature = "webui")]
pub async fn notify_skill_bundle_global(bundle_id: Uuid) {
    if let (Ok(channels), Ok(pool)) = (crate::push_channels(), crate::server_pool()) {
        notify_bundle_clusters(
            &channels,
            &pool,
            BundleAssignmentTable::Cluster,
            bundle_id,
            PushMessage::SyncSkills,
        )
        .await;
    }
}

/// Dioxus server function variant: notify all clusters using an MCP bundle.
#[cfg(feature = "webui")]
pub async fn notify_mcp_bundle_global(bundle_id: Uuid) {
    if let (Ok(channels), Ok(pool)) = (crate::push_channels(), crate::server_pool()) {
        notify_bundle_clusters(
            &channels,
            &pool,
            BundleAssignmentTable::Mcp,
            bundle_id,
            PushMessage::SyncMcpServers,
        )
        .await;
    }
}

/// SSE endpoint for daemon push notifications.
/// Token comes from `Authorization: Bearer ...`; `?token=` is accepted for
/// backward compatibility with daemons that predate the header-based call.
#[get("/events")]
pub async fn sse_events(
    auth: crate::api::auth::SseTokenAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    mut shutdown: Shutdown,
) -> Option<EventStream![]> {
    let hash = hex::encode(Sha256::digest(auth.0.as_bytes()));

    let result = sqlx::query_as::<_, (Option<Uuid>, String)>(
        "SELECT cluster_id, kind FROM tokens WHERE token_hash = $1 AND NOT revoked",
    )
    .bind(&hash)
    .fetch_optional(pool.inner())
    .await
    .ok()??;

    let (cluster_id, kind) = result;
    if kind != "sync" {
        return None;
    }
    let cluster_id = cluster_id?;

    // Get or create broadcast channel for this cluster
    let rx = {
        let mut map = channels.write().await;
        let tx = map
            .entry(cluster_id)
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
                    let json = serde_json::to_string(&PushMessage::Ping).unwrap_or_default();
                    yield Event::data(json);
                }
                _ = &mut shutdown => break,
            }
        }
    })
}
