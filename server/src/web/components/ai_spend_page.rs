//! AI spend dashboard (admin-only).
//!
//! Lands at `/ai-spend`. Renders the per-model breakdown (tokens
//! in/out, sessions, daily trend, estimated dollar spend for priced
//! models) aggregated by the `healer_spend` api-mcp endpoint
//! (`api_mcp::endpoints::healer`) over the last 30 days.

use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::healer::{
    HealerSpendInput, SPEND_WINDOW_DAYS, SpendData, get_healer_spend,
};
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Card, ChartColor, ErrorText, HelpText, Kicker, KpiCard, Pill, PillVariant, Sparkline, Td,
    TdMono, TdMuted, Th,
};

/// Compact token count: 1234 -> "1.2k", 3_400_000 -> "3.4M".
fn fmt_tokens(n: i64) -> String {
    let n = n as f64;
    if n >= 1e9 {
        format!("{:.1}B", n / 1e9)
    } else if n >= 1e6 {
        format!("{:.1}M", n / 1e6)
    } else if n >= 1e3 {
        format!("{:.1}k", n / 1e3)
    } else {
        format!("{n:.0}")
    }
}

fn fmt_usd(v: f64) -> String {
    if v >= 100.0 {
        format!("${v:.0}")
    } else {
        format!("${v:.2}")
    }
}

#[component]
pub fn AiSpend() -> Element {
    use_topbar(
        t!("ai-spend-title").to_string(),
        Some(t!("ai-spend-subtitle", days: SPEND_WINDOW_DAYS).to_string()),
    );

    let data =
        use_server_future(move || async move { get_healer_spend(HealerSpendInput {}).await })?;

    match &*data.read() {
        Some(Ok(d)) => render_spend(d),
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}

fn render_spend(d: &SpendData) -> Element {
    let cost_value = d.total_cost_usd.map(fmt_usd);
    let has_usage = d.total_tokens > 0;

    rsx! {
        // ── KPI strip ──
        div { class: "grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-4 mb-5",
            KpiCard {
                label: t!("ai-spend-kpi-total", days: d.days),
                value: fmt_tokens(d.total_tokens),
                data: d.daily_total.clone(),
                color: ChartColor::Brand,
            }
            KpiCard {
                label: t!("ai-spend-kpi-input"),
                value: fmt_tokens(d.total_input),
                color: ChartColor::Info,
            }
            KpiCard {
                label: t!("ai-spend-kpi-output"),
                value: fmt_tokens(d.total_output),
                color: ChartColor::Ok,
            }
            if let Some(cost) = cost_value {
                KpiCard {
                    label: t!("ai-spend-kpi-cost", days: d.days),
                    value: cost,
                    color: ChartColor::Warn,
                }
            } else {
                KpiCard {
                    label: t!("ai-spend-kpi-sessions"),
                    value: d.total_sessions.to_string(),
                    color: ChartColor::Warn,
                }
            }
        }

        // ── Per-model breakdown ──
        div { class: "mb-3",
            Kicker { {t!("ai-spend-models-title").to_uppercase()} }
        }
        if !has_usage {
            div { class: "mb-3",
                Pill { variant: PillVariant::Muted, {t!("ai-spend-empty")} }
            }
        }
        Card {
            div { class: "overflow-x-auto",
                table { class: "table",
                    thead { class: "thead",
                        tr {
                            Th { {t!("ai-spend-col-model")} }
                            Th { {t!("ai-spend-col-role")} }
                            Th { {t!("ai-spend-col-input")} }
                            Th { {t!("ai-spend-col-output")} }
                            Th { {t!("ai-spend-col-sessions")} }
                            Th { {t!("ai-spend-col-cost")} }
                            Th { {t!("ai-spend-col-trend", days: d.days)} }
                            Th { {t!("ai-spend-col-last-used")} }
                        }
                    }
                    tbody { class: "tbody",
                        for row in d.models.iter() {
                            tr { key: "{row.provider}:{row.model}",
                                Td {
                                    div { class: "font-medium text-fg-strong", "{row.name}" }
                                    div { class: "text-xs text-fg-muted font-mono",
                                        "{row.provider} · {row.model}"
                                    }
                                }
                                Td {
                                    div { class: "flex gap-1",
                                        if !row.validator || !row.configured {
                                            Pill { variant: PillVariant::Info, {t!("ai-spend-role-model")} }
                                        }
                                        if row.validator {
                                            Pill { variant: PillVariant::Accent, {t!("ai-spend-role-validator")} }
                                        }
                                        if !row.configured {
                                            Pill { variant: PillVariant::Muted, {t!("ai-spend-role-removed")} }
                                        }
                                    }
                                }
                                TdMono { {fmt_tokens(row.input_tokens)} }
                                TdMono { {fmt_tokens(row.output_tokens)} }
                                TdMono { "{row.sessions}" }
                                if let Some(cost) = row.cost_usd {
                                    TdMono { {fmt_usd(cost)} }
                                } else {
                                    TdMuted { "—" }
                                }
                                Td { class: "w-40 min-w-32",
                                    if row.input_tokens + row.output_tokens > 0 {
                                        Sparkline {
                                            data: row.daily.clone(),
                                            color: ChartColor::Brand,
                                            height: 24,
                                        }
                                    }
                                }
                                if let Some(at) = &row.last_used {
                                    TdMuted { "{at}" }
                                } else {
                                    TdMuted { {t!("ai-spend-never-used")} }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
