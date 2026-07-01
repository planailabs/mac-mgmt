//! Domain endpoints (HTTP `/api/v1/domains` + MCP `domain_*` tools).
//!
//! Handlers are written once here; the macro-generated `#[server]` wrappers let
//! the Dioxus UI call the same logic.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;

// Glue the macro-generated `#[server]` wrappers call (server-only).
#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

// ── DTOs (shared client + server) ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainRow {
    pub id: Uuid,
    pub name: String,
    pub registrar_type: Option<String>,
    pub ssl_mode: String,
    pub dnssec_enabled: bool,
    pub cloudflare_zone_id: Option<String>,
    pub expires_at: Option<String>,
    pub organization_name: String,
    /// Whether nameservers can be set at the registrar (spaceship + credential).
    pub can_set_nameservers: bool,
    /// Cached NS match status: None = unknown, Some(true) = match, Some(false) = mismatch.
    pub ns_ok: Option<bool>,
    pub has_webspace: bool,
    pub ai_bots_protection: Option<String>,
    pub expires_soon: bool,
}

/// Input for `domain_list`. Results are scoped by the caller's principal;
/// no fields are required.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainListInput {}

// ── Endpoints ─────────────────────────────────────────────────────────

/// List domains the caller may see (admins: all; otherwise their orgs').
#[api_mcp_dioxus_server(server = "list_domains")]
pub async fn domain_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: DomainListInput,
) -> Result<Vec<DomainRow>, ApiError> {
    type Row = (
        Uuid,
        String,
        Option<String>,
        String,
        bool,
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
        String,
        Option<String>,
        Option<Uuid>,
        Option<bool>,
        bool,
        Option<String>,
        bool,
    );

    let query = "SELECT d.id, d.name, d.registrar_type, d.ssl_mode, d.dnssec_enabled, d.cloudflare_zone_id, d.expires_at, o.name, \
         d.registrar_type, d.registrar_credential_id, d.ns_ok, \
         EXISTS(SELECT 1 FROM webspace_host_domains whd WHERE whd.domain_id = d.id) AS has_webspace, \
         d.ai_bots_protection, \
         (d.expires_at IS NOT NULL AND d.expires_at < now() + interval '30 days') AS expires_soon \
         FROM domains d JOIN organizations o ON o.id = d.organization_id";

    let rows = match principal.read_filter() {
        None => {
            sqlx::query_as::<_, Row>(&format!("{query} ORDER BY d.name"))
                .fetch_all(pool)
                .await
        }
        Some(org_ids) => {
            sqlx::query_as::<_, Row>(&format!(
                "{query} WHERE d.organization_id = ANY($1) ORDER BY d.name"
            ))
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
                name,
                registrar_type,
                ssl_mode,
                dnssec_enabled,
                cloudflare_zone_id,
                expires_at,
                organization_name,
                reg_type,
                reg_cred_id,
                ns_ok,
                has_webspace,
                ai_bots_protection,
                expires_soon,
            )| DomainRow {
                id,
                name,
                registrar_type,
                ssl_mode,
                dnssec_enabled,
                cloudflare_zone_id,
                expires_at: expires_at.map(|d| d.format("%Y-%m-%d").to_string()),
                organization_name,
                can_set_nameservers: reg_type.as_deref() == Some("spaceship")
                    && reg_cred_id.is_some(),
                ns_ok,
                has_webspace,
                ai_bots_protection,
                expires_soon,
            },
        )
        .collect())
}
