//! Basic-auth list endpoints.

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
pub struct BasicAuthListRow {
    pub id: Uuid,
    pub name: String,
    pub organization_name: String,
    pub credential_count: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BasicAuthListInput {}

/// List basic-auth lists the caller may see (admins: all; else their orgs').
#[api_mcp_dioxus_server(server = "list_basic_auth_lists")]
pub async fn basic_auth_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: BasicAuthListInput,
) -> Result<Vec<BasicAuthListRow>, ApiError> {
    let base = "SELECT b.id, b.name, o.name, \
                (SELECT count(*) FROM basic_auth_credentials c WHERE c.list_id = b.id) \
                FROM basic_auth_lists b JOIN organizations o ON o.id = b.organization_id";

    let rows = match principal.read_filter() {
        None => {
            sqlx::query_as::<_, (Uuid, String, String, i64)>(&format!("{base} ORDER BY b.name"))
                .fetch_all(pool)
                .await
        }
        Some(org_ids) => {
            sqlx::query_as::<_, (Uuid, String, String, i64)>(&format!(
                "{base} WHERE b.organization_id = ANY($1) ORDER BY b.name"
            ))
            .bind(org_ids)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|(id, name, organization_name, credential_count)| BasicAuthListRow {
            id,
            name,
            organization_name,
            credential_count,
        })
        .collect())
}
