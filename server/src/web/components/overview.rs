//! Command Center / Overview dashboard.
//!
//! Lands at `/overview`. Surfaces a one-screen read of fleet health,
//! recent rollout activity and the open-pings backlog. The design's
//! Command Center mock includes a topology map; we don't carry geo
//! data per cluster yet, so the topology block is left for a future
//! follow-up. Everything else is wired to real data.

use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    ActivityFeed, ActivityItem, ChartColor, Dot, ErrorText, HelpText, Kicker, KpiCard, PageHero,
    Pill, PillVariant,
};
#[cfg(feature = "server")]
use crate::web::user::current_user;

/// Snapshot delivered to the page in a single round-trip.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct OverviewData {
    // KPIs
    instances_online: i64,
    instances_total: i64,
    healthy_services_pct: f64,
    healthy_services_count: i64,
    healthy_services_total: i64,
    active_rollouts: i64,
    rollouts_completed_24h: i64,
    open_staff_pings: i64,
    // Recent activity, newest first.
    activity: Vec<OverviewActivity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OverviewActivity {
    kind: String,   // "rollout-started" | "rollout-completed" | "cluster-online" | "ping-opened"
    text: String,
    /// Absolute timestamp; the page formats relative-ago at render.
    at: DateTime<Utc>,
}

#[server]
async fn get_overview() -> Result<OverviewData, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let accessible = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Heartbeats. "Online" matches the fleet-dashboard threshold.
    let online_query = "SELECT COUNT(*) FROM daemon_heartbeats \
                        WHERE reported_at > now() - interval '5 minutes'";
    let total_query  = "SELECT COUNT(*) FROM daemon_heartbeats";
    let (instances_online, instances_total) = match accessible.as_ref() {
        Some(ids) => {
            let online: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM daemon_heartbeats \
                 WHERE cluster_id = ANY($1) AND reported_at > now() - interval '5 minutes'",
            )
            .bind(ids)
            .fetch_one(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
            let total: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM daemon_heartbeats WHERE cluster_id = ANY($1)",
            )
            .bind(ids)
            .fetch_one(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
            (online, total)
        }
        None => {
            let online: i64 = sqlx::query_scalar(online_query)
                .fetch_one(&pool)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            let total: i64 = sqlx::query_scalar(total_query)
                .fetch_one(&pool)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            (online, total)
        }
    };

    // Service health rolled up across all heartbeats. We only consider
    // recent heartbeats so a long-dead daemon doesn't drag the number
    // down forever.
    let recent_services: Vec<serde_json::Value> = match accessible.as_ref() {
        Some(ids) => sqlx::query_scalar(
            "SELECT services FROM daemon_heartbeats \
             WHERE cluster_id = ANY($1) AND reported_at > now() - interval '5 minutes'",
        )
        .bind(ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?,
        None => sqlx::query_scalar(
            "SELECT services FROM daemon_heartbeats \
             WHERE reported_at > now() - interval '5 minutes'",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?,
    };
    let mut healthy_services_count = 0i64;
    let mut healthy_services_total = 0i64;
    for v in &recent_services {
        if let Some(arr) = v.as_array() {
            for s in arr {
                healthy_services_total += 1;
                if s.get("healthy").and_then(|x| x.as_bool()) == Some(true) {
                    healthy_services_count += 1;
                }
            }
        }
    }
    let healthy_services_pct = if healthy_services_total > 0 {
        healthy_services_count as f64 * 100.0 / healthy_services_total as f64
    } else {
        0.0
    };

    // Rollouts. Two scopes: currently rolling, and "completed in the
    // last 24h" so the operator gets a sense of recent throughput.
    let active_rollouts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM rollouts WHERE status = 'rolling'",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    // The rollouts table tracks state transitions via updated_at —
    // there's no separate completed_at column (those live on
    // rollout_stages). updated_at moves to "now" when status flips
    // to 'completed', so it's the right proxy here.
    let rollouts_completed_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM rollouts \
         WHERE status = 'completed' AND updated_at > now() - interval '24 hours'",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Open staff pings — pings without a resolution timestamp.
    let open_staff_pings: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM staff_pings WHERE resolved_at IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0); // table may not exist on legacy installs

    // Activity feed: union of the most recent rollout state changes
    // and the latest cluster-online heartbeats. The rollouts table
    // doesn't carry separate started_at/completed_at — we use
    // created_at as the start moment and updated_at as the
    // last-state-change moment (which is "completed_at" for completed
    // rollouts and the latest progress tick for rolling ones).
    #[derive(sqlx::FromRow)]
    struct RolloutEvent {
        id: uuid::Uuid,
        status: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        target_version: Option<String>,
    }
    let rollout_events: Vec<RolloutEvent> = sqlx::query_as(
        "SELECT id, status, created_at, updated_at, target_version \
         FROM rollouts \
         WHERE updated_at > now() - interval '24 hours' \
         ORDER BY updated_at DESC \
         LIMIT 6",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut activity: Vec<OverviewActivity> = Vec::new();
    for r in &rollout_events {
        let id_short: String = r.id.to_string().chars().take(8).collect();
        let ver = r
            .target_version
            .as_deref()
            .map(|v| format!(" → v{v}"))
            .unwrap_or_default();
        match r.status.as_str() {
            "completed" => {
                activity.push(OverviewActivity {
                    kind: "rollout-completed".into(),
                    text: format!("Rollout {id_short}{ver} completed"),
                    at: r.updated_at,
                });
            }
            "rolling" => {
                activity.push(OverviewActivity {
                    kind: "rollout-started".into(),
                    text: format!("Rollout {id_short}{ver} started"),
                    at: r.created_at,
                });
            }
            other => {
                activity.push(OverviewActivity {
                    kind: "rollout-other".into(),
                    text: format!("Rollout {id_short}{ver} · {other}"),
                    at: r.updated_at,
                });
            }
        }
    }

    // Recent online heartbeats — first time we see an instance after a
    // gap of 30+ minutes counts as "came online". We approximate by
    // showing the most recent `reported_at` per instance among hosts
    // whose previous heartbeat was older than 30 minutes. For the v1
    // we just take the freshest heartbeats; the gap heuristic is a
    // follow-up.
    #[derive(sqlx::FromRow)]
    struct HbEvent {
        hostname: String,
        cluster_name: String,
        reported_at: DateTime<Utc>,
    }
    let recent_hbs: Vec<HbEvent> = sqlx::query_as(
        "SELECT dh.hostname, c.name AS cluster_name, dh.reported_at \
         FROM daemon_heartbeats dh JOIN clusters c ON c.id = dh.cluster_id \
         WHERE dh.reported_at > now() - interval '1 hour' \
         ORDER BY dh.reported_at DESC \
         LIMIT 4",
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    for h in recent_hbs {
        activity.push(OverviewActivity {
            kind: "cluster-online".into(),
            text: format!("{} · {}", h.cluster_name, h.hostname),
            at: h.reported_at,
        });
    }

    activity.sort_by(|a, b| b.at.cmp(&a.at));
    activity.truncate(8);

    Ok(OverviewData {
        instances_online,
        instances_total,
        healthy_services_pct,
        healthy_services_count,
        healthy_services_total,
        active_rollouts,
        rollouts_completed_24h,
        open_staff_pings,
        activity,
    })
}

/// Format a relative duration like "4s ago" / "12m ago" / "3h ago".
fn relative_ago(at: DateTime<Utc>) -> String {
    let secs = (Utc::now() - at).num_seconds().max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86_400)
    }
}

