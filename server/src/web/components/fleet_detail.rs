use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use crate::web::user::current_user;

/// Per-instance extended assessment page. Reachable at /fleet/:instance_id
/// from the fleet dashboard. Surfaces the latest inventory + security posture
/// + dynamic sample + per-service probe results.
///
/// Access is gated via accessible_cluster_ids — the same rule the fleet
/// dashboard uses — and probe error_detail is only exposed to admins because
/// it can contain stack traces, response bodies, or URLs.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FleetDetailData {
    instance_id: String,
    cluster_id: String,
    cluster_name: String,
    hostname: String,
    environment: String,
    version: String,
    nixpkgs_commit: Option<String>,
    reported_at: DateTime<Utc>,
    sample: Option<serde_json::Value>,
    services_extended: Option<serde_json::Value>,
    /// Raw `services` JSON array from the heartbeat (name, healthy,
    /// upgrade_pending, busy). Surfaced as badges at the top of the page.
    #[serde(default)]
    services: Option<serde_json::Value>,
    /// Raw `tunnels` JSON array (name, port) for the relay proxy buttons.
    #[serde(default)]
    tunnels: Option<serde_json::Value>,
    /// Relay proxy hostname (e.g. "relay.plan.ai") from the heartbeat.
    #[serde(default)]
    relay_proxy_hostname: Option<String>,
    /// Full relay proxy URL (e.g. "http://localhost:7379") for building tunnel links.
    #[serde(default)]
    relay_proxy_url: Option<String>,
    /// Exposed file tunnels for remote config editing.
    #[serde(default)]
    file_tunnels: Option<serde_json::Value>,
    inventory: Option<serde_json::Value>,
    inventory_collected_at: Option<DateTime<Utc>>,
    security: Option<serde_json::Value>,
    probes: Vec<ProbeEntry>,
    viewer_is_admin: bool,
}

/// Same shape as the fleet-dashboard proxy-token flow. Local to this
/// module so the detail page doesn't reach into fleet_dashboard's
/// module-private server fn. Scoped to whatever cluster the user has
/// access to (admin = unscoped, org-scoped = first accessible).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DetailProxyTokenResult {
    proxy_token: String,
}

#[server]
async fn create_detail_proxy_token() -> Result<DetailProxyTokenResult, ServerFnError> {
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let user = current_user().await?;
    let pool = crate::server_pool()?;

    let accessible = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(6);
    let cluster_id: Option<uuid::Uuid> = accessible.as_ref().and_then(|ids| ids.first().copied());

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at) \
         VALUES ($1, $2, 'proxy', 'proxy', $3)",
    )
    .bind(cluster_id)
    .bind(&hash)
    .bind(expires_at)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(DetailProxyTokenResult { proxy_token: raw_token })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProbeEntry {
    service: String,
    kind: String,
    ok: bool,
    duration_ms: i64,
    tokens_in: Option<i32>,
    tokens_out: Option<i32>,
    first_token_ms: Option<i64>,
    model: Option<String>,
    canary_digest: Option<String>,
    error_class: Option<String>,
    error_detail: Option<String>,
    collected_at: DateTime<Utc>,
}

