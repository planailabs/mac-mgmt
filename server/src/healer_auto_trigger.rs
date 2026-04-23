//! Auto-trigger healer sessions for persistently unhealthy instances.
//!
//! A background loop runs every 60 seconds, scanning heartbeats for instances
//! with unhealthy services. An in-memory counter tracks how many consecutive
//! scan ticks each instance has been unhealthy. When the counter reaches the
//! configured threshold, a healer session is spawned automatically.
//!
//! Design choices:
//! - Counters are in-memory (DashMap). Server restart resets them, which is
//!   fine — it avoids stale state and the threshold is low enough to re-trigger.
//! - Uses Ollama by default so auto-sessions are free and don't pause for budget.
//! - Skips instances whose last session ended in `needs_human_attention`.

use mac_mgmt_healer::agent::InstanceInfo;
use mac_mgmt_healer::{HealerState, SpawnRequest};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;
use uuid::Uuid;

/// Configuration for the auto-trigger loop.
pub struct AutoTriggerConfig {
    pub threshold: u32,
    pub provider: String,
    pub model: Option<String>,
}

/// Fetch the per-cluster healer overrides from the latest cluster config.
async fn cluster_healer_config(
    pool: &PgPool,
    cluster_id: Uuid,
) -> mac_mgmt_common::HealerClusterConfig {
    let json = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT config_json FROM cluster_configs \
         WHERE cluster_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(cluster_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    json.and_then(|v| {
        v.get("healer")
            .cloned()
            .and_then(|h| serde_json::from_value(h).ok())
    })
    .unwrap_or_default()
}

/// Run the auto-trigger background loop. Never returns.
pub async fn run_auto_trigger_loop(
    pool: PgPool,
    healer: HealerState,
    config: AutoTriggerConfig,
    interval: Duration,
) {
    let counters: Mutex<HashMap<String, u32>> = Mutex::new(HashMap::new());

    tracing::info!(
        threshold = config.threshold,
        provider = %config.provider,
        model = config.model.as_deref().unwrap_or("(default)"),
        "healer auto-trigger enabled, scanning every {}s",
        interval.as_secs()
    );

    loop {
        tokio::time::sleep(interval).await;

        if let Err(e) = tick(&pool, &healer, &config, &counters).await {
            tracing::error!("healer auto-trigger tick failed: {e:#}");
        }
    }
}

async fn tick(
    pool: &PgPool,
    healer: &HealerState,
    config: &AutoTriggerConfig,
    counters: &Mutex<HashMap<String, u32>>,
) -> anyhow::Result<()> {
    // 1. Find all instances with recent heartbeats (last 10 minutes)
    #[derive(sqlx::FromRow)]
    struct HbRow {
        instance_id: String,
        cluster_id: Uuid,
        services_extended: Option<serde_json::Value>,
    }
    let heartbeats = sqlx::query_as::<_, HbRow>(
        "SELECT instance_id, cluster_id, services_extended \
         FROM daemon_heartbeats \
         WHERE reported_at > now() - interval '10 minutes'",
    )
    .fetch_all(pool)
    .await?;

    // 2. Determine which instances are unhealthy
    let mut currently_unhealthy: HashSet<String> = HashSet::new();
    let mut instance_cluster: std::collections::HashMap<String, Uuid> =
        std::collections::HashMap::new();

    for hb in &heartbeats {
        instance_cluster.insert(hb.instance_id.clone(), hb.cluster_id);

        let has_unhealthy = hb
            .services_extended
            .as_ref()
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().any(|svc| {
                    svc.get("healthy").and_then(|h| h.as_bool()) == Some(false)
                })
            })
            .unwrap_or(false);

        if has_unhealthy {
            currently_unhealthy.insert(hb.instance_id.clone());
        }
    }

    // 3. Update counters: increment unhealthy, remove healthy
    //    Also remove counters for instances we haven't seen (offline).
    let known_instances: HashSet<String> =
        heartbeats.iter().map(|h| h.instance_id.clone()).collect();

    let mut to_trigger: Vec<(String, Uuid)> = Vec::new();
    {
        let mut map = counters.lock().unwrap();
        map.retain(|k, _| known_instances.contains(k));

        for instance_id in &known_instances {
            if currently_unhealthy.contains(instance_id) {
                let count = map.entry(instance_id.clone()).or_insert(0);
                *count += 1;
            } else {
                map.remove(instance_id);
            }
        }

        // 4. Find instances that crossed the threshold
        for (iid, count) in map.iter() {
            if *count >= config.threshold {
                if let Some(&cluster_id) = instance_cluster.get(iid) {
                    to_trigger.push((iid.clone(), cluster_id));
                }
            }
        }

        // Reset triggered counters
        for (iid, _) in &to_trigger {
            map.remove(iid);
        }
    }

    // 5. For each, try to spawn a session
    for (instance_id, cluster_id) in to_trigger {

        // Skip if last session ended in needs_human_attention
        let needs_human = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM healer_sessions \
             WHERE instance_id = $1 AND state = 'needs_human_attention' \
             AND completed_at > now() - interval '24 hours')",
        )
        .bind(&instance_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if needs_human {
            tracing::debug!(
                instance_id = %instance_id,
                "skipping auto-trigger: last session needs human attention"
            );
            continue;
        }

        // Check per-cluster healer overrides.
        let cluster_healer = cluster_healer_config(pool, cluster_id).await;
        if cluster_healer.auto_trigger == Some(false) {
            tracing::debug!(
                instance_id = %instance_id,
                cluster_id = %cluster_id,
                "auto-trigger disabled for this cluster"
            );
            continue;
        }

        // Build SpawnRequest from heartbeat data
        match build_spawn_request(pool, &instance_id, cluster_id, config, &cluster_healer).await {
            Ok(req) => {
                match healer.spawn_session(req).await {
                    Ok(session_id) => {
                        tracing::info!(
                            instance_id = %instance_id,
                            session_id = %session_id,
                            "auto-triggered healer session"
                        );
                    }
                    Err(e) => {
                        // Expected errors: concurrent session, cooldown, no relay URL.
                        // These are normal — don't log at error level.
                        tracing::debug!(
                            instance_id = %instance_id,
                            err = %e,
                            "auto-trigger spawn skipped"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::debug!(
                    instance_id = %instance_id,
                    err = %e,
                    "auto-trigger: failed to build spawn request"
                );
            }
        }
    }

    Ok(())
}

async fn build_spawn_request(
    pool: &PgPool,
    instance_id: &str,
    cluster_id: Uuid,
    config: &AutoTriggerConfig,
    cluster_healer: &mac_mgmt_common::HealerClusterConfig,
) -> anyhow::Result<SpawnRequest> {
    #[derive(sqlx::FromRow)]
    struct HbInfo {
        relay_proxy_url: Option<String>,
        services_extended: Option<serde_json::Value>,
        file_tunnels: Option<serde_json::Value>,
        shell_tunnels: Option<serde_json::Value>,
        sample: Option<serde_json::Value>,
        hostname: Option<String>,
    }
    let hb: HbInfo = sqlx::query_as(
        "SELECT relay_proxy_url, services_extended, \
                file_tunnels, shell_tunnels, sample, hostname \
         FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
    )
    .bind(instance_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("instance not found"))?;

    let relay_url = hb
        .relay_proxy_url
        .filter(|u| !u.is_empty())
        .ok_or_else(|| anyhow::anyhow!("no relay URL for instance {instance_id}"))?;

    // Build relay access for the session
    let pg_store = mac_mgmt_healer::store::pg::PgHealerStore::new(pool.clone());
    let (proxy_token, proxy_expires) = pg_store
        .mint_proxy_token(cluster_id, None)
        .await?;
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
        .bind(cluster_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| cluster_id.to_string());

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
    .bind(cluster_id)
    .bind(instance_id)
    .fetch_all(pool)
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

    Ok(SpawnRequest {
        cluster_id,
        instance_id: instance_id.to_string(),
        created_by: "auto:unhealthy".to_string(),
        user_message: None,
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
        skip_cooldown: false,
        provider: Some(config.provider.clone()),
        model: config.model.clone(),
        label: Some("auto-triggered".to_string()),
        token_budget: None,
        proxy_expires: Some(proxy_expires),
        // Priority: cluster config > server global
        auto_approve: cluster_healer.auto_approve.unwrap_or(true),
        fix_provider: cluster_healer.fix_provider.clone()
            .or_else(|| crate::config::load().healer.fix_provider.clone()),
        fix_model: cluster_healer.fix_model.clone()
            .or_else(|| crate::config::load().healer.fix_model.clone()),
    })
}
