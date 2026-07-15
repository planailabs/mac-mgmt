//! AI helper endpoints: Anthropic-backed name/description generation for
//! skills, bundles, MCP servers and MCP bundles, plus the bulk-save step.
//!
//! The prompt building and Anthropic API call live in `crate::anthropic`
//! (they need the configured API key); the handlers here are the api-mcp
//! entry points.

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::anthropic::{EntityKind, GenerateContext, GeneratedNameDesc};

#[cfg(feature = "server")]
use super::internal;
#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

// ── DTOs ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GenerateNameDescInput {
    /// Which entity to generate for, with its describing context.
    pub context: GenerateContext,
    /// The entity's current name (may be empty).
    pub current_name: String,
    /// The entity's current description (may be empty).
    pub current_desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SaveGeneratedNameDescInput {
    /// Which table the entity lives in.
    pub entity_kind: EntityKind,
    /// Entity id (skill / bundle / MCP server / MCP bundle uuid).
    pub id: Uuid,
    pub name: String,
    pub description: String,
}

// ── Handlers ────────────────────────────────────────────────────────────

/// was: generate_name_desc() in anthropic.rs
#[api_mcp_dioxus_server(server = "generate_name_desc")]
pub async fn ai_generate_name_desc(
    _pool: &sqlx::PgPool,
    _p: &Principal,
    input: GenerateNameDescInput,
) -> Result<GeneratedNameDesc, ApiError> {
    crate::anthropic::generate_name_desc_impl(&input.context, &input.current_name, &input.current_desc)
        .await
        .map_err(internal)
}

/// was: save_generated_name_desc() in anthropic.rs
#[api_mcp_dioxus_server(server = "save_generated_name_desc")]
pub async fn ai_save_generated_name_desc(
    pool: &sqlx::PgPool,
    _p: &Principal,
    input: SaveGeneratedNameDescInput,
) -> Result<(), ApiError> {
    let query = match input.entity_kind {
        EntityKind::Skill => "UPDATE skills SET name = $1, description = $2 WHERE id = $3",
        EntityKind::Bundle => "UPDATE bundles SET name = $1, description = $2 WHERE id = $3",
        EntityKind::McpServer => "UPDATE mcp_servers SET name = $1, description = $2 WHERE id = $3",
        EntityKind::McpBundle => {
            "UPDATE mcp_server_bundles SET name = $1, description = $2 WHERE id = $3"
        }
    };

    sqlx::query(query)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;

    match input.entity_kind {
        EntityKind::Bundle => {
            crate::api::push::notify_skill_bundle_global(input.id).await;
        }
        EntityKind::McpBundle => {
            crate::api::push::notify_mcp_bundle_global(input.id).await;
        }
        _ => {}
    }
    Ok(())
}

// ── Spend dashboard ──────────────────────────────────────────────────────

/// How far back the spend dashboard aggregates.
pub const SPEND_WINDOW_DAYS: i32 = 30;

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AiSpendInput {}

/// Aggregated AI token usage (healer + chat) over the spend window.
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

/// Admin-only AI spend dashboard data (web: components/ai_spend_page.rs).
#[api_mcp_dioxus_server(server = "get_ai_spend")]
pub async fn ai_spend(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: AiSpendInput,
) -> Result<SpendData, ApiError> {
    // Admin-only: the dashboard aggregates spend across the entire fleet
    // (all clusters, all chat users), so no per-org cluster filtering.
    p.require_admin()?;

    let days = SPEND_WINDOW_DAYS;

    // The unified chat_token_events table covers every agent domain
    // (healer + chat sessions alike), so one query each suffices.
    #[derive(sqlx::FromRow)]
    struct SummaryRow {
        provider: String,
        model: String,
        input_tokens: i64,
        output_tokens: i64,
        sessions: i64,
        last_used: chrono::DateTime<chrono::Utc>,
    }
    let summaries: Vec<SummaryRow> = sqlx::query_as(
        "SELECT e.provider, e.model, \
         SUM(e.input_tokens)::BIGINT AS input_tokens, \
         SUM(e.output_tokens)::BIGINT AS output_tokens, \
         COUNT(DISTINCT e.session_id) AS sessions, \
         MAX(e.created_at) AS last_used \
         FROM chat_token_events e \
         WHERE e.created_at > now() - make_interval(days => $1) \
         GROUP BY e.provider, e.model",
    )
    .bind(days)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    #[derive(sqlx::FromRow)]
    struct DailyRow {
        provider: String,
        model: String,
        day: chrono::DateTime<chrono::Utc>,
        tokens: i64,
    }
    let daily_rows: Vec<DailyRow> = sqlx::query_as(
        "SELECT e.provider, e.model, \
         date_trunc('day', e.created_at) AS day, \
         SUM(e.input_tokens + e.output_tokens)::BIGINT AS tokens \
         FROM chat_token_events e \
         WHERE e.created_at > now() - make_interval(days => $1) \
         GROUP BY 1, 2, 3",
    )
    .bind(days)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    // Day axis, oldest first, ending today (UTC).
    let today = chrono::Utc::now().date_naive();
    let axis: Vec<chrono::NaiveDate> = (0..days)
        .rev()
        .map(|off| today - chrono::Duration::days(off as i64))
        .collect();
    let day_index = |d: chrono::NaiveDate| axis.iter().position(|a| *a == d);

    // Configured entries: models first, then validators (a model can be both).
    let cfg = crate::config::config();
    let healer_cfg = &cfg.healer;
    let mut model_entries = if healer_cfg.models.is_empty() {
        crate::config::default_healer_models()
    } else {
        healer_cfg.models.clone()
    };
    // Chat-specific model entries (pricing/config) join the same table.
    model_entries.extend(cfg.chat.models.iter().cloned());
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
