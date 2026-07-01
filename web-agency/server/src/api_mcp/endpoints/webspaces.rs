//! Webspace (folder) endpoints.

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
pub struct WebspaceRow {
    pub id: Uuid,
    pub name: String,
    pub host_id: Uuid,
    pub host_name: String,
    pub host_kind: String,
    pub path_prefix: String,
    pub hosting_type: String,
    pub runtime: Option<String>,
    pub local_status: Option<String>,
    pub auth_mode: String,
    pub organization_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceDeleteInput {
    pub id: Uuid,
}

/// List webspace folders the caller may see (admins: all; else their orgs').
#[api_mcp_dioxus_server(server = "list_webspaces")]
pub async fn webspace_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: WebspaceListInput,
) -> Result<Vec<WebspaceRow>, ApiError> {
    type Row = (
        Uuid,
        String,
        Uuid,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        String,
    );
    let query = "SELECT w.id, w.name, h.id, h.name, h.kind, w.path_prefix, w.hosting_type, w.runtime, w.local_status, w.auth_mode, o.name \
                 FROM webspaces w \
                 JOIN webspace_hosts h ON h.id = w.webspace_host_id \
                 JOIN organizations o ON o.id = w.organization_id";

    let rows = match principal.read_filter() {
        None => {
            sqlx::query_as::<_, Row>(&format!("{query} ORDER BY h.name, w.path_prefix"))
                .fetch_all(pool)
                .await
        }
        Some(org_ids) => {
            sqlx::query_as::<_, Row>(&format!(
                "{query} WHERE w.organization_id = ANY($1) ORDER BY h.name, w.path_prefix"
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
            |(
                id,
                name,
                host_id,
                host_name,
                host_kind,
                path_prefix,
                hosting_type,
                runtime,
                local_status,
                auth_mode,
                organization_name,
            )| WebspaceRow {
                id,
                name,
                host_id,
                host_name,
                host_kind,
                path_prefix,
                hosting_type,
                runtime,
                local_status,
                auth_mode,
                organization_name,
            },
        )
        .collect())
}

/// Delete a webspace folder (requires org write). Removes its deployments and
/// notifies the proxy.
#[api_mcp_dioxus_server(server = "delete_webspace")]
pub async fn webspace_delete(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: WebspaceDeleteInput,
) -> Result<(), ApiError> {
    let org_id = super::owning_org(pool, "webspaces", input.id, "webspace").await?;
    principal.require_write(&org_id)?;

    sqlx::query("DELETE FROM deployments WHERE webspace_id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    sqlx::query("DELETE FROM webspaces WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}
