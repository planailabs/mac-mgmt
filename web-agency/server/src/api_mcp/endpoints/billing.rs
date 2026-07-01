//! Billing endpoints.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;

#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BillingRow {
    pub id: Uuid,
    pub entry_type: String,
    pub description: String,
    pub amount_cents: i32,
    pub currency: String,
    pub provider: Option<String>,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BillingListInput {}

type BillingSqlRow = (
    Uuid,
    String,
    String,
    i32,
    String,
    Option<String>,
    Option<chrono::NaiveDate>,
    Option<chrono::NaiveDate>,
    chrono::DateTime<chrono::Utc>,
);

/// List billing entries the caller may see. Admins see all; org members see
/// their orgs' entries only where the org has billing display enabled.
#[api_mcp_dioxus_server(server = "list_billing")]
pub async fn billing_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: BillingListInput,
) -> Result<Vec<BillingRow>, ApiError> {
    let rows = match principal.read_filter() {
        None => {
            sqlx::query_as::<_, BillingSqlRow>(
                "SELECT id, entry_type, description, amount_cents, currency, provider, \
                 period_start, period_end, created_at \
                 FROM billing_entries ORDER BY created_at DESC LIMIT 100",
            )
            .fetch_all(pool)
            .await
        }
        Some(org_ids) => {
            sqlx::query_as::<_, BillingSqlRow>(
                "SELECT b.id, b.entry_type, b.description, b.amount_cents, b.currency, b.provider, \
                 b.period_start, b.period_end, b.created_at \
                 FROM billing_entries b \
                 JOIN organizations o ON o.id = b.organization_id \
                 WHERE b.organization_id = ANY($1) AND o.show_billing = true \
                 ORDER BY b.created_at DESC LIMIT 100",
            )
            .bind(org_ids)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                entry_type,
                description,
                amount_cents,
                currency,
                provider,
                period_start,
                period_end,
                created_at,
            )| BillingRow {
                id,
                entry_type,
                description,
                amount_cents,
                currency,
                provider,
                period_start: period_start.map(|d| d.to_string()),
                period_end: period_end.map(|d| d.to_string()),
                created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
            },
        )
        .collect())
}
