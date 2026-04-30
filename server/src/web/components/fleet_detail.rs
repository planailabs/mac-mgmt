use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Dot, ErrorText, HelpText, Kicker, Mono, Pill, PillVariant, SectionHeading,
};
#[cfg(feature = "server")]
use crate::web::user::current_user;

/// Build a tunnel URL from the proxy URL and subdomain prefix.
///
/// Given `"https://relay.plan.ai"`, prefix `"abc-ollama"` →
///   `"https://abc-ollama.relay.plan.ai/proxy"`
/// Given `"http://localhost:7379"`, prefix `"abc-ollama"` →
///   `"http://abc-ollama.localhost:7379/proxy"`
pub fn build_tunnel_url(proxy_url: &str, subdomain_prefix: &str, proxy_token: &str) -> String {
    let scheme = if proxy_url.starts_with("https://") {
        "https://"
    } else {
        "http://"
    };
    let without_scheme = proxy_url
        .strip_prefix(scheme)
        .unwrap_or(proxy_url)
        .trim_end_matches('/');
    format!("{scheme}{subdomain_prefix}.{without_scheme}/proxy?proxy_token={proxy_token}")
}

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
    #[serde(default)]
    git_sha: Option<String>,
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
    /// Exposed shell commands for remote execution.
    #[serde(default)]
    shell_tunnels: Option<serde_json::Value>,
    inventory: Option<serde_json::Value>,
    inventory_collected_at: Option<DateTime<Utc>>,
    security: Option<serde_json::Value>,
    /// Per-service dynamic samples from the latest heartbeat.
    #[serde(default)]
    service_samples: Option<serde_json::Value>,
    /// Per-service static inventory from the latest assessment.
    #[serde(default)]
    service_inventories: Option<serde_json::Value>,
    /// Per-service security findings from the latest assessment.
    #[serde(default)]
    service_security: Option<serde_json::Value>,
    probes: Vec<ProbeEntry>,
    viewer_is_admin: bool,
}

use super::fleet_dashboard::create_proxy_token;

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
async fn get_mac_mgmt_commit_count(sha: String) -> Result<Option<u64>, ServerFnError> {
    let shas = std::collections::HashSet::from([sha.clone()]);
    let counts = crate::commit_count::mac_mgmt_commit_counts(&shas).await;
    Ok(counts.get(&sha).copied())
}

