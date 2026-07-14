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

// ── Registration ────────────────────────────────────────────────────────

#[cfg(feature = "server")]
pub fn register(reg: &mut plan_ai_api_mcp::Registry<sqlx::PgPool>) {
    use plan_ai_api_mcp::{OnItem, Risk};
    let mut a = reg.resource("ai", "ai", "AI");
    a.custom(
        "generate_name_desc",
        Risk::Mutating,
        OnItem::No,
        "Generate a concise name + description for a skill, bundle, MCP server, or MCP bundle via the Anthropic API (external call that costs money; nothing is persisted).",
        |pool: sqlx::PgPool, p, input: GenerateNameDescInput| async move {
            ai_generate_name_desc(&pool, &p, input).await
        },
    );
    a.custom(
        "save_generated_name_desc",
        Risk::Mutating,
        OnItem::No,
        "Save a generated name + description onto the given entity (skill, bundle, MCP server, or MCP bundle); bundle updates are pushed to connected daemons.",
        |pool: sqlx::PgPool, p, input: SaveGeneratedNameDescInput| async move {
            ai_save_generated_name_desc(&pool, &p, input).await
        },
    );
}