fn activity_kind_to_pill(kind: &str) -> PillVariant {
    match kind {
        "rollout-completed" => PillVariant::Ok,
        "rollout-started"   => PillVariant::Accent,
        "rollout-other"     => PillVariant::Warn,
        "cluster-online"    => PillVariant::Info,
        _                   => PillVariant::Muted,
    }
}

#[component]
pub fn Overview() -> Element {
    use_topbar(t!("overview-title").to_string(), Some(t!("overview-subtitle").to_string()));

    let data = use_server_future(get_overview)?;

    match &*data.read() {
        Some(Ok(d)) => render_overview(d),
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}

fn render_overview(d: &OverviewData) -> Element {
    let online_value = format!("{} / {}", d.instances_online, d.instances_total);
    let services_value = format!("{:.1}%", d.healthy_services_pct);
    let services_sub = format!("{} / {}", d.healthy_services_count, d.healthy_services_total);
    let rollouts_value = d.active_rollouts.to_string();
    let rollouts_sub = format!("{} completed 24h", d.rollouts_completed_24h);
    let pings_value = d.open_staff_pings.to_string();

    let activity_items: Vec<ActivityItem> = d.activity.iter().map(|a| ActivityItem {
        kind: activity_kind_to_pill(&a.kind),
        text: a.text.clone(),
        time: relative_ago(a.at),
    }).collect();

    rsx! {
        // ── Hero ──
        PageHero {
            title: rsx! { {t!("nav-command-center")} },
            right: rsx! {
                div { class: "flex items-center gap-2 text-fg-muted text-xs",
                    Dot { variant: PillVariant::Ok }
                    "live"
                }
            },
            class: "mb-5",
        }

        // ── KPI strip ──
        div { class: "grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-4 mb-5",
            KpiCard {
                label: t!("overview-kpi-online"),
                value: online_value,
                color: ChartColor::Brand,
            }
            KpiCard {
                label: t!("overview-kpi-services"),
                value: services_value,
                delta: services_sub,
                delta_kind: ChartColor::Ok,
                color: ChartColor::Ok,
            }
            KpiCard {
                label: t!("overview-kpi-rollouts"),
                value: rollouts_value,
                delta: rollouts_sub,
                delta_kind: ChartColor::Info,
                color: ChartColor::Info,
            }
            KpiCard {
                label: t!("overview-kpi-pings"),
                value: pings_value,
                color: ChartColor::Warn,
            }
        }

        // ── Recent activity ──
        div { class: "mb-3 flex items-center justify-between",
            Kicker { {t!("overview-activity-title").to_uppercase()} }
            if d.activity.is_empty() {
                Pill { variant: PillVariant::Muted, "no recent activity" }
            }
        }
        if !d.activity.is_empty() {
            ActivityFeed { items: activity_items }
        }
    }
}