#[server]
async fn get_nixpkgs_commit_count_detail(sha: String) -> Result<Option<u64>, ServerFnError> {
    let shas = std::collections::HashSet::from([sha.clone()]);
    let counts = crate::commit_count::nixpkgs_commit_counts(&shas).await;
    Ok(counts.get(&sha).copied())
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
        git_sha: Option<String>,
        nixpkgs_commit: Option<String>,
        reported_at: DateTime<Utc>,
        sample: Option<serde_json::Value>,
        services_extended: Option<serde_json::Value>,
        services: serde_json::Value,
        tunnels: serde_json::Value,
        relay_proxy_hostname: Option<String>,
        relay_proxy_url: Option<String>,
        file_tunnels: serde_json::Value,
        shell_tunnels: serde_json::Value,
        service_samples: Option<serde_json::Value>,
    }
    let hb: HbRow = sqlx::query_as(
        "SELECT c.id AS cluster_id, c.name AS cluster_name, dh.hostname, dh.environment, \
                dh.version, dh.git_sha, dh.nixpkgs_commit, dh.reported_at, dh.sample, dh.services_extended, \
                dh.services, dh.tunnels, dh.relay_proxy_hostname, dh.relay_proxy_url, dh.file_tunnels, \
                dh.shell_tunnels, dh.service_samples \
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
        service_inventories: Option<serde_json::Value>,
        service_security: Option<serde_json::Value>,
        collected_at: DateTime<Utc>,
    }
    let ass: Option<AssRow> = sqlx::query_as(
        "SELECT inventory, security, service_inventories, service_security, collected_at \
         FROM assessments WHERE instance_id = $1 ORDER BY collected_at DESC LIMIT 1",
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
        "SELECT DISTINCT ON (service, kind) \
                service, kind, ok, duration_ms, tokens_in, tokens_out, first_token_ms, \
                model, canary_digest, error_class, error_detail, collected_at \
         FROM assessment_probes \
         WHERE instance_id = $1 \
         ORDER BY service, kind, collected_at DESC",
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
        git_sha: hb.git_sha,
        nixpkgs_commit: hb.nixpkgs_commit,
        reported_at: hb.reported_at,
        sample: hb.sample,
        services_extended: hb.services_extended,
        services: Some(hb.services),
        tunnels: Some(hb.tunnels),
        relay_proxy_hostname: hb.relay_proxy_hostname,
        relay_proxy_url: hb.relay_proxy_url,
        file_tunnels: Some(hb.file_tunnels),
        shell_tunnels: Some(hb.shell_tunnels),
        inventory: ass.as_ref().map(|a| a.inventory.clone()),
        inventory_collected_at: ass.as_ref().map(|a| a.collected_at),
        security: ass.as_ref().map(|a| a.security.clone()),
        service_samples: hb.service_samples,
        service_inventories: ass.as_ref().and_then(|a| a.service_inventories.clone()),
        service_security: ass.as_ref().and_then(|a| a.service_security.clone()),
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

    // Topbar shows the hostname (or instance id when hostname is empty).
    // We pull it from the loaded data; while loading, the topbar stays
    // empty rather than flashing a placeholder.
    let topbar_title = match &*data.read() {
        Some(Ok(d)) if !d.hostname.is_empty() => d.hostname.clone(),
        Some(Ok(d)) => d.instance_id.clone(),
        _ => String::new(),
    };
    let topbar_subtitle = match &*data.read() {
        Some(Ok(d)) => Some(format!("{} · {}", d.cluster_name, d.environment)),
        _ => None,
    };
    use_topbar(topbar_title, topbar_subtitle);

    match &*data.read() {
        Some(Ok(d)) => render_detail(d),
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}

fn render_detail(d: &FleetDetailData) -> Element {
    let reported = d.reported_at.format("%Y-%m-%d %H:%M:%S").to_string();
    let inv_collected = d
        .inventory_collected_at
        .map(|t| t.format("%Y-%m-%d %H:%M").to_string());

    let sample_rows = d.sample.as_ref().map(build_sample_rows).unwrap_or_default();
    let inventory_rows = d
        .inventory
        .as_ref()
        .map(build_inventory_rows)
        .unwrap_or_default();
    let security_items = d
        .security
        .as_ref()
        .map(build_security_items)
        .unwrap_or_default();
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
                        s.get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?")
                            .to_string(),
                        s.get("healthy").and_then(|v| v.as_bool()).unwrap_or(false),
                        s.get("upgrade_pending")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        s.get("busy").and_then(|v| v.as_bool()).unwrap_or(false),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    service_badges.sort_by(|a, b| a.0.cmp(&b.0));

    let tunnels: Vec<(String, String)> = d
        .tunnels
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let name = t.get("name").and_then(|v| v.as_str())?.to_string();
                    let port = t
                        .get("tcp_port")
                        .or_else(|| t.get("port"))
                        .and_then(|v| v.as_u64())
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| t!("em-dash"));
                    Some((name, port))
                })
                .collect()
        })
        .unwrap_or_default();
    let proxy_url = d.relay_proxy_url.clone();
    let instance_prefix: String = d.instance_id.chars().take(12).collect();

    // ── Page hero ────────────────────────────────────────────────
    // Composes the design's "Cluster Detail" hero: row of status pills
    // (env / version / commit#) above a 36px display name with the
    // monospace instance hash underneath. Right side carries the live
    // dot + last-heartbeat timestamp. Stacks vertically on phones.
    let env_variant = match d.environment.as_str() {
        "production" => PillVariant::Accent,
        "staging" => PillVariant::Warn,
        _ => PillVariant::Muted,
    };
    let online = Utc::now().signed_duration_since(d.reported_at).num_seconds() < 300;
    let live_variant = if online { PillVariant::Ok } else { PillVariant::Bad };
    let instance_short: String = d.instance_id.chars().take(56).collect();

    rsx! {
        div { class: "flex flex-col xl:flex-row xl:items-end xl:justify-between gap-4 mb-6",
            div { class: "min-w-0",
                div { class: "flex items-center gap-2 mb-2 flex-wrap",
                    Pill { variant: env_variant, "{d.environment}" }
                    Pill { variant: PillVariant::Muted, mono: true, "v{d.version}" }
                    if let Some(sha) = &d.git_sha {
                        {
                            let sha_for_count = sha.clone();
                            let count_res = use_resource(move || {
                                let s = sha_for_count.clone();
                                async move { get_mac_mgmt_commit_count(s).await.ok().flatten() }
                            });
                            let count_label = count_res.read().as_ref()
                                .and_then(|n| n.as_ref())
                                .map(|n| format!("#{n}"))
                                .unwrap_or_else(|| {
                                    let short: String = sha.chars().take(7).collect();
                                    short
                                });
                            rsx! {
                                Pill { variant: PillVariant::Muted, mono: true, "{count_label}" }
                            }
                        }
                    }
                }
                Kicker { class: "mb-1", "{d.cluster_name}" }
                h1 { class: "h-display", "{d.hostname}" }
                div { class: "mt-2 text-fg-muted text-sm font-mono truncate",
                    "{instance_short}"
                }
            }
            div { class: "flex flex-col items-start xl:items-end gap-2 shrink-0",
                div { class: "flex items-center gap-2 text-fg-muted text-xs",
                    Dot { variant: live_variant }
                    span { {t!("fleet-detail-last-heartbeat", time: reported.clone())} }
                }
                div { class: "text-fg-faint text-xs",
                    {t!("fleet-detail-instance-id")} " "
                    Mono { class: "text-xs", "{d.instance_id}" }
                }
            }
        }

        // ── Services ──
        SectionHeading { class: "mb-2", {t!("fleet-detail-services")} }
        if service_badges.is_empty() {
            p { class: "text-sm text-fg-muted mb-4",
                {t!("fleet-detail-no-services")}
            }
        } else {
            div { class: "mb-6 overflow-x-auto",
                table { class: "table card",
                    thead { class: "thead",
                        tr {
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-service")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("status")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-upgrade")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-busy")} }
                        }
                    }
                    tbody { class: "tbody",
                        for (name, healthy, upgrade_pending, busy) in service_badges.iter() {
                            {
                                let (badge_cls, badge_text) = if *healthy {
                                    ("badge badge-success", t!("fleet-healthy"))
                                } else {
                                    ("badge badge-danger", t!("fleet-unhealthy"))
                                };
                                let upgrade = if *upgrade_pending { t!("fleet-pending") } else { t!("em-dash") };
                                let busy_text = if *busy { "yes".to_string() } else { t!("em-dash") };
                                rsx! {
                                    tr {
                                        td { class: "px-4 py-2 text-sm font-medium text-fg-strong", "{name}" }
                                        td { class: "px-4 py-2",
                                            span { class: "{badge_cls}", "{badge_text}" }
                                        }
                                        td { class: "px-4 py-2 text-xs text-fg", "{upgrade}" }
                                        td { class: "px-4 py-2 text-xs text-fg", "{busy_text}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Tunnels ──
        if !tunnels.is_empty() {
            SectionHeading { class: "mb-2", {t!("fleet-detail-tunnels")} }
            div { class: "mb-6 overflow-x-auto",
                table { class: "table card",
                    thead { class: "thead",
                        tr {
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("name")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-port")} }
                            if proxy_url.is_some() {
                                th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", "" }
                            }
                        }
                    }
                    tbody { class: "tbody",
                        for (tname, tport) in tunnels.iter() {
                            {
                                let has_proxy = proxy_url.is_some();
                                let tn = tname.clone();
                                let pu = proxy_url.clone().unwrap_or_default();
                                let iid = instance_prefix.clone();
                                rsx! {
                                    tr {
                                        td { class: "px-4 py-2 text-sm font-medium text-fg-strong", "{tname}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-fg", "{tport}" }
                                        if has_proxy {
                                            td { class: "px-4 py-2",
                                                button { class: "btn btn-xs btn-info-soft",
                                                    title: "Open a short-lived proxy URL in a new tab",
                                                    onclick: move |_| {
                                                        let tn = tn.clone();
                                                        let pu = pu.clone();
                                                        let iid = iid.clone();
                                                        async move {
                                                            match create_proxy_token().await {
                                                                Ok(res) => {
                                                                    let prefix = format!("{iid}-{tn}");
                                                                    let url = build_tunnel_url(&pu, &prefix, &res.proxy_token);
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
                                                    {t!("fleet-detail-open")}
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

        // ── Remote tools ──
        {
            let has_relay = d.relay_proxy_url.is_some();
            let has_files = d.file_tunnels
                .as_ref()
                .and_then(|v| v.as_array())
                .is_some_and(|a| !a.is_empty());
            let has_shell = d.shell_tunnels
                .as_ref()
                .and_then(|v| v.as_array())
                .is_some_and(|a| !a.is_empty());
            if has_files || has_shell || has_relay {
                let files_url = format!("/fleet/{}/files", d.instance_id);
                let shell_url = format!("/fleet/{}/shell", d.instance_id);
                let logs_url = format!("/fleet/{}/logs", d.instance_id);
                let healer_url = format!("/fleet/{}/healer", d.instance_id);
                rsx! {
                    div { class: "mb-6 flex flex-wrap gap-2",
                        if has_files {
                            Link {
                                to: files_url,
                                class: "btn btn-md btn-primary inline-flex items-center gap-2",
                                {t!("fleet-detail-config-files")}
                            }
                        }
                        if has_shell {
                            Link {
                                to: shell_url,
                                class: "btn btn-md btn-primary inline-flex items-center gap-2",
                                {t!("fleet-detail-shell-commands")}
                            }
                        }
                        Link {
                            to: logs_url,
                            class: "btn btn-md btn-primary inline-flex items-center gap-2",
                            {t!("fleet-detail-logs")}
                        }
                        Link {
                            to: healer_url,
                            class: "btn btn-md btn-success-soft inline-flex items-center gap-2",
                            {t!("fleet-detail-healer-agent")}
                        }
                        super::push_menu::PushMenu { cluster_id: d.cluster_id.clone() }
                    }
                }
            } else {
                rsx! {}
            }
        }

        // ── Probes ──
        SectionHeading { class: "mb-2", {t!("fleet-detail-probes")} }
        if d.probes.is_empty() {
            p { class: "text-sm text-fg-muted mb-4",
                {t!("fleet-detail-no-probes")}
            }
        } else {
            div { class: "mb-6 overflow-x-auto",
                table { class: "table card",
                    thead { class: "thead",
                        tr {
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-service")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-kind")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-result")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-duration")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-tokens")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-model")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-collected")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-detail")} }
                        }
                    }
                    tbody { class: "tbody",
                        for p in d.probes.iter() {
                            {
                                let (badge_cls, badge_text) = if p.ok {
                                    ("badge badge-success", t!("fleet-ok"))
                                } else {
                                    ("badge badge-danger", t!("fleet-fail"))
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
                                    (None, None) => t!("em-dash"),
                                };
                                let model = p.model.clone().unwrap_or_else(|| t!("em-dash"));
                                let collected = p.collected_at.format("%Y-%m-%d %H:%M:%S").to_string();
                                let detail = match (&p.error_class, &p.error_detail) {
                                    (Some(c), Some(d)) => format!("{c}: {d}"),
                                    (Some(c), None) => c.clone(),
                                    (None, _) => t!("em-dash"),
                                };
                                rsx! {
                                    tr {
                                        td { class: "px-4 py-2 text-sm font-medium text-fg-strong", "{p.service}" }
                                        td { class: "px-4 py-2 text-xs text-fg-muted", "{p.kind}" }
                                        td { class: "px-4 py-2",
                                            span { class: "{badge_cls}", "{badge_text}" }
                                        }
                                        td { class: "px-4 py-2 text-xs font-mono text-fg",
                                            "{duration}{first_tok}"
                                        }
                                        td { class: "px-4 py-2 text-xs font-mono text-fg", "{tokens}" }
                                        td { class: "px-4 py-2 text-xs text-fg", "{model}" }
                                        td { class: "px-4 py-2 text-xs text-fg-muted", "{collected}" }
                                        td { class: "px-4 py-2 text-xs text-fg max-w-md truncate",
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
        SectionHeading { class: "mb-2", {t!("fleet-detail-dynamic-sample")} }
        div { class: "mb-6 card p-4",
            if sample_rows.is_empty() {
                p { class: "text-sm text-fg-muted",
                    {t!("fleet-detail-no-sample")}
                }
            } else {
                KvGrid { rows: sample_rows }
                if !disks.is_empty() {
                    h4 { class: "mt-4 mb-2 text-sm font-semibold text-fg-strong", {t!("fleet-detail-disks")} }
                    table { class: "min-w-full text-sm",
                        thead {
                            tr { class: "text-xs text-fg-muted",
                                th { class: "text-left py-1 pr-4", {t!("fleet-detail-col-mount")} }
                                th { class: "text-left py-1 pr-4", {t!("fleet-detail-col-free")} }
                                th { class: "text-left py-1 pr-4", {t!("fleet-detail-col-total")} }
                                th { class: "text-left py-1", {t!("fleet-detail-col-used")} }
                            }
                        }
                        tbody {
                            for disk in disks.iter() {
                                {
                                    let mount = disk.get("mount").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                    let free = disk.get("free_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let total = disk.get("total_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let used_pct = if total == 0 {
                                        t!("em-dash")
                                    } else {
                                        format!("{}%", ((total - free) * 100 / total).min(100))
                                    };
                                    let free_s = human_bytes(free);
                                    let total_s = human_bytes(total);
                                    rsx! {
                                        tr { class: "text-fg",
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
        SectionHeading { class: "mb-2",
            {t!("fleet-detail-inventory")}
            if let Some(at) = inv_collected.as_ref() {
                span { class: "ml-2 text-xs font-normal text-fg-muted",
                    {t!("fleet-detail-inventory-collected", time: at.clone())}
                }
            }
        }
        div { class: "mb-6 card p-4",
            if inventory_rows.is_empty() {
                p { class: "text-sm text-fg-muted",
                    {t!("fleet-detail-no-inventory")}
                }
            } else {
                KvGrid { rows: inventory_rows }
                // Nixpkgs commit with link + async commit count
                if let Some(nix_sha) = &d.nixpkgs_commit {
                    {
                        let short: String = nix_sha.chars().take(12).collect();
                        let url = format!("https://git.plan.ai/plan-ai/nixpkgs/-/commit/{nix_sha}");
                        let sha_for_count = nix_sha.clone();
                        let count_res = use_resource(move || {
                            let s = sha_for_count.clone();
                            async move { get_nixpkgs_commit_count_detail(s).await.ok().flatten() }
                        });
                        let count_label = count_res.read().as_ref()
                            .and_then(|n| n.as_ref())
                            .map(|n| format!(" #{n}"))
                            .unwrap_or_default();
                        rsx! {
                            div { class: "flex justify-between border-b border-line-soft pb-1 text-sm",
                                dt { class: "text-fg-muted mr-4", {t!("fleet-detail-nixpkgs-pin")} }
                                dd { class: "text-right font-mono text-xs text-fg-strong",
                                    a {
                                        class: "hover:text-brand",
                                        href: "{url}",
                                        target: "_blank",
                                        title: "{nix_sha}",
                                        "{short}{count_label}"
                                    }
                                }
                            }
                        }
                    }
                }
                if !interfaces.is_empty() {
                    h4 { class: "mt-4 mb-2 text-sm font-semibold text-fg-strong", {t!("fleet-detail-network")} }
                    div { class: "flex flex-wrap gap-2",
                        for iface in interfaces.iter() {
                            {
                                let name = iface.get("name").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                let up = iface.get("up").and_then(|v| v.as_bool()).unwrap_or(false);
                                let cls = if up {
                                    "badge badge-success"
                                } else {
                                    "badge badge-neutral"
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
            SectionHeading { class: "mb-2", {t!("fleet-detail-gpus")} }
            div { class: "mb-6 overflow-x-auto",
                table { class: "table card",
                    thead { class: "thead",
                        tr {
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-gpu-index")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-gpu-vendor")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("name")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-gpu-driver")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-gpu-vram")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-gpu-util")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-gpu-temp")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-gpu-power")} }
                            th { class: "px-4 py-2 text-left text-xs font-medium text-fg-muted uppercase", {t!("fleet-detail-col-gpu-pci")} }
                        }
                    }
                    tbody { class: "tbody",
                        for g in gpus.iter() {
                            {
                                let vendor_cls = match g.vendor.as_str() {
                                    "nvidia" => "badge badge-success",
                                    "amd" => "badge badge-danger",
                                    "apple" => "badge badge-neutral",
                                    "intel" => "badge badge-info",
                                    _ => "badge badge-neutral",
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
                                    t!("em-dash")
                                };
                                let util = g.utilization_pct.map(|v| format!("{v}%")).unwrap_or_else(|| t!("em-dash"));
                                let temp = g.temperature_c.map(|v| format!("{v} °C")).unwrap_or_else(|| t!("em-dash"));
                                let power = g.power_watts.map(|v| format!("{v:.0} W")).unwrap_or_else(|| t!("em-dash"));
                                let driver = g.driver_version.clone().unwrap_or_else(|| t!("em-dash"));
                                let pci = g.pci_bus_id.clone().unwrap_or_default();
                                rsx! {
                                    tr {
                                        td { class: "px-4 py-2 text-xs font-mono text-fg", "{g.index}" }
                                        td { class: "px-4 py-2",
                                            span { class: "{vendor_cls}", "{g.vendor}" }
                                        }
                                        td { class: "px-4 py-2 text-sm text-fg-strong", "{g.name}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-fg", "{driver}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-fg", "{vram}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-fg", "{util}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-fg", "{temp}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-fg", "{power}" }
                                        td { class: "px-4 py-2 text-xs font-mono text-fg-muted", "{pci}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Security posture ──
        SectionHeading { class: "mb-2", {t!("fleet-detail-security")} }
        div { class: "mb-6 card p-4",
            if security_items.is_empty() {
                p { class: "text-sm text-fg-muted",
                    {t!("fleet-detail-no-posture")}
                }
            } else {
                dl { class: "grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-2",
                    for (label, value, badge_cls) in security_items.iter() {
                        div { class: "flex justify-between border-b border-line-soft pb-1 text-sm",
                            dt { class: "text-fg-muted mr-4", "{label}" }
                            dd { class: "text-right",
                                span { class: "px-2 py-0.5 rounded text-xs font-mono {badge_cls}",
                                    "{value}"
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Per-service details ──
        {render_per_service_sections(&d)}

        if let Some(commit) = d.nixpkgs_commit.as_ref() {
            div { class: "text-xs text-fg-muted",
                {t!("fleet-detail-nixpkgs-commit")}
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
                div { class: "flex justify-between border-b border-line-soft pb-1 text-sm",
                    dt { class: "text-fg-muted mr-4", "{k}" }
                    dd { class: "text-right font-mono text-xs text-fg-strong break-all", "{v}" }
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
        rows.push((
            "OS".into(),
            format!("{os_name} {os_ver}").trim().to_string(),
        ));
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
    // nixpkgs_commit handled separately with link + commit count

    rows
}

/// Returns (label, value, badge_css_class) triples for security posture.
fn build_security_items(v: &serde_json::Value) -> Vec<(String, String, String)> {
    let green = "badge badge-success";
    let red = "badge badge-danger";
    let yellow = "badge badge-warn";
    let gray = "badge badge-neutral";

    // New format: Vec<SecurityFinding> (array of {id, severity, message, pass})
    if let Some(arr) = v.as_array() {
        return arr
            .iter()
            .filter_map(|f| {
                let msg = f.get("message")?.as_str()?.to_string();
                let pass = f.get("pass")?.as_bool()?;
                let severity = f.get("severity").and_then(|s| s.as_str()).unwrap_or("info");
                let (value, cls) = if pass {
                    ("pass".to_string(), green)
                } else {
                    let cls = match severity {
                        "critical" | "high" => red,
                        "medium" => yellow,
                        _ => gray,
                    };
                    (severity.to_string(), cls)
                };
                Some((msg, value, cls.to_string()))
            })
            .collect();
    }

    // Legacy format: SecurityPosture struct ({sip_enabled: bool, ...})
    let mut items = Vec::new();
    for (key, label) in [
        ("sip_enabled", "SIP"),
        ("filevault_enabled", "FileVault"),
        ("firewall_enabled", "Firewall"),
        ("gatekeeper_enabled", "Gatekeeper"),
        ("fde_enabled", "Full-disk encryption"),
        ("ufw_active", "ufw"),
    ] {
        if let Some(b) = v.get(key).and_then(|x| x.as_bool()) {
            let (val, cls) = if b { ("on", green) } else { ("off", red) };
            items.push((label.to_string(), val.to_string(), cls.to_string()));
        }
    }
    for (key, label) in [
        ("xprotect_version", "XProtect"),
        ("selinux_mode", "SELinux"),
    ] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            if !s.is_empty() {
                items.push((label.to_string(), s.to_string(), gray.to_string()));
            }
        }
    }
    if let Some(n) = v.get("apparmor_profiles").and_then(|x| x.as_u64()) {
        items.push(("AppArmor profiles".to_string(), n.to_string(), gray.to_string()));
    }
    if let Some(n) = v.get("nftables_rule_count").and_then(|x| x.as_u64()) {
        let label = if n == 0 {
            "nftables rules (no policy)".to_string()
        } else {
            "nftables rules".to_string()
        };
        let cls = if n == 0 { red } else { green };
        items.push((label, n.to_string(), cls.to_string()));
    }
    items
}

/// Render per-service inventory, samples, and security findings.
fn render_per_service_sections(d: &FleetDetailData) -> Element {
    // Collect all service names across inventory, samples, and security.
    let mut service_names: Vec<String> = Vec::new();
    let mut add_services = |json: &Option<serde_json::Value>| {
        if let Some(arr) = json.as_ref().and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(name) = item.get("service").and_then(|s| s.as_str()) {
                    if !service_names.contains(&name.to_string()) {
                        service_names.push(name.to_string());
                    }
                }
            }
        }
    };
    add_services(&d.service_inventories);
    add_services(&d.service_samples);
    add_services(&d.service_security);

    if service_names.is_empty() {
        return rsx! {};
    }

    let inv_arr = d.service_inventories.as_ref().and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let sample_arr = d.service_samples.as_ref().and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let sec_arr = d.service_security.as_ref().and_then(|v| v.as_array()).cloned().unwrap_or_default();

    rsx! {
        SectionHeading { class: "mb-2", {t!("fleet-detail-per-service")} }
        div { class: "mb-6 space-y-4",
            for svc_name in service_names.iter() {
                {
                    let inv_entries: Vec<&serde_json::Value> = inv_arr.iter()
                        .filter(|item| item.get("service").and_then(|s| s.as_str()) == Some(svc_name))
                        .flat_map(|item| item.get("entries").and_then(|e| e.as_array()).into_iter().flatten())
                        .collect();
                    let sample_entries: Vec<&serde_json::Value> = sample_arr.iter()
                        .filter(|item| item.get("service").and_then(|s| s.as_str()) == Some(svc_name))
                        .flat_map(|item| item.get("entries").and_then(|e| e.as_array()).into_iter().flatten())
                        .collect();
                    let sec_findings: Vec<&serde_json::Value> = sec_arr.iter()
                        .filter(|item| item.get("service").and_then(|s| s.as_str()) == Some(svc_name))
                        .flat_map(|item| item.get("findings").and_then(|f| f.as_array()).into_iter().flatten())
                        .collect();

                    let svc = svc_name.clone();
                    rsx! {
                        details { class: "card",
                            key: "{svc}",
                            summary { class: "px-4 py-2 cursor-pointer font-medium text-fg-strong hover:bg-surface-2 rounded",
                                "{svc}"
                            }
                            div { class: "px-4 pb-4 space-y-3",
                                // Inventory entries
                                if !inv_entries.is_empty() {
                                    div {
                                        h4 { class: "text-sm font-semibold text-fg mb-1", {t!("fleet-detail-tab-inventory")} }
                                        {render_inventory_entries(&inv_entries)}
                                    }
                                }
                                // Dynamic sample entries
                                if !sample_entries.is_empty() {
                                    div {
                                        h4 { class: "text-sm font-semibold text-fg mb-1", {t!("fleet-detail-tab-live-status")} }
                                        {render_inventory_entries(&sample_entries)}
                                    }
                                }
                                // Security findings
                                if !sec_findings.is_empty() {
                                    div {
                                        h4 { class: "text-sm font-semibold text-fg mb-1", {t!("fleet-detail-tab-security")} }
                                        {render_security_findings(&sec_findings)}
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

/// Render a list of InventoryEntry values as a key-value grid (dl/dt/dd).
fn render_inventory_entries(entries: &[&serde_json::Value]) -> Element {
    rsx! {
        dl { class: "grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-2",
            for entry in entries.iter() {
                {
                    let name = entry.get("name").and_then(|n| n.as_str()).unwrap_or("?").to_string();
                    let value = entry.get("value").cloned().unwrap_or(serde_json::Value::Null);
                    let display = format_inventory_value(&value);
                    rsx! {
                        div { class: "flex justify-between border-b border-line-soft pb-1 text-sm",
                            dt { class: "text-fg-muted mr-4", "{name}" }
                            dd { class: "text-right font-mono text-xs text-fg-strong break-all", "{display}" }
                        }
                    }
                }
            }
        }
    }
}

/// Render security findings as key-value rows (dl/dt/dd).
fn render_security_findings(findings: &[&serde_json::Value]) -> Element {
    rsx! {
        dl { class: "grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-2",
            for finding in findings.iter() {
                {
                    let msg = finding.get("message").and_then(|m| m.as_str()).unwrap_or("?").to_string();
                    let pass = finding.get("pass").and_then(|p| p.as_bool()).unwrap_or(false);
                    let severity = finding.get("severity").and_then(|s| s.as_str()).unwrap_or("info").to_string();
                    let cls = if pass {
                        "text-success"
                    } else {
                        match severity.as_str() {
                            "critical" | "high" => "text-danger font-semibold",
                            "medium" => "text-warn-strong",
                            _ => "text-fg-muted",
                        }
                    };
                    let status = if pass { "pass" } else { "fail" };
                    rsx! {
                        div { class: "flex justify-between border-b border-line-soft pb-1 text-sm",
                            dt { class: "text-fg-muted mr-4", "{msg}" }
                            dd { class: "text-right font-mono text-xs {cls}", "{status}" }
                        }
                    }
                }
            }
        }
    }
}

/// Format an InventoryEntry value for display.
fn format_inventory_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => if *b { "yes" } else { "no" }.to_string(),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(|item| {
                item.as_str().map(String::from).unwrap_or_else(|| item.to_string())
            }).collect();
            items.join(", ")
        }
        serde_json::Value::Null => t!("em-dash"),
        other => other.to_string(),
    }
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
                vram_used_bytes: s
                    .and_then(|s| s.get("vram_used_bytes"))
                    .and_then(|v| v.as_u64()),
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