#[server]
async fn get_fleet_detail(instance_id: String) -> Result<FleetDetailData, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    // Step 1: resolve the latest heartbeat row for this instance and check
    // the viewer is allowed to see its cluster.
    #[derive(sqlx::FromRow)]
    struct HbRow {
        cluster_id: uuid::Uuid,
        cluster_name: String,
        hostname: String,
        environment: String,
        version: String,
        nixpkgs_commit: Option<String>,
        reported_at: DateTime<Utc>,
        sample: Option<serde_json::Value>,
        services_extended: Option<serde_json::Value>,
        services: serde_json::Value,
        tunnels: serde_json::Value,
        relay_proxy_hostname: Option<String>,
        relay_proxy_url: Option<String>,
        file_tunnels: serde_json::Value,
    }
    let hb: HbRow = sqlx::query_as(
        "SELECT c.id AS cluster_id, c.name AS cluster_name, dh.hostname, dh.environment, \
                dh.version, dh.nixpkgs_commit, dh.reported_at, dh.sample, dh.services_extended, \
                dh.services, dh.tunnels, dh.relay_proxy_hostname, dh.relay_proxy_url, dh.file_tunnels \
         FROM daemon_heartbeats dh JOIN clusters c ON c.id = dh.cluster_id \
         WHERE dh.instance_id = $1 \
         ORDER BY dh.reported_at DESC LIMIT 1",
    )
    .bind(&instance_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("instance not found"))?;

    user.require_cluster_read(&pool, hb.cluster_id).await?;

    // Step 2: latest assessment snapshot (static inventory + security).
    #[derive(sqlx::FromRow)]
    struct AssRow {
        inventory: serde_json::Value,
        security: serde_json::Value,
        collected_at: DateTime<Utc>,
    }
    let ass: Option<AssRow> = sqlx::query_as(
        "SELECT inventory, security, collected_at FROM assessments \
         WHERE instance_id = $1 ORDER BY collected_at DESC LIMIT 1",
    )
    .bind(&instance_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Step 3: latest probe result per service.
    #[derive(sqlx::FromRow)]
    struct ProbeRow {
        service: String,
        kind: String,
        ok: bool,
        duration_ms: i64,
        tokens_in: Option<i32>,
        tokens_out: Option<i32>,
        first_token_ms: Option<i64>,
        model: Option<String>,
        canary_digest: Option<String>,
        error_class: Option<String>,
        error_detail: Option<String>,
        collected_at: DateTime<Utc>,
    }
    let probe_rows: Vec<ProbeRow> = sqlx::query_as(
        "SELECT DISTINCT ON (service) \
                service, kind, ok, duration_ms, tokens_in, tokens_out, first_token_ms, \
                model, canary_digest, error_class, error_detail, collected_at \
         FROM assessment_probes \
         WHERE instance_id = $1 \
         ORDER BY service, collected_at DESC",
    )
    .bind(&instance_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let is_admin = user.is_admin;
    let probes = probe_rows
        .into_iter()
        .map(|p| ProbeEntry {
            service: p.service,
            kind: p.kind,
            ok: p.ok,
            duration_ms: p.duration_ms,
            tokens_in: p.tokens_in,
            tokens_out: p.tokens_out,
            first_token_ms: p.first_token_ms,
            model: p.model,
            canary_digest: p.canary_digest,
            error_class: p.error_class,
            // GDPR: error_detail can carry request bodies / paths — admin only.
            error_detail: if is_admin { p.error_detail } else { None },
            collected_at: p.collected_at,
        })
        .collect();

    Ok(FleetDetailData {
        instance_id,
        cluster_id: hb.cluster_id.to_string(),
        cluster_name: hb.cluster_name,
        hostname: hb.hostname,
        environment: hb.environment,
        version: hb.version,
        nixpkgs_commit: hb.nixpkgs_commit,
        reported_at: hb.reported_at,
        sample: hb.sample,
        services_extended: hb.services_extended,
        services: Some(hb.services),
        tunnels: Some(hb.tunnels),
        relay_proxy_hostname: hb.relay_proxy_hostname,
        relay_proxy_url: hb.relay_proxy_url,
        file_tunnels: Some(hb.file_tunnels),
        inventory: ass.as_ref().map(|a| a.inventory.clone()),
        inventory_collected_at: ass.as_ref().map(|a| a.collected_at),
        security: ass.as_ref().map(|a| a.security.clone()),
        probes,
        viewer_is_admin: is_admin,
    })
}

#[component]
pub fn FleetDetail(instance_id: String) -> Element {
    let iid = instance_id.clone();
    let data = use_server_future(move || {
        let iid = iid.clone();
        async move { get_fleet_detail(iid).await }
    })?;

    match &*data.read() {
        Some(Ok(d)) => render_detail(d),
        Some(Err(e)) => rsx! {
            p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" }
        },
        None => rsx! {
            p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." }
        },
    }
}

fn render_detail(d: &FleetDetailData) -> Element {
    let reported = d.reported_at.format("%Y-%m-%d %H:%M:%S").to_string();
    let inv_collected = d
        .inventory_collected_at
        .map(|t| t.format("%Y-%m-%d %H:%M").to_string());

    let sample_rows = d.sample.as_ref().map(build_sample_rows).unwrap_or_default();
    let inventory_rows = d.inventory.as_ref().map(build_inventory_rows).unwrap_or_default();
    let security_rows = d.security.as_ref().map(build_security_rows).unwrap_or_default();
    let disks = d
        .sample
        .as_ref()
        .and_then(|s| s.get("disk_free"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let interfaces = d
        .inventory
        .as_ref()
        .and_then(|i| i.get("interfaces"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let gpus = merge_gpu_data(d.inventory.as_ref(), d.sample.as_ref());

    // Service badges from the daemon's own health flags. Same semantics
    // as the fleet-dashboard Services column: green = daemon says up,
    // red = daemon says down. Probe state is elsewhere on the page.
    let mut service_badges: Vec<(String, bool, bool, bool)> = d
        .services
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|s| {
                    (
                        s.get("name").and_then(|v| v.as_str()).unwrap_or("?").to_string(),
                        s.get("healthy").and_then(|v| v.as_bool()).unwrap_or(false),
                        s.get("upgrade_pending").and_then(|v| v.as_bool()).unwrap_or(false),
                        s.get("busy").and_then(|v| v.as_bool()).unwrap_or(false),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    service_badges.sort_by(|a, b| a.0.cmp(&b.0));

    // Tunnel entries paired with the relay hostname. Present only when
    // the daemon published both — a daemon behind a relay it can't reach
    // won't emit relay_proxy_hostname so we won't show clickable buttons.
    let tunnel_names: Vec<String> = d
        .tunnels
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let proxy_url = d.relay_proxy_url.clone();
    let proxy_hostname = d.relay_proxy_hostname.clone();
    let instance_prefix: String = d.instance_id.chars().take(12).collect();

    rsx! {
        div { class: "flex items-baseline justify-between mb-4",
            div {
                h2 { class: "text-2xl font-bold", "{d.hostname}" }
                div { class: "text-sm text-gray-500 dark:text-gray-400",
                    "{d.cluster_name} · {d.environment} · v{d.version}"
                }
            }
            div { class: "text-right text-xs text-gray-500 dark:text-gray-400",
                "instance_id: "
                code { class: "font-mono", "{d.instance_id}" }
                br {}
                "last heartbeat: {reported}"
            }
        }

        // ── Services ──
        if !service_badges.is_empty() {
            div { class: "mb-4",
                h3 { class: "text-lg font-semibold mb-2", "Services" }
                div { class: "flex flex-wrap gap-2",
                    for (name, healthy, upgrade_pending, busy) in service_badges.iter() {
                        {
                            let cls = if *healthy {
                                "bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200"
                            } else {
                                "bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200"
                            };
                            let title = match (*healthy, *upgrade_pending, *busy) {
                                (true, true, _) => "healthy · upgrade pending".to_string(),
                                (true, _, true) => "healthy · busy".to_string(),
                                (true, _, _) => "healthy".to_string(),
                                (false, _, _) => "unhealthy".to_string(),
                            };
                            rsx! {
                                span {
                                    class: "inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium {cls}",
                                    title: "{title}",
                                    "{name}"
                                    if *upgrade_pending {
                                        span { class: "opacity-70", "⏫" }
                                    }
                                    if *busy {
                                        span { class: "opacity-70", "…" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Tunnels ──
        // Clickable only when both the tunnels array and relay_proxy_url
        // are present. Without a proxy URL the daemon is either on a
        // bare relay or offline, so the buttons have nowhere to point.
        if !tunnel_names.is_empty() {
            div { class: "mb-6",
                h3 { class: "text-lg font-semibold mb-2", "Tunnels" }
                if let Some(ref purl) = proxy_url {
                    div { class: "flex flex-wrap gap-2",
                        for tname in tunnel_names.iter() {
                            {
                                let tn = tname.clone();
                                let pu = purl.clone();
                                let ph = proxy_hostname.clone().unwrap_or_default();
                                let iid = instance_prefix.clone();
                                rsx! {
                                    button {
                                        class: "inline-block px-2 py-0.5 rounded text-xs font-medium bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200 hover:bg-blue-200 dark:hover:bg-blue-800 cursor-pointer",
                                        title: "Open a short-lived proxy URL in a new tab",
                                        onclick: move |_| {
                                            let tn = tn.clone();
                                            let pu = pu.clone();
                                            let ph = ph.clone();
                                            let iid = iid.clone();
                                            async move {
                                                match create_detail_proxy_token().await {
                                                    Ok(res) => {
                                                        let scheme = if pu.starts_with("https://") { "https://" } else { "http://" };
                                                        let url = format!(
                                                            "{scheme}{iid}-{tn}.{ph}/proxy?proxy_token={}",
                                                            res.proxy_token
                                                        );
                                                        let _ = document::eval(&format!(
                                                            "window.open('{}', '_blank')",
                                                            url.replace('\'', "\\'"),
                                                        ));
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("proxy token creation failed: {e}");
                                                    }
                                                }
                                            }
                                        },
                                        "{tname}"
                                    }
                                }
                            }
                        }
                    }
                } else {
                    div { class: "flex flex-wrap gap-2",
                        for tname in tunnel_names.iter() {
                            span { class: "inline-block px-2 py-0.5 rounded text-xs font-medium bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300",
                                title: "No relay proxy hostname reported — daemon isn't reachable via the relay",
                                "{tname}"
                            }
                        }
                    }
                }
            }
        }

        // ── Configuration Files link ──
        {
            let has_files = d.file_tunnels
                .as_ref()
                .and_then(|v| v.as_array())
                .is_some_and(|a| !a.is_empty());
            if has_files {
                let files_url = format!("/fleet/{}/files", d.instance_id);
                rsx! {
                    div { class: "mb-6",
                        Link {
                            to: files_url,
                            class: "inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium bg-blue-600 text-white rounded hover:bg-blue-700",
                            "Configuration Files"
                        }
                    }
                }
            } else {
                rsx! {}
            }
        }

        // ── Probes ──
        h3 { class: "text-lg font-semibold mb-2", "Probes" }
        if d.probes.is_empty() {
            p { class: "text-sm text-gray-500 dark:text-gray-400 mb-4",
                "No probe results yet — the first run can take up to 15 minutes."
            }
        } else {
            div { class: "mb-6 overflow-x-auto",
                table { class: "min-w-full divide-y divide-gray-200 dark:divide-gray-700 bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30",
                    thead { class: "bg-gray-50 dark:bg-gray-700",
                        tr {
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Service" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Kind" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Result" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Duration" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Tokens" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Model" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Collected" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Detail" }
                        }
                    }
                    tbody { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for p in d.probes.iter() {
                            {
                                let (badge_cls, badge_text) = if p.ok {
                                    ("bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200", "ok")
                                } else {
                                    ("bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200", "fail")
                                };
                                let duration = format!("{} ms", p.duration_ms);
                                let first_tok = p
                                    .first_token_ms
                                    .map(|v| format!(" · TTFT {v}ms"))
                                    .unwrap_or_default();
                                let tokens = match (p.tokens_in, p.tokens_out) {
                                    (Some(i), Some(o)) => format!("{i}→{o}"),
                                    (None, Some(o)) => format!("→{o}"),
                                    (Some(i), None) => format!("{i}→"),
                                    (None, None) => "—".to_string(),
                                };
                                let model = p.model.clone().unwrap_or_else(|| "—".to_string());
                                let collected = p.collected_at.format("%Y-%m-%d %H:%M:%S").to_string();
                                let detail = match (&p.error_class, &p.error_detail) {
                                    (Some(c), Some(d)) => format!("{c}: {d}"),
                                    (Some(c), None) => c.clone(),
                                    (None, _) => "—".to_string(),
                                };
                                rsx! {
                                    tr {
                                        td { class: "px-4 py-2 text-sm font-medium text-gray-900 dark:text-gray-100", "{p.service}" }
                                        td { class: "px-4 py-2 text-xs text-gray-500 dark:text-gray-400", "{p.kind}" }
                                        td { class: "px-4 py-2",
                                            span { class: "px-2 py-0.5 rounded text-xs font-medium {badge_cls}",
                                                "{badge_text}"
                                            }
                                        }
                                        td { class: "px-4 py-2 text-xs font-mono text-gray-600 dark:text-gray-300",
                                            "{duration}{first_tok}"
                                        }
                                        td { class: "px-4 py-2 text-xs font-mono text-gray-600 dark:text-gray-300", "{tokens}" }
                                        td { class: "px-4 py-2 text-xs text-gray-600 dark:text-gray-300", "{model}" }
                                        td { class: "px-4 py-2 text-xs text-gray-500 dark:text-gray-400", "{collected}" }
                                        td { class: "px-4 py-2 text-xs text-gray-600 dark:text-gray-300 max-w-md truncate",
                                            title: "{detail}",
                                            "{detail}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Dynamic sample ──
        h3 { class: "text-lg font-semibold mb-2", "Dynamic sample" }
        div { class: "mb-6 bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 p-4",
            if sample_rows.is_empty() {
                p { class: "text-sm text-gray-500 dark:text-gray-400",
                    "No sample on the most recent heartbeat."
                }
            } else {
                KvGrid { rows: sample_rows }
                if !disks.is_empty() {
                    h4 { class: "mt-4 mb-2 text-sm font-semibold text-gray-700 dark:text-gray-200", "Disks" }
                    table { class: "min-w-full text-sm",
                        thead {
                            tr { class: "text-xs text-gray-500 dark:text-gray-400",
                                th { class: "text-left py-1 pr-4", "Mount" }
                                th { class: "text-left py-1 pr-4", "Free" }
                                th { class: "text-left py-1 pr-4", "Total" }
                                th { class: "text-left py-1", "Used" }
                            }
                        }
                        tbody {
                            for disk in disks.iter() {
                                {
                                    let mount = disk.get("mount").and_then(|v| v.as_str()).unwrap_or("—").to_string();
                                    let free = disk.get("free_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let total = disk.get("total_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let used_pct = if total == 0 {
                                        "—".to_string()
                                    } else {
                                        format!("{}%", ((total - free) * 100 / total).min(100))
                                    };
                                    let free_s = human_bytes(free);
                                    let total_s = human_bytes(total);
                                    rsx! {
                                        tr { class: "text-gray-700 dark:text-gray-300",
                                            td { class: "py-1 pr-4 font-mono text-xs", "{mount}" }
                                            td { class: "py-1 pr-4 font-mono text-xs", "{free_s}" }
                                            td { class: "py-1 pr-4 font-mono text-xs", "{total_s}" }
                                            td { class: "py-1 font-mono text-xs", "{used_pct}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Inventory ──
        h3 { class: "text-lg font-semibold mb-2",
            "Inventory"
            if let Some(at) = inv_collected.as_ref() {
                span { class: "ml-2 text-xs font-normal text-gray-500 dark:text-gray-400",
                    "collected {at}"
                }
            }
        }
        div { class: "mb-6 bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 p-4",
            if inventory_rows.is_empty() {
                p { class: "text-sm text-gray-500 dark:text-gray-400",
                    "No inventory snapshot yet."
                }
            } else {
                KvGrid { rows: inventory_rows }
                if !interfaces.is_empty() {
                    h4 { class: "mt-4 mb-2 text-sm font-semibold text-gray-700 dark:text-gray-200", "Network interfaces" }
                    div { class: "flex flex-wrap gap-2",
                        for iface in interfaces.iter() {
                            {
                                let name = iface.get("name").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                let up = iface.get("up").and_then(|v| v.as_bool()).unwrap_or(false);
                                let cls = if up {
                                    "bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200"
                                } else {
                                    "bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300"
                                };
                                rsx! {
                                    span { class: "px-2 py-0.5 rounded text-xs font-mono {cls}",
                                        "{name}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── GPUs ──
        if !gpus.is_empty() {
            h3 { class: "text-lg font-semibold mb-2", "GPUs" }
            div { class: "mb-6 overflow-x-auto",
                table { class: "min-w-full divide-y divide-gray-200 dark:divide-gray-700 bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30",
                    thead { class: "bg-gray-50 dark:bg-gray-700",
                        tr {
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "#" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Vendor" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Name" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Driver" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "VRAM" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Util" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Temp" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "Power" }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase", "PCI" }
                        }
                    }
                    tbody { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for g in gpus.iter() {
                            {
                                let vendor_cls = match g.vendor.as_str() {
                                    "nvidia" => "bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200",
                                    "amd" => "bg-red-100 dark:bg-red-900 text-red-800 dark:text-red-200",
                                    "apple" => "bg-gray-200 dark:bg-gray-700 text-gray-800 dark:text-gray-200",
                                    "intel" => "bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200",
                                    _ => "bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300",
                                };
                                let vram = if g.vram_total_bytes > 0 {
                                    match g.vram_used_bytes {
                                        Some(used) => format!(
                                            "{} / {}",
                                            human_bytes(used),
                                            human_bytes(g.vram_total_bytes),
                                        ),
                                        None => human_bytes(g.vram_total_bytes),
                                    }
                                } else {
                                    "—".to_string()
                                };
                                let util = g.utilization_pct.map(|v| format!("{v}%")).unwrap_or_else(|| "—".into());
                                let temp = g.temperature_c.map(|v| format!("{v} °C")).unwrap_or_else(|| "—".into());
                                let power = g.power_watts.map(|v| format!("{v:.0} W")).unwrap_or_else(|| "—".into());
                                let driver = g.driver_version.clone().unwrap_or_else(|| "—".into());
                                let pci = g.pci_bus_id.clone().unwrap_or_default();
                                rsx! {
                                    tr {
                                        td { class: "px-4 py-2 text-xs font-mono text-gray-600 dark:text-gray-300", "{g.index}" }
                                        td { class: "px-4 py-2",
                                            span { class: "px-2 py-0.5 rounded text-xs font-medium {vendor_cls}",
                                                "{g.vendor}"
                                            }
                                        }
                                        td { class: "px-4 py-2 text-sm text-gray-900 dark:text-gray-100", "{g.name}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-gray-600 dark:text-gray-300", "{driver}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-gray-600 dark:text-gray-300", "{vram}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-gray-600 dark:text-gray-300", "{util}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-gray-600 dark:text-gray-300", "{temp}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-gray-600 dark:text-gray-300", "{power}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-gray-500 dark:text-gray-400", "{pci}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Security posture ──
        h3 { class: "text-lg font-semibold mb-2", "Security posture" }
        div { class: "mb-6 bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 p-4",
            if security_rows.is_empty() {
                p { class: "text-sm text-gray-500 dark:text-gray-400",
                    "No posture data yet."
                }
            } else {
                KvGrid { rows: security_rows }
            }
        }

        if let Some(commit) = d.nixpkgs_commit.as_ref() {
            div { class: "text-xs text-gray-500 dark:text-gray-400",
                "Nixpkgs commit: "
                code { class: "font-mono", "{commit}" }
            }
        }
    }
}

#[component]
fn KvGrid(rows: Vec<(String, String)>) -> Element {
    rsx! {
        dl { class: "grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-2",
            for (k, v) in rows.iter() {
                div { class: "flex justify-between border-b border-gray-100 dark:border-gray-700 pb-1 text-sm",
                    dt { class: "text-gray-500 dark:text-gray-400 mr-4", "{k}" }
                    dd { class: "text-right font-mono text-xs text-gray-800 dark:text-gray-200 break-all", "{v}" }
                }
            }
        }
    }
}

fn build_sample_rows(v: &serde_json::Value) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    if let Some(n) = v.get("cpu_load_1m").and_then(|x| x.as_f64()) {
        rows.push(("CPU load (1m)".into(), format!("{n:.2}")));
    }
    if let (Some(used), Some(total)) = (
        v.get("mem_used_bytes").and_then(|x| x.as_u64()),
        v.get("mem_total_bytes").and_then(|x| x.as_u64()),
    ) {
        let pct = if total > 0 { used * 100 / total } else { 0 };
        rows.push((
            "Memory".into(),
            format!("{} / {} ({pct}%)", human_bytes(used), human_bytes(total)),
        ));
    }
    if let Some(swap) = v.get("swap_used_bytes").and_then(|x| x.as_u64()) {
        if swap > 0 {
            rows.push(("Swap".into(), human_bytes(swap)));
        }
    }
    if let Some(rx) = v.get("net_rx_bytes").and_then(|x| x.as_u64()) {
        rows.push(("Net RX".into(), human_bytes(rx)));
    }
    if let Some(tx) = v.get("net_tx_bytes").and_then(|x| x.as_u64()) {
        rows.push(("Net TX".into(), human_bytes(tx)));
    }
    if let Some(pc) = v.get("process_count").and_then(|x| x.as_u64()) {
        rows.push(("Processes".into(), pc.to_string()));
    }
    if let Some(t) = v.get("thermal_state").and_then(|x| x.as_str()) {
        rows.push(("Thermal".into(), t.to_string()));
    }
    rows
}

fn build_inventory_rows(v: &serde_json::Value) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let get_s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(String::from);
    if let Some(os_name) = get_s("os_name") {
        let os_ver = get_s("os_version").unwrap_or_default();
        rows.push(("OS".into(), format!("{os_name} {os_ver}").trim().to_string()));
    }
    if let Some(k) = get_s("kernel_version") {
        rows.push(("Kernel".into(), k));
    }
    if let Some(a) = get_s("arch") {
        rows.push(("Arch".into(), a));
    }
    if let Some(c) = get_s("cpu_model") {
        rows.push(("CPU".into(), c));
    }
    if let (Some(phys), Some(log)) = (
        v.get("cpu_cores_physical").and_then(|x| x.as_u64()),
        v.get("cpu_cores_logical").and_then(|x| x.as_u64()),
    ) {
        rows.push(("Cores".into(), format!("{phys} physical / {log} logical")));
    }
    if let Some(mem) = v.get("mem_total_bytes").and_then(|x| x.as_u64()) {
        rows.push(("Memory".into(), human_bytes(mem)));
    }
    if let Some(u) = v.get("uptime_secs").and_then(|x| x.as_u64()) {
        rows.push(("Uptime".into(), human_duration(u)));
    }
    if let Some(s) = get_s("supervisor") {
        rows.push(("Supervisor".into(), s));
    }
    if let Some(n) = get_s("nix_version") {
        rows.push(("Nix".into(), n));
    }
    if let Some(c) = get_s("nixpkgs_commit") {
        rows.push(("Nixpkgs pin".into(), c));
    }
    rows
}

fn build_security_rows(v: &serde_json::Value) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    for (key, label) in [
        ("sip_enabled", "SIP"),
        ("filevault_enabled", "FileVault"),
        ("firewall_enabled", "Firewall"),
        ("gatekeeper_enabled", "Gatekeeper"),
        ("fde_enabled", "Full-disk encryption"),
        ("ufw_active", "ufw"),
    ] {
        if let Some(b) = v.get(key).and_then(|x| x.as_bool()) {
            rows.push((label.into(), if b { "on".into() } else { "off".into() }));
        }
    }
    for (key, label) in [
        ("xprotect_version", "XProtect"),
        ("selinux_mode", "SELinux"),
    ] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            if !s.is_empty() {
                rows.push((label.into(), s.into()));
            }
        }
    }
    if let Some(n) = v.get("apparmor_profiles").and_then(|x| x.as_u64()) {
        rows.push(("AppArmor profiles".into(), n.to_string()));
    }
    if let Some(n) = v.get("nftables_rule_count").and_then(|x| x.as_u64()) {
        let label = if n == 0 {
            "nftables rules (no policy)".to_string()
        } else {
            "nftables rules".to_string()
        };
        rows.push((label, n.to_string()));
    }
    rows
}

/// One row per GPU combining the static inventory entry with its latest
/// sample (utilization, temp, power, vram_used). Indices are matched via
/// `index` — inventory is the source of truth for ordering.
#[derive(Debug, Clone)]
struct GpuRow {
    index: u32,
    vendor: String,
    name: String,
    driver_version: Option<String>,
    pci_bus_id: Option<String>,
    vram_total_bytes: u64,
    vram_used_bytes: Option<u64>,
    utilization_pct: Option<u8>,
    temperature_c: Option<i32>,
    power_watts: Option<f32>,
}

fn merge_gpu_data(
    inventory: Option<&serde_json::Value>,
    sample: Option<&serde_json::Value>,
) -> Vec<GpuRow> {
    let inv_arr = inventory
        .and_then(|i| i.get("gpus"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let sample_arr = sample
        .and_then(|s| s.get("gpus"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut samples_by_idx: std::collections::HashMap<u32, &serde_json::Value> =
        std::collections::HashMap::new();
    for s in &sample_arr {
        if let Some(idx) = s.get("index").and_then(|v| v.as_u64()) {
            samples_by_idx.insert(idx as u32, s);
        }
    }

    inv_arr
        .iter()
        .map(|g| {
            let index = g.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let s = samples_by_idx.get(&index).copied();
            GpuRow {
                index,
                vendor: g
                    .get("vendor")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                name: g
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                driver_version: g
                    .get("driver_version")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                pci_bus_id: g
                    .get("pci_bus_id")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                vram_total_bytes: g
                    .get("vram_total_bytes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                vram_used_bytes: s.and_then(|s| s.get("vram_used_bytes")).and_then(|v| v.as_u64()),
                utilization_pct: s
                    .and_then(|s| s.get("utilization_pct"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u8),
                temperature_c: s
                    .and_then(|s| s.get("temperature_c"))
                    .and_then(|v| v.as_i64())
                    .map(|n| n as i32),
                power_watts: s
                    .and_then(|s| s.get("power_watts"))
                    .and_then(|v| v.as_f64())
                    .map(|n| n as f32),
            }
        })
        .collect()
}

fn human_bytes(n: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    if n == 0 {
        return "0 B".into();
    }
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", UNITS[i])
}

fn human_duration(secs: u64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}
