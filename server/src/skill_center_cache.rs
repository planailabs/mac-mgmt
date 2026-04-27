use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use mac_mgmt_common::{FederationCatalog, FederationEvent};
use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::api::push::{PushChannels, PushMessage};
use crate::skill_center_client::SkillCenterClient;

static GLOBAL_CACHE: std::sync::OnceLock<SkillCenterCache> = std::sync::OnceLock::new();

/// Cached catalog from a single skill center.
#[derive(Clone)]
pub struct CachedCatalog {
    pub catalog: FederationCatalog,
    pub fetched_at: DateTime<Utc>,
}

/// In-memory cache of catalogs from all registered skill centers.
#[derive(Clone)]
pub struct SkillCenterCache {
    inner: Arc<RwLock<HashMap<Uuid, CachedCatalog>>>,
}

impl SkillCenterCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get(&self, skill_center_id: &Uuid) -> Option<CachedCatalog> {
        self.inner.read().await.get(skill_center_id).cloned()
    }

    pub async fn get_all(&self) -> HashMap<Uuid, CachedCatalog> {
        self.inner.read().await.clone()
    }

    /// Set the global cache instance (called once during init).
    pub fn set_global(cache: SkillCenterCache) {
        let _ = GLOBAL_CACHE.set(cache);
    }

    /// Get the global cache instance (for Dioxus server functions).
    pub fn global() -> Option<SkillCenterCache> {
        GLOBAL_CACHE.get().cloned()
    }

    pub async fn update(&self, skill_center_id: Uuid, catalog: FederationCatalog) {
        self.inner.write().await.insert(
            skill_center_id,
            CachedCatalog {
                catalog,
                fetched_at: Utc::now(),
            },
        );
    }

    async fn remove(&self, skill_center_id: &Uuid) {
        self.inner.write().await.remove(skill_center_id);
    }
}

#[derive(sqlx::FromRow)]
struct SkillCenterRow {
    id: Uuid,
    url: String,
    federation_token: String,
    name: String,
}

/// Refresh the catalog for a single skill center. Detects deletions (Option A)
/// and cleans up dangling assignments.
async fn refresh_one(
    sc: &SkillCenterRow,
    cache: &SkillCenterCache,
    pool: &PgPool,
    push_channels: &PushChannels,
) {
    let client = SkillCenterClient::new(sc.url.clone(), sc.federation_token.clone());

    let new_catalog = match client.fetch_catalog().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("failed to refresh catalog from skill center '{}': {e}", sc.name);
            return;
        }
    };

    // Option A: diff against previous cache to detect deletions
    if let Some(old) = cache.get(&sc.id).await {
        let mut affected_clusters: HashSet<Uuid> = HashSet::new();

        // Detect deleted skill channels
        let new_sc_ids: HashSet<Uuid> = new_catalog
            .skill_channels
            .iter()
            .map(|s| s.id)
            .collect();
        let deleted_sc_ids: Vec<Uuid> = old
            .catalog
            .skill_channels
            .iter()
            .map(|s| s.id)
            .filter(|id| !new_sc_ids.contains(id))
            .collect();

        if !deleted_sc_ids.is_empty() {
            let cluster_ids: Vec<Uuid> = sqlx::query_scalar(
                "DELETE FROM cluster_skills \
                 WHERE skill_center_id = $1 AND remote_id = ANY($2) \
                 RETURNING cluster_id",
            )
            .bind(sc.id)
            .bind(&deleted_sc_ids)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            affected_clusters.extend(cluster_ids);
        }

        // Detect deleted bundles
        let new_bundle_ids: HashSet<Uuid> =
            new_catalog.bundles.iter().map(|b| b.id).collect();
        let deleted_bundle_ids: Vec<Uuid> = old
            .catalog
            .bundles
            .iter()
            .map(|b| b.id)
            .filter(|id| !new_bundle_ids.contains(id))
            .collect();

        if !deleted_bundle_ids.is_empty() {
            let cluster_ids: Vec<Uuid> = sqlx::query_scalar(
                "DELETE FROM cluster_bundles \
                 WHERE skill_center_id = $1 AND remote_id = ANY($2) \
                 RETURNING cluster_id",
            )
            .bind(sc.id)
            .bind(&deleted_bundle_ids)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            affected_clusters.extend(cluster_ids);
        }

        // Push SyncSkills to clusters that had skills/bundles removed
        if !affected_clusters.is_empty() {
            let map = push_channels.read().await;
            for cid in &affected_clusters {
                if let Some(tx) = map.get(cid) {
                    let _ = tx.send(PushMessage::SyncSkills);
                }
            }
        }

        // Detect deleted MCP servers
        let new_mcp_ids: HashSet<Uuid> = new_catalog
            .mcp_servers
            .iter()
            .map(|m| m.id)
            .collect();
        let deleted_mcp_ids: Vec<Uuid> = old
            .catalog
            .mcp_servers
            .iter()
            .map(|m| m.id)
            .filter(|id| !new_mcp_ids.contains(id))
            .collect();

        let mut mcp_affected: HashSet<Uuid> = HashSet::new();

        if !deleted_mcp_ids.is_empty() {
            let cluster_ids: Vec<Uuid> = sqlx::query_scalar(
                "DELETE FROM cluster_mcp_servers \
                 WHERE skill_center_id = $1 AND remote_id = ANY($2) \
                 RETURNING cluster_id",
            )
            .bind(sc.id)
            .bind(&deleted_mcp_ids)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            mcp_affected.extend(cluster_ids);
        }

        // Detect deleted MCP bundles
        let new_mcp_bundle_ids: HashSet<Uuid> = new_catalog
            .mcp_bundles
            .iter()
            .map(|b| b.id)
            .collect();
        let deleted_mcp_bundle_ids: Vec<Uuid> = old
            .catalog
            .mcp_bundles
            .iter()
            .map(|b| b.id)
            .filter(|id| !new_mcp_bundle_ids.contains(id))
            .collect();

        if !deleted_mcp_bundle_ids.is_empty() {
            let cluster_ids: Vec<Uuid> = sqlx::query_scalar(
                "DELETE FROM cluster_mcp_bundles \
                 WHERE skill_center_id = $1 AND remote_id = ANY($2) \
                 RETURNING cluster_id",
            )
            .bind(sc.id)
            .bind(&deleted_mcp_bundle_ids)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            mcp_affected.extend(cluster_ids);
        }

        // Push SyncMcpServers to clusters that had MCP items removed
        if !mcp_affected.is_empty() {
            let map = push_channels.read().await;
            for cid in &mcp_affected {
                if let Some(tx) = map.get(cid) {
                    let _ = tx.send(PushMessage::SyncMcpServers);
                }
            }
        }
    }

    // Update the cache with the new catalog
    cache.update(sc.id, new_catalog).await;
}

