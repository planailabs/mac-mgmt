use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
use crate::web::components::table_utils::{Searchable, SortableTh, TableToolbar};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FleetEntry {
    cluster_id: String,
    cluster_name: String,
    instance_id: String,
    hostname: String,
    environment: String,
    version: String,
    /// Git commit the daemon binary was built from. `None` on older daemons.
    #[serde(default)]
    git_sha: Option<String>,
    /// Number of commits leading up to git_sha (fetched from GitLab).
    #[serde(default)]
    commit_count: Option<u64>,
    #[serde(default)]
    nixpkgs_commit: Option<String>,
    services: serde_json::Value,
    tunnels: serde_json::Value,
    relay_proxy_hostname: Option<String>,
    relay_proxy_url: Option<String>,
    reported_at: DateTime<Utc>,
    /// Latest dynamic sample piggybacked on the heartbeat (CPU/mem/thermal).
    #[serde(default)]
    sample: Option<serde_json::Value>,
    /// Rolled-up extended service state from the last probe run.
    #[serde(default)]
    services_extended: Option<serde_json::Value>,
    /// True if the viewer may see probe error_detail (admin only).
    #[serde(default)]
    viewer_is_admin: bool,
}

/// Wrapped response so the UI can render a "Filtered by …" banner with
/// a human-readable label without a second round-trip per refresh.
/// `stage_label` is `None` when no stage filter is active.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FleetStatusResult {
    entries: Vec<FleetEntry>,
    stage_label: Option<String>,
}

