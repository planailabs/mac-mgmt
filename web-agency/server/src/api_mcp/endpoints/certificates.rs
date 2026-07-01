//! Certificate endpoints.

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
pub struct CertRow {
    pub id: Uuid,
    pub domain: String,
    pub issuer: String,
    pub acme_status: String,
    pub not_before: String,
    pub not_after: String,
    pub expires_soon: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CertListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CertIssueInput {
    pub domain: String,
}

/// List TLS certificates (admin only).
#[api_mcp_dioxus_server(server = "list_certificates")]
pub async fn certificate_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: CertListInput,
) -> Result<Vec<CertRow>, ApiError> {
    principal.require_admin()?;
    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            String,
            chrono::DateTime<chrono::Utc>,
            chrono::DateTime<chrono::Utc>,
            bool,
            Option<String>,
        ),
    >(
        "SELECT id, domain, issuer, acme_status, not_before, not_after, \
         not_after < now() + interval '30 days', last_error \
         FROM certificates ORDER BY domain",
    )
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

    Ok(rows
        .into_iter()
        .map(
            |(id, domain, issuer, acme_status, not_before, not_after, expires_soon, last_error)| {
                CertRow {
                    id,
                    domain,
                    issuer,
                    acme_status,
                    not_before: not_before.format("%Y-%m-%d").to_string(),
                    not_after: not_after.format("%Y-%m-%d").to_string(),
                    expires_soon,
                    last_error,
                }
            },
        )
        .collect())
}

/// Issue (or reissue) the ACME certificate for a domain (admin only). Returns
/// a human-readable status string ("issued" or "error: ...").
#[api_mcp_dioxus_server(server = "reissue_cert")]
pub async fn certificate_issue(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: CertIssueInput,
) -> Result<String, ApiError> {
    principal.require_admin()?;

    match crate::api::acme::issue_cert(pool, &input.domain).await {
        Ok(()) => Ok("issued".into()),
        Err(e) => Ok(format!("error: {e}")),
    }
}