/// Background task: for each enabled skill center, connect to its federation
/// SSE stream and refresh the catalog on `CatalogChanged` events.
/// Falls back to polling on the given interval if SSE is unavailable.
pub async fn run_cache_refresh_loop(
    cache: SkillCenterCache,
    pool: PgPool,
    push_channels: PushChannels,
    interval: std::time::Duration,
) {
    // Initial refresh for all skill centers
    refresh_all(&cache, &pool, &push_channels).await;

    // Spawn per-skill-center SSE listeners
    spawn_sse_listeners(&cache, &pool, &push_channels).await;

    // Fallback polling loop
    loop {
        tokio::time::sleep(interval).await;
        refresh_all(&cache, &pool, &push_channels).await;
        // Re-check for newly added skill centers
        spawn_sse_listeners(&cache, &pool, &push_channels).await;
    }
}

/// Refresh catalogs for all enabled skill centers.
async fn refresh_all(cache: &SkillCenterCache, pool: &PgPool, push_channels: &PushChannels) {
    let skill_centers: Vec<SkillCenterRow> = sqlx::query_as(
        "SELECT id, url, federation_token, name FROM skill_centers WHERE enabled = true",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for sc in &skill_centers {
        refresh_one(sc, cache, pool, push_channels).await;
    }

    // Clean up cache entries for skill centers that were removed/disabled
    let active_ids: HashSet<Uuid> = skill_centers.iter().map(|s| s.id).collect();
    let cached = cache.get_all().await;
    for id in cached.keys() {
        if !active_ids.contains(id) {
            cache.remove(id).await;
        }
    }
}

/// Track which skill centers already have SSE listeners.
static SSE_LISTENERS: std::sync::LazyLock<Arc<RwLock<HashSet<Uuid>>>> =
    std::sync::LazyLock::new(|| Arc::new(RwLock::new(HashSet::new())));

/// Spawn SSE listener tasks for skill centers that don't have one yet.
async fn spawn_sse_listeners(
    cache: &SkillCenterCache,
    pool: &PgPool,
    push_channels: &PushChannels,
) {
    let skill_centers: Vec<SkillCenterRow> = sqlx::query_as(
        "SELECT id, url, federation_token, name FROM skill_centers WHERE enabled = true",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut listeners = SSE_LISTENERS.write().await;

    for sc in skill_centers {
        if listeners.contains(&sc.id) {
            continue;
        }
        listeners.insert(sc.id);

        let cache = cache.clone();
        let pool = pool.clone();
        let push_channels = push_channels.clone();
        let sc_id = sc.id;
        let sc_name = sc.name.clone();

        tokio::spawn(async move {
            sse_listener_loop(sc, cache, pool, push_channels).await;
            // If the loop exits, remove from the set so it can be respawned
            SSE_LISTENERS.write().await.remove(&sc_id);
            tracing::info!("SSE listener for skill center '{}' exited, will respawn on next poll", sc_name);
        });
    }
}

/// SSE listener for a single skill center. Reconnects on failure with backoff.
async fn sse_listener_loop(
    sc: SkillCenterRow,
    cache: SkillCenterCache,
    pool: PgPool,
    push_channels: PushChannels,
) {
    let mut backoff = std::time::Duration::from_secs(1);
    let max_backoff = std::time::Duration::from_secs(60);

    loop {
        let client = SkillCenterClient::new(sc.url.clone(), sc.federation_token.clone());

        match client.subscribe_events().await {
            Ok(mut rx) => {
                backoff = std::time::Duration::from_secs(1); // reset on success
                tracing::info!("connected to SSE stream for skill center '{}'", sc.name);

                while let Some(result) = rx.recv().await {
                    match result {
                        Ok(FederationEvent::CatalogChanged) => {
                            tracing::debug!(
                                "catalog changed event from skill center '{}'",
                                sc.name
                            );
                            refresh_one(&sc, &cache, &pool, &push_channels).await;
                        }
                        Ok(FederationEvent::Ping) => {} // keepalive, ignore
                        Err(e) => {
                            tracing::warn!(
                                "SSE error from skill center '{}': {e}",
                                sc.name
                            );
                            break; // reconnect
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "failed to connect SSE for skill center '{}': {e}",
                    sc.name
                );
            }
        }

        // Backoff before reconnecting
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(max_backoff);
    }
}
