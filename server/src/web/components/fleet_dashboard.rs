use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::api_mcp::endpoints::fleet::{
    DeleteStaleInstanceInput, FleetEntry, FleetStatusInput, delete_stale_instance,
    get_fleet_status,
};
use crate::web::app::Route;
use crate::web::components::table_utils::{Searchable, SortableTh, TableToolbar};
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    ChartColor, Dot, ErrorText, HelpText, KpiCard, Mono, PageHero, Pill, PillVariant, page_window,
};
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

#[server]
async fn get_commit_counts(
    shas: Vec<String>,
) -> Result<std::collections::HashMap<String, u64>, ServerFnError> {
    let set: std::collections::HashSet<String> = shas.into_iter().collect();
    Ok(crate::commit_count::mac_mgmt_commit_counts(&set).await)
}

#[server]
async fn get_nixpkgs_commit_counts(
    shas: Vec<String>,
) -> Result<std::collections::HashMap<String, u64>, ServerFnError> {
    let set: std::collections::HashSet<String> = shas.into_iter().collect();
    Ok(crate::commit_count::nixpkgs_commit_counts(&set).await)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTokenResult {
    pub proxy_token: String,
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
    use_topbar(t!("fleet-title"), None);

    let mut data: Signal<Option<Result<Vec<FleetEntry>, String>>> = use_signal(|| None);
    let mut stage_label: Signal<Option<String>> = use_signal(|| None);
    let mut last_refreshed = use_signal(|| None::<DateTime<Utc>>);
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);
    let page = use_signal(|| 0usize);
    let sort = use_signal(|| ("last_seen".to_string(), false));
    let mut unhealthy_only = use_signal(|| false);
    let filter_stage = stage_id.clone();

    // Fetch immediately, then every 5 seconds. use_hook + spawn so it runs
    // exactly once and signal writes don't restart the loop.
    use_hook(move || {
        let stage_for_loop = filter_stage.clone();
        spawn(async move {
            // Route-string stage id parsed once, up front — a bad id can
            // never become valid, so surface the error instead of looping.
            let stage_uuid: Option<uuid::Uuid> = match stage_for_loop.as_deref().map(str::parse) {
                Some(Ok(id)) => Some(id),
                Some(Err(e)) => {
                    data.set(Some(Err(format!("invalid stage_id: {e}"))));
                    return;
                }
                None => None,
            };
            loop {
                match get_fleet_status(FleetStatusInput {
                    stage_id: stage_uuid,
                })
                .await
                {
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
    type CountPair = (
        std::collections::HashMap<String, u64>,
        std::collections::HashMap<String, u64>,
    );
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
            get_nixpkgs_commit_counts(nix_shas)
                .await
                .unwrap_or_default()
        };
        (git_counts, nix_counts) as CountPair
    });
    let counts_read = all_counts.read();
    let (counts, nix_counts) = counts_read
        .as_ref()
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
                                let ac = a
                                    .git_sha
                                    .as_ref()
                                    .and_then(|sha| counts.and_then(|m| m.get(sha)))
                                    .copied()
                                    .unwrap_or(0);
                                let bc = b
                                    .git_sha
                                    .as_ref()
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
            let (start, shown) = page_window(*page.read(), limit_val, filtered_count);

            // ── KPI summary computed from the current heartbeat snapshot.
            // We deliberately compute these synchronously from `entries`
            // rather than firing a separate query: the dashboard already
            // re-fetches heartbeats every 5s so these stats stay live for
            // free.
            let now = Utc::now();
            let total_instances = entries.len();
            let online_instances = entries
                .iter()
                .filter(|e| now.signed_duration_since(e.reported_at).num_seconds() < 300)
                .count();
            let total_clusters: usize = entries
                .iter()
                .map(|e| e.cluster_id.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len();

            // Healthy services percentage across every heartbeat's services
            // array. Cheaper than a SQL aggregate and mirrors the table
            // visually: the same green/red state drives both.
            let (svc_healthy, svc_total): (usize, usize) = entries
                .iter()
                .filter_map(|e| e.services.as_array())
                .flat_map(|arr| arr.iter())
                .fold((0usize, 0usize), |(h, t), s| {
                    let healthy = s.get("healthy").and_then(|v| v.as_bool()).unwrap_or(false);
                    (h + usize::from(healthy), t + 1)
                });
            let svc_pct: f64 = if svc_total == 0 {
                0.0
            } else {
                (svc_healthy as f64) / (svc_total as f64) * 100.0
            };

            // Failing probes — a separate signal from "service healthy"
            // because a probe failure means the verification layer noticed
            // something the daemon's self-report didn't.
            let failing_probes: usize = entries
                .iter()
                .filter_map(|e| e.services_extended.as_ref())
                .filter_map(|v| v.as_array())
                .flat_map(|arr| arr.iter())
                .filter(|s| {
                    s.get("last_probe_ok")
                        .and_then(|v| v.as_bool())
                        .map(|ok| !ok)
                        .unwrap_or(false)
                })
                .count();

            rsx! {
                // ── Page hero ────────────────────────────────────────
                // Composes display title + right-side live indicator.
                // The kicker slot was dropped now that breadcrumbs sit
                // above every page hero.
                PageHero {
                    title: rsx! { {t!("nav-fleet")} },
                    right: rsx! {
                        div { class: "flex items-center gap-2 text-fg-muted text-xs",
                            Dot { variant: PillVariant::Ok }
                            span { "live" }
                        }
                        span { class: "text-fg-faint text-xs font-mono",
                            {t!("fleet-last-refreshed", time: refresh_ago)}
                        }
                    },
                    class: "mb-5",
                }

                // ── KPI strip ────────────────────────────────────────
                // Responsive: 1 col on phones, 2 on tablets, 4 on desktop.
                // Sparklines are deliberately omitted — we don't have a
                // time-series feed yet; passing empty `data` makes
                // KpiCard skip the chart cleanly.
                div { class: "grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-4 mb-5",
                    KpiCard {
                        label: t!("fleet-kpi-online"),
                        value: format!("{online_instances}"),
                        delta: Some(format!("/ {total_instances}")),
                        delta_kind: ChartColor::Muted,
                        color: ChartColor::Brand,
                    }
                    KpiCard {
                        label: t!("fleet-kpi-services-healthy"),
                        value: format!("{:.1}%", svc_pct),
                        delta: Some(format!("{svc_healthy}/{svc_total}")),
                        delta_kind: ChartColor::Muted,
                        color: ChartColor::Ok,
                    }
                    KpiCard {
                        label: t!("fleet-kpi-clusters"),
                        value: format!("{total_clusters}"),
                        color: ChartColor::Info,
                        to: Route::ClusterList {},
                    }
                    KpiCard {
                        label: t!("fleet-kpi-failing-probes"),
                        value: format!("{failing_probes}"),
                        delta_kind: if failing_probes == 0 { ChartColor::Ok } else { ChartColor::Bad },
                        color: ChartColor::Bad,
                    }
                }
                // Stage filter banner — visible when the route was loaded
                // with a stage_id and the server resolved it to a label.
                if let Some(label) = stage_label.read().clone() {
                    div { class: "flex items-center justify-between gap-2 mb-3 px-3 py-2 rounded bg-info-soft border border-info",
                        div { class: "text-sm text-info",
                            span { class: "font-medium", {t!("fleet-filtered-by")} }
                            "{label}"
                        }
                        Link { to: Route::FleetDashboard { stage_id: None }, class: "text-xs link",
                            {t!("fleet-clear-filter")}
                        }
                    }
                }
                if entries.is_empty() {
                    HelpText { {t!("fleet-no-daemons")} }
                } else {
                    TableToolbar { search, limit, page, total, filtered: filtered_count, shown }
                    div { class: "flex items-center gap-2 mb-3",
                        {
                            let active = *unhealthy_only.read();
                            rsx! {
                                button {
                                    class: if active {
                                        "btn btn-md btn-danger-soft border border-danger"
                                    } else {
                                        "btn btn-md btn-secondary"
                                    },
                                    onclick: move |_| unhealthy_only.set(!active),
                                    {t!("fleet-unhealthy-only")}
                                }
                            }
                        }
                    }
                    div { class: "card",
                        div { class: "overflow-x-auto",
                            table { class: "table",
                            thead { class: "thead",
                                tr {
                                    SortableTh { label: t!("fleet-col-cluster"), sort_key: "cluster".to_string(), sort }
                                    SortableTh { label: t!("fleet-col-hostname"), sort_key: "hostname".to_string(), sort }
                                    SortableTh { label: t!("fleet-col-env"), sort_key: "env".to_string(), sort }
                                    SortableTh { label: t!("version"), sort_key: "version".to_string(), sort }
                                    th { class: "th", {t!("cluster-list-col-nixpkgs")} }
                                    th { class: "th", {t!("fleet-col-status")} }
                                    th { class: "th", {t!("fleet-col-load")} }
                                    th { class: "th", {t!("fleet-col-services")} }
                                    th { class: "th", {t!("fleet-col-probes")} }
                                    th { class: "th", {t!("fleet-col-tunnels")} }
                                    SortableTh { label: t!("fleet-col-last-seen"), sort_key: "last_seen".to_string(), sort }
                                    th { class: "th text-right", "" }
                                }
                            }
                            tbody { class: "tbody",
                                for entry in filtered.into_iter().skip(start).take(limit_val) {
                                    {
                                        let now = Utc::now();
                                        let age = now.signed_duration_since(entry.reported_at);
                                        let is_online = age.num_seconds() < 300;
                                        // Status pill: ok pill+dot when online, bad pill+dot otherwise.
                                        // (`status_class` / `status_text` removed — replaced by the Pill below.)
                                        let status_variant = if is_online { PillVariant::Ok } else { PillVariant::Bad };
                                        let status_text = if is_online { t!("fleet-online") } else { t!("fleet-offline") };
                                        // Env variant: production stands out (brand-tinted accent),
                                        // staging is warn-tinted, anything else (dev / unset) is muted.
                                        let env_variant = match entry.environment.as_str() {
                                            "production" => PillVariant::Accent,
                                            "staging"    => PillVariant::Warn,
                                            _            => PillVariant::Muted,
                                        };
                                        let last_seen = if age.num_seconds() < 60 {
                                            t!("fleet-just-now")
                                        } else if age.num_minutes() < 60 {
                                            format!("{}m ago", age.num_minutes())
                                        } else if age.num_hours() < 24 {
                                            format!("{}h ago", age.num_hours())
                                        } else {
                                            entry.reported_at.format("%Y-%m-%d %H:%M").to_string()
                                        };

                                        // Service pills reflect the local daemon's health flag only — the
                                        // Services column answers "does the daemon think the service is up?".
                                        // Probe data goes in its own column below so the two signals don't
                                        // get conflated when only one of them is failing.
                                        //
                                        // Per design language: healthy services are muted/quiet (a fleet of
                                        // green pills is loud and useless); only unhealthy services pop in
                                        // bad-red. The tooltip carries the full status word.
                                        let mut services_pills: Vec<(String, PillVariant, String)> = entry.services
                                            .as_array()
                                            .map(|arr| {
                                                arr.iter().map(|s| {
                                                    let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                                    let healthy = s.get("healthy").and_then(|v| v.as_bool()).unwrap_or(false);
                                                    let (variant, title) = if healthy {
                                                        (PillVariant::Muted, t!("fleet-healthy").to_string())
                                                    } else {
                                                        (PillVariant::Bad, t!("fleet-unhealthy").to_string())
                                                    };
                                                    (name, variant, title)
                                                }).collect()
                                            })
                                            .unwrap_or_default();
                                        services_pills.sort_by(|a, b| a.0.cmp(&b.0));

                                        // Probe pills: one per service in services_extended, colored by the
                                        // latest probe outcome. Same loudness rule as Services — passing
                                        // probes are muted, failures pop in bad-red. Tooltip carries kind +
                                        // age + duration so operators can tell a just-ran green from a
                                        // stale-but-previously-ok green.
                                        let mut probe_pills: Vec<(String, PillVariant, String)> = entry.services_extended
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
                                                let (variant, ok_text) = match ok {
                                                    Some(true)  => (PillVariant::Muted, t!("fleet-ok").to_string()),
                                                    Some(false) => (PillVariant::Bad,   t!("fleet-fail").to_string()),
                                                    None        => (PillVariant::Muted, t!("fleet-no-result").to_string()),
                                                };
                                                let title = format!("{kind} {ok_text} · {age_txt}{dur_txt}");
                                                Some((name, variant, title))
                                            }).collect())
                                            .unwrap_or_default();
                                        probe_pills.sort_by(|a, b| a.0.cmp(&b.0));

                                        // Compact load cell: "1.2 · 78% · nominal"
                                        let load_cell: Option<String> = entry.sample.as_ref().map(|s| {
                                            let cpu = s.get("cpu_load_1m").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                            let mem_used = s.get("mem_used_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                                            let mem_total = s.get("mem_total_bytes").and_then(|v| v.as_u64()).unwrap_or(1);
                                            let mem_pct = (mem_used * 100 / mem_total.max(1)).min(100);
                                            let thermal = s.get("thermal_state").and_then(|v| v.as_str()).unwrap_or("—");
                                            format!("{cpu:.2} · {mem_pct}% · {thermal}")
                                        });

                                        // Compact GPU pill set, one per GPU: "gpu0 72% · 64°C".
                                        // Hot GPUs (≥85% utilisation) render in warn-yellow; idle in muted.
                                        let gpu_cells: Vec<(String, PillVariant)> = entry.sample.as_ref()
                                            .and_then(|s| s.get("gpus"))
                                            .and_then(|v| v.as_array())
                                            .map(|arr| {
                                                arr.iter().map(|g| {
                                                    let util = g.get("utilization_pct").and_then(|v| v.as_u64());
                                                    let temp = g.get("temperature_c").and_then(|v| v.as_i64());
                                                    let mut parts: Vec<String> = Vec::new();
                                                    if let Some(u) = util { parts.push(format!("{u}%")); }
                                                    if let Some(t) = temp { parts.push(format!("{t}°C")); }
                                                    let label = if parts.is_empty() { t!("fleet-idle").to_string() } else { parts.join(" · ") };
                                                    let idx = g.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
                                                    let variant = match util {
                                                        Some(u) if u >= 85 => PillVariant::Warn,
                                                        Some(_)            => PillVariant::Accent,
                                                        None               => PillVariant::Muted,
                                                    };
                                                    (format!("gpu{idx} {label}"), variant)
                                                }).collect()
                                            })
                                            .unwrap_or_default();

                                        rsx! {
                                            tr {
                                                // Cluster name in brand orange (principle: cluster names are the
                                                // page's primary navigation surface — they get the only colored link).
                                                td { class: "td text-sm",
                                                    Link {
                                                        to: Route::ClusterDetail { id: entry.cluster_id.clone() },
                                                        class: "text-brand font-medium hover:underline",
                                                        "{entry.cluster_name}"
                                                    }
                                                }
                                                // Hostname is a "fact" → mono. Empty hostnames fall back to a
                                                // truncated mono instance_id so each row still has something
                                                // identifying.
                                                td { class: "td",
                                                    Link {
                                                        to: Route::FleetDetail { instance_id: entry.instance_id.clone() },
                                                        class: "text-fg hover:text-brand transition-colors",
                                                        if entry.hostname.is_empty() {
                                                            Mono { class: "text-xs", "{entry.instance_id}" }
                                                        } else {
                                                            Mono { class: "text-xs", "{entry.hostname}" }
                                                        }
                                                    }
                                                }
                                                // Env pill (production / staging / dev tones).
                                                td { class: "td text-sm",
                                                    if !entry.environment.is_empty() {
                                                        Pill { variant: env_variant, "{entry.environment}" }
                                                    }
                                                }
                                                td { class: "td",
                                                    Mono { class: "text-xs text-fg-muted", "v{entry.version}" }
                                                    if let Some(sha) = &entry.git_sha {
                                                        {
                                                            let short: String = sha.chars().take(12).collect();
                                                            let url = format!("https://git.plan.ai/plan-ai/mac-mgmt/-/commit/{sha}");
                                                            let count_label = counts.as_ref()
                                                                .and_then(|m| m.get(sha))
                                                                .map(|n| format!(" #{n}"))
                                                                .unwrap_or_default();
                                                            rsx! {
                                                                a { class: "text-xs font-mono text-fg-muted hover:text-brand",
                                                                    href: "{url}",
                                                                    target: "_blank",
                                                                    title: "{sha}",
                                                                    "{short}{count_label}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "td text-sm",
                                                    if let Some(nix_sha) = &entry.nixpkgs_commit {
                                                        {
                                                            let short: String = nix_sha.chars().take(12).collect();
                                                            let url = format!("https://git.plan.ai/plan-ai/nixpkgs/-/commit/{nix_sha}");
                                                            let count_label = nix_counts.as_ref()
                                                                .and_then(|m| m.get(nix_sha))
                                                                .map(|n| format!(" #{n}"))
                                                                .unwrap_or_default();
                                                            rsx! {
                                                                a { class: "text-xs font-mono text-fg-muted hover:text-brand",
                                                                    href: "{url}",
                                                                    target: "_blank",
                                                                    title: "{nix_sha}",
                                                                    "{short}{count_label}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                // Status: pill with leading dot (the design's
                                                // "online" / "down" pattern — colour redundant
                                                // with the dot for accessibility).
                                                td { class: "td",
                                                    Pill { variant: status_variant,
                                                        Dot { variant: status_variant }
                                                        "{status_text}"
                                                    }
                                                }
                                                td { class: "td text-xs font-mono",
                                                    if let Some(lc) = &load_cell {
                                                        span { "{lc}" }
                                                    } else {
                                                        span { class: "text-fg-faint", {t!("em-dash")} }
                                                    }
                                                    if !gpu_cells.is_empty() {
                                                        div { class: "flex gap-1 flex-wrap mt-1",
                                                            for (label, variant) in gpu_cells.iter().cloned() {
                                                                Pill { variant, mono: true, "{label}" }
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "td",
                                                    div { class: "flex gap-1 flex-wrap",
                                                        for (name, variant, title) in services_pills.iter().cloned() {
                                                            Pill { variant, mono: true, title: title.clone(), "{name}" }
                                                        }
                                                    }
                                                }
                                                td { class: "td",
                                                    if probe_pills.is_empty() {
                                                        span { class: "text-fg-faint", {t!("em-dash")} }
                                                    } else {
                                                        div { class: "flex gap-1 flex-wrap",
                                                            for (name, variant, title) in probe_pills.iter().cloned() {
                                                                Pill { variant, mono: true, title: title.clone(), "{name}" }
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "td text-sm",
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
                                                                                    button { class: "btn btn-xs btn-info-soft",
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
                                                                                                        // Open in new tab (JS-injection-safe)
                                                                                                        if let Some(js) = super::fleet_detail::open_url_js(&url) {
                                                                                                            let _ = document::eval(&js);
                                                                                                        }
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
                                                                            span { class: "badge badge-neutral", "{tname}" }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "td-muted text-sm", "{last_seen}" }
                                                td { class: "td text-right",
                                                    if age.num_hours() >= 24 {
                                                        button { class: "link-danger text-xs",
                                                            title: "Delete this stale instance from the dashboard. Only available after 24h of silence.",
                                                            onclick: {
                                                                let iid = entry.instance_id.clone();
                                                                move |_| {
                                                                    let iid = iid.clone();
                                                                    async move {
                                                                        match delete_stale_instance(DeleteStaleInstanceInput { instance_id: iid }).await {
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
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}
