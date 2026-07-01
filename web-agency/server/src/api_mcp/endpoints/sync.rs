//! Provider-sync endpoints.

use serde::{Deserialize, Serialize};

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;

#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SyncTriggerInput {}

/// Trigger a background provider sync (admin only). The sync runs
/// asynchronously; if one is already running, the new request is a no-op.
#[api_mcp_dioxus_server(server = "trigger_sync")]
pub async fn sync_trigger(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: SyncTriggerInput,
) -> Result<String, ApiError> {
    principal.require_admin()?;
    let pool2 = pool.clone();
    tokio::spawn(async move {
        crate::api::sync::trigger_sync(&pool2).await;
    });
    Ok("Sync triggered".to_string())
}
