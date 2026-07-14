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

use crate::api_mcp::endpoints::fleet::{OverviewData, OverviewInput, get_overview};
use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    ActivityFeed, ActivityItem, ChartColor, Dot, ErrorText, HelpText, Kicker, KpiCard, PageHero,
    Pill, PillVariant,
};

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
        "rollout-started" => PillVariant::Accent,
        "rollout-other" => PillVariant::Warn,
        "cluster-online" => PillVariant::Info,
        _ => PillVariant::Muted,
    }
}

#[component]
pub fn Overview() -> Element {
    use_topbar(
        t!("overview-title").to_string(),
        Some(t!("overview-subtitle").to_string()),
    );

    let data = use_server_future(|| get_overview(OverviewInput {}))?;

    match &*data.read() {
        Some(Ok(d)) => render_overview(d),
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}

fn render_overview(d: &OverviewData) -> Element {
    let online_value = format!("{} / {}", d.instances_online, d.instances_total);
    let services_value = format!("{:.1}%", d.healthy_services_pct);
    let services_sub = format!(
        "{} / {}",
        d.healthy_services_count, d.healthy_services_total
    );
    let rollouts_value = d.active_rollouts.to_string();
    let rollouts_sub = format!("{} completed 24h", d.rollouts_completed_24h);
    let pings_value = d.open_staff_pings.to_string();
    let signals_value = d.open_failure_signals.to_string();

    let activity_items: Vec<ActivityItem> = d
        .activity
        .iter()
        .map(|a| ActivityItem {
            kind: activity_kind_to_pill(&a.kind),
            text: a.text.clone(),
            time: relative_ago(a.at),
        })
        .collect();

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
        // Each card drills down to the page that owns the metric:
        // online → Fleet, healthy services → Fleet, active rollouts →
        // Rollouts, open pings → Staff Pings.
        div { class: "grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-5 gap-4 mb-5",
            KpiCard {
                label: t!("overview-kpi-online"),
                value: online_value,
                color: ChartColor::Brand,
                to: Route::FleetDashboard { stage_id: None },
            }
            KpiCard {
                label: t!("overview-kpi-services"),
                value: services_value,
                delta: services_sub,
                delta_kind: ChartColor::Ok,
                color: ChartColor::Ok,
                to: Route::FleetDashboard { stage_id: None },
            }
            KpiCard {
                label: t!("overview-kpi-rollouts"),
                value: rollouts_value,
                delta: rollouts_sub,
                delta_kind: ChartColor::Info,
                color: ChartColor::Info,
                to: Route::RolloutList {},
            }
            KpiCard {
                label: t!("overview-kpi-pings"),
                value: pings_value,
                color: ChartColor::Warn,
                to: Route::StaffPings {},
            }
            KpiCard {
                label: t!("overview-kpi-signals"),
                value: signals_value,
                color: ChartColor::Bad,
                to: Route::FleetDashboard { stage_id: None },
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
