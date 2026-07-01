//! Domain-contact endpoints.

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
pub struct ContactRow {
    pub id: Uuid,
    pub label: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub country: String,
    pub spaceship_synced: bool,
    pub organization_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ContactListInput {}

/// List domain contacts the caller may see (admins: all; else their orgs').
#[api_mcp_dioxus_server(server = "list_contacts")]
pub async fn contact_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: ContactListInput,
) -> Result<Vec<ContactRow>, ApiError> {
    type Row = (
        Uuid,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
    );
    let base = "SELECT c.id, c.label, c.first_name, c.last_name, c.email, c.country, \
                c.spaceship_contact_id, o.name \
                FROM domain_contacts c JOIN organizations o ON o.id = c.organization_id";

    let rows = match principal.read_filter() {
        None => {
            sqlx::query_as::<_, Row>(&format!("{base} ORDER BY c.label"))
                .fetch_all(pool)
                .await
        }
        Some(org_ids) => {
            sqlx::query_as::<_, Row>(&format!(
                "{base} WHERE c.organization_id = ANY($1) ORDER BY c.label"
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
            |(id, label, first_name, last_name, email, country, spaceship_id, organization_name)| {
                ContactRow {
                    id,
                    label,
                    first_name,
                    last_name,
                    email,
                    country,
                    spaceship_synced: spaceship_id.is_some(),
                    organization_name,
                }
            },
        )
        .collect())
}
