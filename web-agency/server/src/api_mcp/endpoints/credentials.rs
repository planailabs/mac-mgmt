//! Credential endpoints.

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
pub struct CredentialRow {
    pub id: Uuid,
    pub name: String,
    pub credential_type: String,
    pub organization_name: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CredentialListInput {}

/// List credentials the caller may see (admins: all; else their orgs' + global).
#[api_mcp_dioxus_server(server = "list_credentials")]
pub async fn credential_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: CredentialListInput,
) -> Result<Vec<CredentialRow>, ApiError> {
    type Row = (
        Uuid,
        String,
        String,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
    );
    let base = "SELECT c.id, c.name, c.credential_type, o.name, c.created_at \
                FROM credentials c LEFT JOIN organizations o ON o.id = c.organization_id";

    let rows = match principal.read_filter() {
        None => {
            sqlx::query_as::<_, Row>(&format!("{base} ORDER BY c.created_at DESC"))
                .fetch_all(pool)
                .await
        }
        Some(org_ids) => {
            // Non-admins also see global (NULL org) credentials.
            sqlx::query_as::<_, Row>(&format!(
                "{base} WHERE c.organization_id = ANY($1) OR c.organization_id IS NULL \
                 ORDER BY c.created_at DESC"
            ))
            .bind(org_ids)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(super::internal)?;

    Ok(rows
        .into_iter()
        .map(
            |(id, name, credential_type, organization_name, created_at)| CredentialRow {
                id,
                name,
                credential_type,
                organization_name,
                created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
            },
        )
        .collect())
}
