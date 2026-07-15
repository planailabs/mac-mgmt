//! Healer read surface: staff pings (list + resolve).
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

// ── Registration ────────────────────────────────────────────────────────