#[server]
async fn get_fleet_status(stage_id: Option<String>) -> Result<FleetStatusResult, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let is_admin = user.is_admin;

    #[derive(sqlx::FromRow)]
    struct Row {
        cluster_id: uuid::Uuid,
        cluster_name: String,
        instance_id: String,
        hostname: String,
        environment: String,
        version: String,
        git_sha: Option<String>,
        nixpkgs_commit: Option<String>,
        services: serde_json::Value,
        tunnels: serde_json::Value,
        relay_proxy_hostname: Option<String>,
        relay_proxy_url: Option<String>,
        reported_at: DateTime<Utc>,
        sample: Option<serde_json::Value>,
        services_extended: Option<serde_json::Value>,
    }

    let accessible = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Optional stage filter — resolves the stage's cohort cluster_ids and
    // a human label ("Stage 1 · canary"). Intersects with accessible
    // clusters so org-scoped users can't see hosts they couldn't reach
    // in an unfiltered view.
    let (stage_cohort, stage_label) = if let Some(sid) = stage_id.as_deref() {
        let sid_uuid: uuid::Uuid = sid
            .parse()
            .map_err(|e: uuid::Error| ServerFnError::new(format!("invalid stage_id: {e}")))?;

        #[derive(sqlx::FromRow)]
        struct StageMeta {
            stage_order: i32,
            group_name: String,
            group_id: uuid::Uuid,
        }
        let meta: StageMeta = sqlx::query_as(
            "SELECT rs.stage_order, rg.name AS group_name, rs.group_id \
             FROM rollout_stages rs JOIN rollout_groups rg ON rg.id = rs.group_id \
             WHERE rs.id = $1",
        )
        .bind(sid_uuid)
        .fetch_optional(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("stage not found"))?;

        let cohort: Vec<uuid::Uuid> = sqlx::query_scalar(
            "SELECT cluster_id FROM rollout_group_members WHERE group_id = $1 \
             UNION ALL \
             SELECT id FROM clusters WHERE $1 = '00000000-0000-0000-0000-000000000000'::uuid",
        )
        .bind(meta.group_id)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        (
            Some(cohort),
            Some(format!("Stage {} · {}", meta.stage_order, meta.group_name)),
        )
    } else {
        (None, None)
    };

    // Compose the effective cluster_id filter from accessible ∩ cohort.
    // None on either side means "no filter from that source".
    let effective: Option<Vec<uuid::Uuid>> = match (accessible, stage_cohort) {
        (Some(a), Some(c)) => {
            let cset: std::collections::HashSet<_> = c.iter().copied().collect();
            Some(a.into_iter().filter(|id| cset.contains(id)).collect())
        }
        (Some(a), None) => Some(a),
        (None, Some(c)) => Some(c),
        (None, None) => None,
    };

    let rows = if let Some(ids) = effective {
        sqlx::query_as::<_, Row>(
            "SELECT c.id AS cluster_id, c.name AS cluster_name, dh.instance_id, dh.hostname, dh.environment, dh.version, dh.git_sha, dh.nixpkgs_commit, dh.services, dh.tunnels, dh.relay_proxy_hostname, dh.relay_proxy_url, dh.reported_at, dh.sample, dh.services_extended \
             FROM daemon_heartbeats dh \
             JOIN clusters c ON c.id = dh.cluster_id \
             WHERE dh.cluster_id = ANY($1) \
             ORDER BY dh.reported_at DESC",
        )
        .bind(&ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, Row>(
            "SELECT c.id AS cluster_id, c.name AS cluster_name, dh.instance_id, dh.hostname, dh.environment, dh.version, dh.git_sha, dh.nixpkgs_commit, dh.services, dh.tunnels, dh.relay_proxy_hostname, dh.relay_proxy_url, dh.reported_at, dh.sample, dh.services_extended \
             FROM daemon_heartbeats dh \
             JOIN clusters c ON c.id = dh.cluster_id \
             ORDER BY dh.reported_at DESC",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    let entries = rows
        .into_iter()
        .map(|r| {
            FleetEntry {
                cluster_id: r.cluster_id.to_string(),
                cluster_name: r.cluster_name,
                instance_id: r.instance_id,
                hostname: r.hostname,
                environment: r.environment,
                version: r.version,
                git_sha: r.git_sha,
                commit_count: None, // fetched async in the component
                nixpkgs_commit: r.nixpkgs_commit,
                services: r.services,
                tunnels: r.tunnels,
                relay_proxy_hostname: r.relay_proxy_hostname,
                relay_proxy_url: r.relay_proxy_url,
                reported_at: r.reported_at,
                sample: r.sample,
                services_extended: r.services_extended,
                viewer_is_admin: is_admin,
            }
        })
        .collect();

    Ok(FleetStatusResult {
        entries,
        stage_label,
    })
}

#[server]
async fn get_commit_counts(shas: Vec<String>) -> Result<std::collections::HashMap<String, u64>, ServerFnError> {
    let set: std::collections::HashSet<String> = shas.into_iter().collect();
    Ok(crate::commit_count::mac_mgmt_commit_counts(&set).await)
}

#[server]
async fn get_nixpkgs_commit_counts(shas: Vec<String>) -> Result<std::collections::HashMap<String, u64>, ServerFnError> {
    let set: std::collections::HashSet<String> = shas.into_iter().collect();
    Ok(crate::commit_count::nixpkgs_commit_counts(&set).await)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTokenResult {
    pub proxy_token: String,
}

/// Stale-instance delete. Server-side gate is the source of truth — the
/// UI hides the button when last-seen is recent, but a malicious or
/// stale browser tab can still call this directly. We:
///   1. Require write access to the cluster (admins always pass).
///   2. Re-read `reported_at` and reject if it's within the last 24h
///      so a delete can't race a fresh heartbeat.
///   3. Delete the heartbeat row. Migration 031 adds ON DELETE CASCADE
///      FKs from `assessments` and `assessment_probes` on
///      `(cluster_id, instance_id)`, so those rows go with it.
///      `rollout_stage_health_evaluations` references stage_id, not
///      instance, and cohort queries naturally exclude the missing
///      daemon.
#[server]
async fn delete_stale_instance(instance_id: String) -> Result<(), ServerFnError> {
    use chrono::Duration;

    let user = current_user().await?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        cluster_id: uuid::Uuid,
        reported_at: DateTime<Utc>,
    }
    let row: Row = sqlx::query_as(
        "SELECT cluster_id, reported_at FROM daemon_heartbeats WHERE instance_id = $1",
    )
    .bind(&instance_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("instance not found"))?;

    user.require_cluster_write(&pool, row.cluster_id).await?;

    let age = Utc::now().signed_duration_since(row.reported_at);
    if age < Duration::days(1) {
        return Err(ServerFnError::new(format!(
            "instance reported {}h ago — only stale instances (>24h) can be deleted",
            age.num_hours().max(0)
        )));
    }

    sqlx::query("DELETE FROM daemon_heartbeats WHERE instance_id = $1 AND cluster_id = $2")
        .bind(&instance_id)
        .bind(row.cluster_id)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
pub async fn create_proxy_token() -> Result<ProxyTokenResult, ServerFnError> {
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let user = current_user().await?;
    let pool = crate::server_pool()?;

    // Require org write access to mint proxy tokens.
    let writable = user
        .writable_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(6);

    // For admin users (writable == None), create an admin-scoped proxy token.
    // For org-scoped users, scope to their first writable cluster.
    let cluster_id: Option<uuid::Uuid> = writable.as_ref().and_then(|ids| ids.first().copied());
    if let Some(ref ids) = writable {
        if ids.is_empty() {
            return Err(ServerFnError::new(
                "write access required to create proxy tokens",
            ));
        }
    }

    let scopes = serde_json::json!(["tcp:*"]);
    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at, scopes) \
         VALUES ($1, $2, 'proxy', 'proxy', $3, $4)",
    )
    .bind(cluster_id)
    .bind(&hash)
    .bind(expires_at)
    .bind(&scopes)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(ProxyTokenResult {
        proxy_token: raw_token,
    })
}

impl FleetEntry {
    /// Returns true if any service is unhealthy or any probe has failed.
    fn has_unhealthy(&self) -> bool {
        let has_unhealthy_service = self
            .services
            .as_array()
            .map(|arr| {
                arr.iter().any(|s| {
                    s.get("healthy")
                        .and_then(|v| v.as_bool())
                        .map(|h| !h)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        let has_failed_probe = self
            .services_extended
            .as_ref()
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().any(|s| {
                    s.get("last_probe_ok")
                        .and_then(|v| v.as_bool())
                        .map(|ok| !ok)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        has_unhealthy_service || has_failed_probe
    }
}

impl Searchable for FleetEntry {
    fn matches_search(&self, query: &str) -> bool {
        self.cluster_name.to_lowercase().contains(query)
            || self.hostname.to_lowercase().contains(query)
            || self.instance_id.to_lowercase().contains(query)
            || self.version.to_lowercase().contains(query)
            || self.environment.to_lowercase().contains(query)
    }
}

#[component]
pub fn FleetDashboard(stage_id: Option<String>) -> Element {
    let mut data: Signal<Option<Result<Vec<FleetEntry>, String>>> = use_signal(|| None);
    let mut stage_label: Signal<Option<String>> = use_signal(|| None);
    let mut last_refreshed = use_signal(|| None::<DateTime<Utc>>);
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);
    let sort = use_signal(|| ("last_seen".to_string(), false));
    let mut unhealthy_only = use_signal(|| false);
    let filter_stage = stage_id.clone();

    // Fetch immediately, then every 5 seconds. use_hook + spawn so it runs
    // exactly once and signal writes don't restart the loop.
    use_hook(move || {
        let stage_for_loop = filter_stage.clone();
        spawn(async move {
            loop {
                match get_fleet_status(stage_for_loop.clone()).await {
                    Ok(result) => {
                        stage_label.set(result.stage_label);
                        data.set(Some(Ok(result.entries)));
                    }
                    Err(e) => {
                        if data.read().is_none() || data.read().as_ref().is_some_and(|r| r.is_err())
                        {
                            data.set(Some(Err(e.to_string())));
                        }
                    }
                }
                last_refreshed.set(Some(Utc::now()));
                let _ = document::eval("setTimeout(() => dioxus.send(null), 5000)")
                    .recv::<serde_json::Value>()
                    .await;
            }
        })
    });

    let refresh_ago = match *last_refreshed.read() {
        Some(ts) => {
            let secs = Utc::now().signed_duration_since(ts).num_seconds();
            if secs < 5 {
                t!("fleet-just-now")
            } else {
                format!("{secs}s ago")
            }
        }
        None => "...".to_string(),
    };

    // Fetch commit counts async for all unique SHAs in the current data.
    // Both mac-mgmt and nixpkgs counts are fetched in a single resource
    // to avoid double-borrowing the data signal.
    type CountPair = (std::collections::HashMap<String, u64>, std::collections::HashMap<String, u64>);
    let all_counts = use_resource(move || async move {
        let entries = data.read();
        let (git_shas, nix_shas): (Vec<String>, Vec<String>) = entries
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|entries| {
                let git: Vec<String> = entries
                    .iter()
                    .filter_map(|e| e.git_sha.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                let nix: Vec<String> = entries
                    .iter()
                    .filter_map(|e| e.nixpkgs_commit.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                (git, nix)
            })
            .unwrap_or_default();
        let git_counts = if git_shas.is_empty() {
            std::collections::HashMap::new()
        } else {
            get_commit_counts(git_shas).await.unwrap_or_default()
        };
        let nix_counts = if nix_shas.is_empty() {
            std::collections::HashMap::new()
        } else {
            get_nixpkgs_commit_counts(nix_shas).await.unwrap_or_default()
        };
        (git_counts, nix_counts) as CountPair
    });
    let counts_read = all_counts.read();
    let (counts, nix_counts) = counts_read.as_ref()
        .map(|(g, n)| (Some(g), Some(n)))
        .unwrap_or((None, None));

    let snapshot = data.read();
    match snapshot.as_ref() {
        Some(Ok(entries)) => {
            let entries_clone = entries.clone();
            let mut filtered: Vec<FleetEntry> = {
                let q = search.read().to_lowercase();
                let show_unhealthy = *unhealthy_only.read();
                let mut result: Vec<FleetEntry> = if q.is_empty() {
                    entries_clone.clone()
                } else {
                    entries_clone
                        .iter()
                        .filter(|e| e.matches_search(&q))
                        .cloned()
                        .collect()
                };
                if show_unhealthy {
                    result.retain(|e| e.has_unhealthy());
                }
                result
            };

            {
                let (key, asc) = sort.read().clone();
                // Online threshold matches the Status column's 5-minute rule.
                let now = Utc::now();
                let is_online =
                    |e: &FleetEntry| now.signed_duration_since(e.reported_at).num_seconds() < 300;
                filtered.sort_by(|a, b| {
                    let ord = match key.as_str() {
                        "cluster" => a
                            .cluster_name
                            .to_lowercase()
                            .cmp(&b.cluster_name.to_lowercase()),
                        "hostname" => a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()),
                        "env" => a
                            .environment
                            .to_lowercase()
                            .cmp(&b.environment.to_lowercase()),
                        "version" => {
                            // Parse semver components for numeric ordering,
                            // then tie-break on git commit count (higher = newer).
                            let parse_semver = |v: &str| -> (u64, u64, u64) {
                                let mut parts = v.split('.');
                                let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                                let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                                let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                                (major, minor, patch)
                            };
                            let av = parse_semver(&a.version);
                            let bv = parse_semver(&b.version);
                            av.cmp(&bv).then_with(|| {
                                let ac = a.git_sha.as_ref()
                                    .and_then(|sha| counts.and_then(|m| m.get(sha)))
                                    .copied()
                                    .unwrap_or(0);
                                let bc = b.git_sha.as_ref()
                                    .and_then(|sha| counts.and_then(|m| m.get(sha)))
                                    .copied()
                                    .unwrap_or(0);
                                ac.cmp(&bc)
                            })
                        }
                        _ => {
                            // Default "last seen" sort: group online hosts first,
                            // alphabetical (hostname, then cluster name) within them
                            // so the list stays stable while heartbeats tick. Offline
                            // hosts fall through to the standard reported_at order
                            // so the most-recently-seen offline sits at the top of
                            // its group.
                            let a_on = is_online(a);
                            let b_on = is_online(b);
                            match (a_on, b_on) {
                                (true, false) => std::cmp::Ordering::Less,
                                (false, true) => std::cmp::Ordering::Greater,
                                (true, true) => {
                                    let a_name = if a.hostname.is_empty() {
                                        &a.cluster_name
                                    } else {
                                        &a.hostname
                                    };
                                    let b_name = if b.hostname.is_empty() {
                                        &b.cluster_name
                                    } else {
                                        &b.hostname
                                    };
                                    a_name.to_lowercase().cmp(&b_name.to_lowercase())
                                }
                                (false, false) => a.reported_at.cmp(&b.reported_at),
                            }
                        }
                    };
                    // The online-first ordering is intrinsic — don't flip it on
                    // asc. Only the within-group tie-breaker follows the arrow.
                    if key != "last_seen" && !asc {
                        ord.reverse()
                    } else if key == "last_seen" {
                        // Preserve the online-first grouping regardless of arrow
                        // direction: the arrow only flips the tie-breakers.
                        match (is_online(a), is_online(b)) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => {
                                if asc {
                                    ord
                                } else {
                                    ord.reverse()
                                }
                            }
                        }
                    } else {
                        ord
                    }
                });
            }

            let total = entries.len();
            let filtered_count = filtered.len();
            let limit_val = *limit.read();
            let shown = filtered_count.min(limit_val);

            rsx! {
                div { class: "flex items-center justify-between mb-4",
                    h2 { class: "text-2xl font-bold", {t!("fleet-title")} }
                    span { class: "text-xs text-gray-400 dark:text-gray-500",
                        {t!("fleet-last-refreshed", time: refresh_ago)}
                    }
                }
                // Stage filter banner — visible when the route was loaded
                // with a stage_id and the server resolved it to a label.
                if let Some(label) = stage_label.read().clone() {
                    div { class: "flex items-center justify-between gap-2 mb-3 px-3 py-2 rounded bg-blue-50 dark:bg-blue-950 border border-blue-200 dark:border-blue-800",
                        div { class: "text-sm text-blue-800 dark:text-blue-200",
                            span { class: "font-medium", {t!("fleet-filtered-by")} }
                            "{label}"
                        }
                        Link {
                            to: Route::FleetDashboard { stage_id: None },
                            class: "text-xs text-blue-700 dark:text-blue-300 hover:underline",
                            {t!("fleet-clear-filter")}
                        }
                    }
                }
                if entries.is_empty() {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", {t!("fleet-no-daemons")} }
                } else {
                    TableToolbar { search, limit, total, filtered: filtered_count, shown }
                    div { class: "flex items-center gap-2 mb-3",
                        {
                            let active = *unhealthy_only.read();
                            rsx! {
                                button {
                                    class: if active {
                                        "px-3 py-1.5 rounded text-xs font-medium bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200 border border-red-300 dark:border-red-700"
                                    } else {
                                        "px-3 py-1.5 rounded text-xs font-medium bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 border border-gray-300 dark:border-gray-600 hover:bg-gray-200 dark:hover:bg-gray-600"
                                    },
                                    onclick: move |_| unhealthy_only.set(!active),
                                    {t!("fleet-unhealthy-only")}
                                }
                            }
                        }
                    }
                    div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 overflow-hidden",
                        table { class: "min-w-full divide-y divide-gray-200 dark:divide-gray-700",
                            thead { class: "bg-gray-50 dark:bg-gray-700",
                                tr {
                                    SortableTh { label: t!("fleet-col-cluster"), sort_key: "cluster".to_string(), sort }
                                    SortableTh { label: t!("fleet-col-hostname"), sort_key: "hostname".to_string(), sort }
                                    SortableTh { label: t!("fleet-col-env"), sort_key: "env".to_string(), sort }
                                    SortableTh { label: t!("version"), sort_key: "version".to_string(), sort }
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", {t!("cluster-list-col-nixpkgs")} }
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", {t!("fleet-col-status")} }
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", {t!("fleet-col-load")} }
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", {t!("fleet-col-services")} }
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", {t!("fleet-col-probes")} }
                                    th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", {t!("fleet-col-tunnels")} }
                                    SortableTh { label: t!("fleet-col-last-seen"), sort_key: "last_seen".to_string(), sort }
                                    th { class: "px-6 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "" }
                                }
                            }
                            tbody { class: "bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700",
                                for entry in filtered.into_iter().take(limit_val) {
                                    {
                                        let now = Utc::now();
                                        let age = now.signed_duration_since(entry.reported_at);
                                        let is_online = age.num_seconds() < 300;
                                        let status_class = if is_online { "text-green-600 dark:text-green-400 font-semibold" } else { "text-red-600 dark:text-red-400 font-semibold" };
                                        let status_text = if is_online { t!("fleet-online") } else { t!("fleet-offline") };
                                        let last_seen = if age.num_seconds() < 60 {
                                            t!("fleet-just-now")
                                        } else if age.num_minutes() < 60 {
                                            format!("{}m ago", age.num_minutes())
                                        } else if age.num_hours() < 24 {
                                            format!("{}h ago", age.num_hours())
                                        } else {
                                            entry.reported_at.format("%Y-%m-%d %H:%M").to_string()
                                        };

                                        // Service badges reflect the local daemon's health flag only — the
                                        // Services column answers "does the daemon think the service is up?".
                                        // Probe data goes in its own column below so the two signals don't
                                        // get conflated when only one of them is failing.
                                        let mut services_badges: Vec<(String, String, String)> = entry.services
                                            .as_array()
                                            .map(|arr| {
                                                arr.iter().map(|s| {
                                                    let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                                    let healthy = s.get("healthy").and_then(|v| v.as_bool()).unwrap_or(false);
                                                    let (cls, title) = if healthy {
                                                        (
                                                            "bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200".to_string(),
                                                            t!("fleet-healthy"),
                                                        )
                                                    } else {
                                                        (
                                                            "bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200".to_string(),
                                                            t!("fleet-unhealthy"),
                                                        )
                                                    };
                                                    (name, cls, title)
                                                }).collect()
                                            })
                                            .unwrap_or_default();
                                        services_badges.sort_by(|a, b| a.0.cmp(&b.0));

                                        // Probe badges: one per service present in services_extended, colored
                                        // by the latest probe outcome. Tooltip carries kind + age so operators
                                        // can tell a just-ran green from a stale-but-previously-ok green.
                                        let mut probe_badges: Vec<(String, String, String)> = entry.services_extended
                                            .as_ref()
                                            .and_then(|v| v.as_array())
                                            .map(|arr| arr.iter().filter_map(|s| {
                                                let name = s.get("name").and_then(|v| v.as_str())?.to_string();
                                                let ok = s.get("last_probe_ok").and_then(|v| v.as_bool());
                                                let kind = s.get("last_probe_kind").and_then(|v| v.as_str()).unwrap_or("probe").to_string();
                                                let at = s.get("last_probe_at").and_then(|v| v.as_i64());
                                                let dur = s.get("last_probe_duration_ms").and_then(|v| v.as_u64());
                                                let age_txt = at.map(|ts| {
                                                    let secs = (Utc::now().timestamp() - ts).max(0);
                                                    if secs < 60 {
                                                        format!("{secs}s ago")
                                                    } else if secs < 3600 {
                                                        format!("{}m ago", secs / 60)
                                                    } else {
                                                        format!("{}h ago", secs / 3600)
                                                    }
                                                }).unwrap_or_else(|| "never".into());
                                                let dur_txt = dur.map(|d| format!(", {d}ms")).unwrap_or_default();
                                                let (cls, label, ok_text) = match ok {
                                                    Some(true) => (
                                                        "bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200",
                                                        t!("fleet-ok"),
                                                        t!("fleet-ok"),
                                                    ),
                                                    Some(false) => (
                                                        "bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200",
                                                        t!("fleet-fail"),
                                                        t!("fleet-fail"),
                                                    ),
                                                    None => (
                                                        "bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300",
                                                        t!("fleet-pending"),
                                                        t!("fleet-no-result"),
                                                    ),
                                                };
                                                let _ = label;
                                                let title = format!("{kind} {ok_text} · {age_txt}{dur_txt}");
                                                Some((name, cls.to_string(), title))
                                            }).collect())
                                            .unwrap_or_default();
                                        probe_badges.sort_by(|a, b| a.0.cmp(&b.0));

                                        // Compact load cell: "1.2 · 78% · nominal"
                                        let load_cell: Option<String> = entry.sample.as_ref().map(|s| {
                                            let cpu = s.get("cpu_load_1m").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                            let mem_used = s.get("mem_used_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                                            let mem_total = s.get("mem_total_bytes").and_then(|v| v.as_u64()).unwrap_or(1);
                                            let mem_pct = (mem_used * 100 / mem_total.max(1)).min(100);
                                            let thermal = s.get("thermal_state").and_then(|v| v.as_str()).unwrap_or("—");
                                            format!("{cpu:.2} · {mem_pct}% · {thermal}")
                                        });

                                        // Compact GPU badge set, one per GPU: "nvidia 72% · 64°C".
                                        let gpu_cells: Vec<(String, String)> = entry.sample.as_ref()
                                            .and_then(|s| s.get("gpus"))
                                            .and_then(|v| v.as_array())
                                            .map(|arr| {
                                                arr.iter().map(|g| {
                                                    let util = g.get("utilization_pct").and_then(|v| v.as_u64());
                                                    let temp = g.get("temperature_c").and_then(|v| v.as_i64());
                                                    let mut parts: Vec<String> = Vec::new();
                                                    if let Some(u) = util { parts.push(format!("{u}%")); }
                                                    if let Some(t) = temp { parts.push(format!("{t}°C")); }
                                                    let label = if parts.is_empty() { t!("fleet-idle") } else { parts.join(" · ") };
                                                    let idx = g.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
                                                    let cls = match util {
                                                        Some(u) if u >= 85 => "bg-orange-100 dark:bg-orange-900 text-orange-800 dark:text-orange-200",
                                                        Some(_) => "bg-purple-100 dark:bg-purple-900 text-purple-800 dark:text-purple-200",
                                                        None => "bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300",
                                                    };
                                                    (format!("gpu{idx} {label}"), cls.to_string())
                                                }).collect()
                                            })
                                            .unwrap_or_default();

                                        rsx! {
                                            tr {
                                                td { class: "px-6 py-4 text-sm",
                                                    Link {
                                                        to: Route::ClusterDetail { id: entry.cluster_id.clone() },
                                                        class: "text-blue-600 dark:text-blue-400 hover:underline",
                                                        "{entry.cluster_name}"
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm",
                                                    Link {
                                                        to: Route::FleetDetail { instance_id: entry.instance_id.clone() },
                                                        class: "text-blue-600 dark:text-blue-400 hover:underline",
                                                        if entry.hostname.is_empty() {
                                                            span { class: "font-mono text-xs", "{entry.instance_id}" }
                                                        } else {
                                                            span { "{entry.hostname}" }
                                                        }
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm",
                                                    if !entry.environment.is_empty() {
                                                        span { class: "px-2 py-0.5 rounded text-xs font-medium bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-200",
                                                            "{entry.environment}"
                                                        }
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm",
                                                    div { "{entry.version}" }
                                                    if let Some(sha) = &entry.git_sha {
                                                        {
                                                            let short: String = sha.chars().take(12).collect();
                                                            let url = format!("https://git.plan.ai/plan-ai/mac-mgmt/-/commit/{sha}");
                                                            let count_label = counts.as_ref()
                                                                .and_then(|m| m.get(sha))
                                                                .map(|n| format!(" #{n}"))
                                                                .unwrap_or_default();
                                                            rsx! {
                                                                a {
                                                                    class: "text-xs font-mono text-gray-500 dark:text-gray-400 hover:text-blue-600 dark:hover:text-blue-400",
                                                                    href: "{url}",
                                                                    target: "_blank",
                                                                    title: "{sha}",
                                                                    "{short}{count_label}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm",
                                                    if let Some(nix_sha) = &entry.nixpkgs_commit {
                                                        {
                                                            let short: String = nix_sha.chars().take(12).collect();
                                                            let url = format!("https://git.plan.ai/plan-ai/nixpkgs/-/commit/{nix_sha}");
                                                            let count_label = nix_counts.as_ref()
                                                                .and_then(|m| m.get(nix_sha))
                                                                .map(|n| format!(" #{n}"))
                                                                .unwrap_or_default();
                                                            rsx! {
                                                                a {
                                                                    class: "text-xs font-mono text-gray-500 dark:text-gray-400 hover:text-blue-600 dark:hover:text-blue-400",
                                                                    href: "{url}",
                                                                    target: "_blank",
                                                                    title: "{nix_sha}",
                                                                    "{short}{count_label}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm {status_class}", "{status_text}" }
                                                td { class: "px-6 py-4 text-xs font-mono text-gray-600 dark:text-gray-300",
                                                    if let Some(lc) = &load_cell {
                                                        span { "{lc}" }
                                                    } else {
                                                        span { class: "text-gray-400 dark:text-gray-500", {t!("em-dash")} }
                                                    }
                                                    if !gpu_cells.is_empty() {
                                                        div { class: "flex gap-1 flex-wrap mt-1",
                                                            for (label, cls) in &gpu_cells {
                                                                span { class: "inline-block px-1.5 py-0.5 rounded text-xs font-medium {cls}",
                                                                    "{label}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm",
                                                    div { class: "flex gap-1 flex-wrap",
                                                        for (name, badge_class, title) in &services_badges {
                                                            span {
                                                                class: "inline-block px-2 py-0.5 rounded text-xs font-medium {badge_class}",
                                                                title: "{title}",
                                                                "{name}"
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm",
                                                    if probe_badges.is_empty() {
                                                        span { class: "text-gray-400 dark:text-gray-500", {t!("em-dash")} }
                                                    } else {
                                                        div { class: "flex gap-1 flex-wrap",
                                                            for (name, badge_class, title) in &probe_badges {
                                                                span {
                                                                    class: "inline-block px-2 py-0.5 rounded text-xs font-medium {badge_class}",
                                                                    title: "{title}",
                                                                    "{name}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm",
                                                    {
                                                        let proxy_url = entry.relay_proxy_url.clone();
                                                        let tunnel_names: Vec<String> = entry.tunnels
                                                            .as_array()
                                                            .map(|arr| arr.iter().filter_map(|t| {
                                                                t.get("name").and_then(|v| v.as_str()).map(String::from)
                                                            }).collect())
                                                            .unwrap_or_default();

                                                        rsx! {
                                                            if is_online {
                                                                if let Some(ref purl) = proxy_url {
                                                                    div { class: "flex gap-1 flex-wrap",
                                                                        for tname in &tunnel_names {
                                                                            {
                                                                                let iid = entry.instance_id.chars().take(12).collect::<String>();
                                                                                let tn = tname.clone();
                                                                                let pu = purl.clone();
                                                                                rsx! {
                                                                                    button {
                                                                                        class: "inline-block px-2 py-0.5 rounded text-xs font-medium bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200 hover:bg-blue-200 dark:hover:bg-blue-800 cursor-pointer",
                                                                                        title: "Open tunnel in new tab",
                                                                                        onclick: move |_| {
                                                                                            let iid = iid.clone();
                                                                                            let tn = tn.clone();
                                                                                            let pu = pu.clone();
                                                                                            async move {
                                                                                                match create_proxy_token().await {
                                                                                                    Ok(result) => {
                                                                                                        let prefix = format!("{iid}-{tn}");
                                                                                                        let url = super::fleet_detail::build_tunnel_url(&pu, &prefix, &result.proxy_token);
                                                                                                        // Open in new tab
                                                                                                        let _ = document::eval(&format!(
                                                                                                            "window.open('{}', '_blank')",
                                                                                                            url
                                                                                                        ));
                                                                                                    }
                                                                                                    Err(e) => {
                                                                                                        tracing::error!("failed to create proxy token: {e}");
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                        },
                                                                                        "{tn}"
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                } else if !tunnel_names.is_empty() {
                                                                    div { class: "flex gap-1 flex-wrap",
                                                                        for tname in &tunnel_names {
                                                                            span {
                                                                                class: "inline-block px-2 py-0.5 rounded text-xs font-medium bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300",
                                                                                "{tname}"
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "px-6 py-4 text-sm text-gray-500 dark:text-gray-400", "{last_seen}" }
                                                td { class: "px-6 py-4 text-right",
                                                    if age.num_hours() >= 24 {
                                                        button {
                                                            class: "text-red-600 dark:text-red-400 text-xs hover:underline",
                                                            title: "Delete this stale instance from the dashboard. Only available after 24h of silence.",
                                                            onclick: {
                                                                let iid = entry.instance_id.clone();
                                                                move |_| {
                                                                    let iid = iid.clone();
                                                                    async move {
                                                                        match delete_stale_instance(iid).await {
                                                                            Ok(()) => {
                                                                                // The 5s polling loop will pick up the change.
                                                                            }
                                                                            Err(e) => {
                                                                                let _ = document::eval(&format!(
                                                                                    "alert('Delete failed: {}')",
                                                                                    e.to_string().replace('\'', "\\'")
                                                                                ));
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            },
                                                            {t!("delete")}
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
                }
            }
        }
        Some(Err(e)) => rsx! {
            p { class: "text-red-600 dark:text-red-400 text-sm", {t!("error-message", message: e.to_string())} }
        },
        None => rsx! {
            p { class: "text-gray-500 dark:text-gray-400 text-sm", {t!("loading")} }
        },
    }
}
