//! Healer read surface: staff pings (list + resolve) and the spend
//! dashboard aggregation over `healer_token_events`.
//!
//! Live-session driving endpoints (start/cancel/pause/resume) intentionally
//! remain legacy `#[server]` functions in `web/components/healer_page.rs`.

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "server")]
use super::internal;
#[cfg(feature = "server")]
use crate::api_mcp::access;
#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

/// How far back the spend dashboard aggregates.
pub const SPEND_WINDOW_DAYS: i32 = 30;

// ── DTOs ────────────────────────────────────────────────────────────────

/// One healer staff ping as shown on the staff-pings page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StaffPingRow {
    pub id: String,
    pub session_id: String,
    pub instance_id: String,
    pub cluster_name: String,
    pub category: String,
    pub message: String,
    pub resolved: bool,
    pub resolved_by: Option<String>,
    /// Formatted as `%Y-%m-%d %H:%M` (UTC).
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StaffPingsListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PingResolveInput {
    /// Staff-ping id to mark resolved.
    pub ping_id: Uuid,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HealerSpendInput {}

/// Aggregated healer token usage over the spend window.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SpendData {
    pub days: i32,
    pub total_tokens: i64,
    pub total_input: i64,
    pub total_output: i64,
    pub total_sessions: i64,
    /// Sum over priced models only; `None` when no model is priced.
    pub total_cost_usd: Option<f64>,
    /// Total tokens per day, oldest first, one entry per day in the window.
    pub daily_total: Vec<f64>,
    pub models: Vec<ModelSpendRow>,
}

/// Per-model usage row for the spend dashboard.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelSpendRow {
    /// Display name from the healer config; falls back to the model id for
    /// usage recorded under a model that has since left the config.
    pub name: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub sessions: i64,
    /// Total tokens per day, oldest first (sparkline).
    pub daily: Vec<f64>,
    /// Estimated spend; `None` when the model has no configured prices.
    pub cost_usd: Option<f64>,
    /// Still present in the healer config (`models` or `validator_models`).
    pub configured: bool,
    /// Present in `validator_models`.
    pub validator: bool,
    pub last_used: Option<String>,
}

// ── Staff pings ─────────────────────────────────────────────────────────

/// was: list_all_staff_pings() in web/components/staff_pings_page.rs
#[api_mcp_dioxus_server(server = "list_all_staff_pings")]
pub async fn healer_pings_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: StaffPingsListInput,
) -> Result<Vec<StaffPingRow>, ApiError> {
    let accessible = access::accessible_cluster_ids(pool, p).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        session_id: Uuid,
        instance_id: String,
        cluster_name: Option<String>,
        category: String,
        message: String,
        resolved: bool,
        resolved_by: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let rows = match accessible {
        None => {
            // Admin: all pings
            sqlx::query_as::<_, Row>(
                "SELECT p.id, p.session_id, p.instance_id, c.name AS cluster_name, \
                        p.category, p.message, p.resolved, p.resolved_by, p.created_at \
                 FROM healer_staff_pings p \
                 JOIN clusters c ON c.id = p.cluster_id \
                 ORDER BY p.resolved ASC, p.created_at DESC \
                 LIMIT 200",
            )
            .fetch_all(pool)
            .await
            .map_err(internal)?
        }
        Some(ids) => {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            sqlx::query_as::<_, Row>(
                "SELECT p.id, p.session_id, p.instance_id, c.name AS cluster_name, \
                        p.category, p.message, p.resolved, p.resolved_by, p.created_at \
                 FROM healer_staff_pings p \
                 JOIN clusters c ON c.id = p.cluster_id \
                 WHERE p.cluster_id = ANY($1) \
                 ORDER BY p.resolved ASC, p.created_at DESC \
                 LIMIT 200",
            )
            .bind(&ids)
            .fetch_all(pool)
            .await
            .map_err(internal)?
        }
    };

    Ok(rows
        .into_iter()
        .map(|r| StaffPingRow {
            id: r.id.to_string(),
            session_id: r.session_id.to_string(),
            instance_id: r.instance_id,
            cluster_name: r.cluster_name.unwrap_or_default(),
            category: r.category,
            message: r.message,
            resolved: r.resolved,
            resolved_by: r.resolved_by,
            created_at: r.created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect())
}

/// was: resolve_ping() in web/components/staff_pings_page.rs
#[api_mcp_dioxus_server(server = "resolve_ping")]
pub async fn healer_ping_resolve(
    _pool: &sqlx::PgPool,
    p: &Principal,
    input: PingResolveInput,
) -> Result<(), ApiError> {
    let healer = crate::server_state::healer_state()
        .ok_or_else(|| ApiError::internal("healer not initialized"))?;
    healer
        .store()
        .resolve_staff_ping(input.ping_id, &p.subject)
        .await
        .map_err(internal)
}

// ── Spend dashboard ─────────────────────────────────────────────────────

/// was: get_healer_spend() in web/components/healer_spend_page.rs
#[api_mcp_dioxus_server(server = "get_healer_spend")]
pub async fn healer_spend(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: HealerSpendInput,
) -> Result<SpendData, ApiError> {
    let accessible = access::accessible_cluster_ids(pool, p).await?;

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
        .fetch_all(pool)
        .await
        .map_err(internal)?,
        None => sqlx::query_as(&format!("{summary_sql_base} GROUP BY e.provider, e.model"))
            .bind(days)
            .fetch_all(pool)
            .await
            .map_err(internal)?,
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
        .fetch_all(pool)
        .await
        .map_err(internal)?,
        None => sqlx::query_as(&format!("{daily_sql_base} GROUP BY 1, 2, 3"))
            .bind(days)
            .fetch_all(pool)
            .await
            .map_err(internal)?,
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
    let find_row = |rows: &mut Vec<ModelSpendRow>, provider: &str, model: &str| -> Option<usize> {
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

// ── Registration ────────────────────────────────────────────────────────

#[cfg(feature = "server")]
pub fn register(reg: &mut plan_ai_api_mcp::Registry<sqlx::PgPool>) {
    use plan_ai_api_mcp::{OnItem, Risk};
    let mut h = reg.resource("healer", "healer", "Healer");
    h.custom(
        "pings_list",
        Risk::ReadOnly,
        OnItem::No,
        "List healer staff pings across clusters visible to the caller (unresolved first, newest first, max 200).",
        |pool: sqlx::PgPool, p, input: StaffPingsListInput| async move {
            healer_pings_list(&pool, &p, input).await
        },
    );
    h.custom(
        "ping_resolve",
        Risk::Mutating,
        OnItem::No,
        "Mark a healer staff ping as resolved by the caller.",
        |pool: sqlx::PgPool, p, input: PingResolveInput| async move {
            healer_ping_resolve(&pool, &p, input).await
        },
    );
    h.custom(
        "spend",
        Risk::ReadOnly,
        OnItem::No,
        "Aggregate healer token usage over the last 30 days into a per-model breakdown (tokens in/out, sessions, daily trend, estimated USD for priced models), scoped to clusters visible to the caller.",
        |pool: sqlx::PgPool, p, input: HealerSpendInput| async move {
            healer_spend(&pool, &p, input).await
        },
    );
}
