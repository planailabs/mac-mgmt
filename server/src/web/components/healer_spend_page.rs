//! Healer spend dashboard.
//!
//! Lands at `/healer-spend`. Aggregates `healer_token_events` into a
//! per-model breakdown (tokens in/out, sessions, daily trend) over the
//! last 30 days. Models carrying `input_cost_per_mtok` /
//! `output_cost_per_mtok` in the healer config additionally get an
//! estimated dollar spend; unpriced models show token counts only.
//! Every configured model (including validators) is listed even with
//! zero usage, so newly added models are visible immediately.

use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Card, ChartColor, ErrorText, HelpText, Kicker, KpiCard, Pill, PillVariant, Sparkline, Td,
    TdMono, TdMuted, Th,
};
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

/// How far back the dashboard aggregates.
const SPEND_WINDOW_DAYS: i32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SpendData {
    days: i32,
    total_tokens: i64,
    total_input: i64,
    total_output: i64,
    total_sessions: i64,
    /// Sum over priced models only; `None` when no model is priced.
    total_cost_usd: Option<f64>,
    /// Total tokens per day, oldest first, one entry per day in the window.
    daily_total: Vec<f64>,
    models: Vec<ModelSpendRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelSpendRow {
    /// Display name from the healer config; falls back to the model id for
    /// usage recorded under a model that has since left the config.
    name: String,
    provider: String,
    model: String,
    input_tokens: i64,
    output_tokens: i64,
    sessions: i64,
    /// Total tokens per day, oldest first (sparkline).
    daily: Vec<f64>,
    /// Estimated spend; `None` when the model has no configured prices.
    cost_usd: Option<f64>,
    /// Still present in the healer config (`models` or `validator_models`).
    configured: bool,
    /// Present in `validator_models`.
    validator: bool,
    last_used: Option<String>,
}

#[server]
async fn get_healer_spend() -> Result<SpendData, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let accessible = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let days = SPEND_WINDOW_DAYS;

    #[derive(sqlx::FromRow)]
    struct SummaryRow {
        provider: String,
        model: String,
        input_tokens: i64,
        output_tokens: i64,
        sessions: i64,
        last_used: chrono::DateTime<chrono::Utc>,
    }
    let summary_sql_base = "SELECT e.provider, e.model, \
         SUM(e.input_tokens)::BIGINT AS input_tokens, \
         SUM(e.output_tokens)::BIGINT AS output_tokens, \
         COUNT(DISTINCT e.session_id) AS sessions, \
         MAX(e.created_at) AS last_used \
         FROM healer_token_events e \
         JOIN healer_sessions s ON s.id = e.session_id \
         WHERE e.created_at > now() - make_interval(days => $1)";
    let summaries: Vec<SummaryRow> = match accessible.as_ref() {
        Some(ids) => sqlx::query_as(&format!(
            "{summary_sql_base} AND s.cluster_id = ANY($2) GROUP BY e.provider, e.model"
        ))
        .bind(days)
        .bind(ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?,
        None => sqlx::query_as(&format!("{summary_sql_base} GROUP BY e.provider, e.model"))
            .bind(days)
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?,
    };

    #[derive(sqlx::FromRow)]
    struct DailyRow {
        provider: String,
        model: String,
        day: chrono::DateTime<chrono::Utc>,
        tokens: i64,
    }
    let daily_sql_base = "SELECT e.provider, e.model, \
         date_trunc('day', e.created_at) AS day, \
         SUM(e.input_tokens + e.output_tokens)::BIGINT AS tokens \
         FROM healer_token_events e \
         JOIN healer_sessions s ON s.id = e.session_id \
         WHERE e.created_at > now() - make_interval(days => $1)";
    let daily_rows: Vec<DailyRow> = match accessible.as_ref() {
        Some(ids) => sqlx::query_as(&format!(
            "{daily_sql_base} AND s.cluster_id = ANY($2) GROUP BY 1, 2, 3"
        ))
        .bind(days)
        .bind(ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?,
        None => sqlx::query_as(&format!("{daily_sql_base} GROUP BY 1, 2, 3"))
            .bind(days)
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?,
    };

    // Day axis, oldest first, ending today (UTC).
    let today = chrono::Utc::now().date_naive();
    let axis: Vec<chrono::NaiveDate> = (0..days)
        .rev()
        .map(|off| today - chrono::Duration::days(off as i64))
        .collect();
    let day_index = |d: chrono::NaiveDate| axis.iter().position(|a| *a == d);

    // Configured entries: models first, then validators (a model can be both).
    let healer_cfg = &crate::config::config().healer;
    let model_entries = if healer_cfg.models.is_empty() {
        crate::config::default_healer_models()
    } else {
        healer_cfg.models.clone()
    };
    let validator_entries = if healer_cfg.validator_models.is_empty() {
        crate::config::default_validator_models()
    } else {
        healer_cfg.validator_models.clone()
    };

    let mut rows: Vec<ModelSpendRow> = Vec::new();
    let find_row =
        |rows: &mut Vec<ModelSpendRow>, provider: &str, model: &str| -> Option<usize> {
            rows.iter()
                .position(|r| r.provider == provider && r.model == model)
        };

    for (entries, validator) in [(&model_entries, false), (&validator_entries, true)] {
        for e in entries.iter() {
            if let Some(i) = find_row(&mut rows, &e.provider, &e.model) {
                rows[i].validator |= validator;
                continue;
            }
            rows.push(ModelSpendRow {
                name: e.display_name(),
                provider: e.provider.clone(),
                model: e.model.clone(),
                input_tokens: 0,
                output_tokens: 0,
                sessions: 0,
                daily: vec![0.0; days as usize],
                cost_usd: None,
                configured: true,
                validator,
                last_used: None,
            });
        }
    }

    // Price lookup from config (either list; first match wins).
    let price_of = |provider: &str, model: &str| -> Option<(f64, f64)> {
        model_entries
            .iter()
            .chain(validator_entries.iter())
            .find(|e| e.provider == provider && e.model == model)
            .and_then(|e| Some((e.input_cost_per_mtok?, e.output_cost_per_mtok?)))
    };

    for s in &summaries {
        let i = match find_row(&mut rows, &s.provider, &s.model) {
            Some(i) => i,
            None => {
                // Usage recorded under a model no longer in the config.
                rows.push(ModelSpendRow {
                    name: s.model.clone(),
                    provider: s.provider.clone(),
                    model: s.model.clone(),
                    input_tokens: 0,
                    output_tokens: 0,
                    sessions: 0,
                    daily: vec![0.0; days as usize],
                    cost_usd: None,
                    configured: false,
                    validator: false,
                    last_used: None,
                });
                rows.len() - 1
            }
        };
        rows[i].input_tokens = s.input_tokens;
        rows[i].output_tokens = s.output_tokens;
        rows[i].sessions = s.sessions;
        rows[i].last_used = Some(s.last_used.format("%Y-%m-%d %H:%M").to_string());
        rows[i].cost_usd = price_of(&s.provider, &s.model).map(|(inp, outp)| {
            (s.input_tokens as f64 * inp + s.output_tokens as f64 * outp) / 1_000_000.0
        });
    }

    let mut daily_total = vec![0.0; days as usize];
    for d in &daily_rows {
        let Some(di) = day_index(d.day.date_naive()) else {
            continue;
        };
        daily_total[di] += d.tokens as f64;
        if let Some(i) = find_row(&mut rows, &d.provider, &d.model) {
            rows[i].daily[di] += d.tokens as f64;
        }
    }

    // Busiest models first; unused configured models trail alphabetically.
    rows.sort_by(|a, b| {
        (b.input_tokens + b.output_tokens)
            .cmp(&(a.input_tokens + a.output_tokens))
            .then_with(|| a.name.cmp(&b.name))
    });

    let total_input: i64 = rows.iter().map(|r| r.input_tokens).sum();
    let total_output: i64 = rows.iter().map(|r| r.output_tokens).sum();
    let total_sessions: i64 = summaries.iter().map(|s| s.sessions).sum();
    let priced: Vec<f64> = rows.iter().filter_map(|r| r.cost_usd).collect();
    let total_cost_usd = (!priced.is_empty()).then(|| priced.iter().sum());

    Ok(SpendData {
        days,
        total_tokens: total_input + total_output,
        total_input,
        total_output,
        total_sessions,
        total_cost_usd,
        daily_total,
        models: rows,
    })
}

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
pub fn HealerSpend() -> Element {
    use_topbar(
        t!("healer-spend-title").to_string(),
        Some(t!("healer-spend-subtitle", days: SPEND_WINDOW_DAYS).to_string()),
    );

    let data = use_server_future(get_healer_spend)?;

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
                label: t!("healer-spend-kpi-total", days: d.days),
                value: fmt_tokens(d.total_tokens),
                data: d.daily_total.clone(),
                color: ChartColor::Brand,
            }
            KpiCard {
                label: t!("healer-spend-kpi-input"),
                value: fmt_tokens(d.total_input),
                color: ChartColor::Info,
            }
            KpiCard {
                label: t!("healer-spend-kpi-output"),
                value: fmt_tokens(d.total_output),
                color: ChartColor::Ok,
            }
            if let Some(cost) = cost_value {
                KpiCard {
                    label: t!("healer-spend-kpi-cost", days: d.days),
                    value: cost,
                    color: ChartColor::Warn,
                }
            } else {
                KpiCard {
                    label: t!("healer-spend-kpi-sessions"),
                    value: d.total_sessions.to_string(),
                    color: ChartColor::Warn,
                }
            }
        }

        // ── Per-model breakdown ──
        div { class: "mb-3",
            Kicker { {t!("healer-spend-models-title").to_uppercase()} }
        }
        if !has_usage {
            div { class: "mb-3",
                Pill { variant: PillVariant::Muted, {t!("healer-spend-empty")} }
            }
        }
        Card {
            div { class: "overflow-x-auto",
                table { class: "table",
                    thead { class: "thead",
                        tr {
                            Th { {t!("healer-spend-col-model")} }
                            Th { {t!("healer-spend-col-role")} }
                            Th { {t!("healer-spend-col-input")} }
                            Th { {t!("healer-spend-col-output")} }
                            Th { {t!("healer-spend-col-sessions")} }
                            Th { {t!("healer-spend-col-cost")} }
                            Th { {t!("healer-spend-col-trend", days: d.days)} }
                            Th { {t!("healer-spend-col-last-used")} }
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
                                            Pill { variant: PillVariant::Info, {t!("healer-spend-role-model")} }
                                        }
                                        if row.validator {
                                            Pill { variant: PillVariant::Accent, {t!("healer-spend-role-validator")} }
                                        }
                                        if !row.configured {
                                            Pill { variant: PillVariant::Muted, {t!("healer-spend-role-removed")} }
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
                                    TdMuted { {t!("healer-spend-never-used")} }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
