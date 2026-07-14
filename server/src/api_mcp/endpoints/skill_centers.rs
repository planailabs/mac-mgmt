//! Skill-center endpoints: CRUD over the `skill_centers` table plus the
//! cached-catalog summary and an on-demand catalog sync. All admin-only.

use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "server")]
use super::internal;
#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

// ── DTOs ────────────────────────────────────────────────────────────────

/// A skill center as shown in the list/detail pages. The federation token is
/// write-only and never returned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillCenterRow {
    pub id: String,
    pub name: String,
    pub url: String,
    pub priority: i32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(feature = "server")]
#[derive(sqlx::FromRow)]
struct SkillCenterDbRow {
    id: Uuid,
    name: String,
    url: String,
    priority: i32,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[cfg(feature = "server")]
impl From<SkillCenterDbRow> for SkillCenterRow {
    fn from(r: SkillCenterDbRow) -> Self {
        SkillCenterRow {
            id: r.id.to_string(),
            name: r.name,
            url: r.url,
            priority: r.priority,
            enabled: r.enabled,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[cfg(feature = "server")]
const SKILL_CENTER_COLS: &str = "id, name, url, priority, enabled, created_at, updated_at";

/// Summary of a skill center's cached catalog.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CatalogSummary {
    pub skill_channels: usize,
    pub bundles: usize,
    pub mcp_servers: usize,
    pub mcp_bundles: usize,
    /// RFC 3339 timestamp of the last successful fetch, if any.
    pub fetched_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillCentersListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillCenterGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillCenterCreateInput {
    /// Display name (required).
    pub name: String,
    /// Base URL of the skill center (required, unique).
    pub url: String,
    /// Federation token used to authenticate against the center (required).
    pub federation_token: String,
    /// Priority; higher wins when catalogs overlap.
    pub priority: i32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillCenterUpdateInput {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    /// New federation token; leave empty to keep the existing one.
    pub federation_token: String,
    pub priority: i32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillCenterDeleteInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CatalogSummaryInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillCenterSyncInput {
    pub id: Uuid,
}

// ── Handlers ────────────────────────────────────────────────────────────

/// was: list_skill_centers() in web/components/skill_center_list.rs
#[api_mcp_dioxus_server(server = "list_skill_centers")]
pub async fn skill_center_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: SkillCentersListInput,
) -> Result<Vec<SkillCenterRow>, ApiError> {
    p.require_admin()?;
    let rows = sqlx::query_as::<_, SkillCenterDbRow>(&format!(
        "SELECT {SKILL_CENTER_COLS} FROM skill_centers ORDER BY priority DESC, name"
    ))
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// was: get_skill_center() in web/components/skill_center_detail.rs
#[api_mcp_dioxus_server(server = "get_skill_center")]
pub async fn skill_center_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SkillCenterGetInput,
) -> Result<Option<SkillCenterRow>, ApiError> {
    p.require_admin()?;
    let row = sqlx::query_as::<_, SkillCenterDbRow>(&format!(
        "SELECT {SKILL_CENTER_COLS} FROM skill_centers WHERE id = $1"
    ))
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    Ok(row.map(Into::into))
}

/// was: create_skill_center() in web/components/skill_center_form.rs
#[api_mcp_dioxus_server(server = "create_skill_center")]
pub async fn skill_center_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SkillCenterCreateInput,
) -> Result<String, ApiError> {
    p.require_admin()?;

    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad_request("name-required"));
    }
    let url = input.url.trim().to_string();
    if url.is_empty() {
        return Err(ApiError::bad_request("url-required"));
    }
    let federation_token = input.federation_token.trim().to_string();
    if federation_token.is_empty() {
        return Err(ApiError::bad_request("token-required"));
    }

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO skill_centers (name, url, federation_token, priority, enabled) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(&name)
    .bind(&url)
    .bind(&federation_token)
    .bind(input.priority)
    .bind(input.enabled)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.code().as_deref() == Some("23505") {
                return ApiError::conflict(format!(
                    "A skill center with URL '{url}' already exists"
                ));
            }
        }
        internal(e)
    })?;

    Ok(id.to_string())
}

/// was: update_skill_center() in web/components/skill_center_detail.rs
#[api_mcp_dioxus_server(server = "update_skill_center")]
pub async fn skill_center_update(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SkillCenterUpdateInput,
) -> Result<SkillCenterRow, ApiError> {
    p.require_admin()?;

    if crate::builtin_skill_center::is_builtin(&input.id) {
        return Err(ApiError::bad_request(
            "cannot edit the built-in skill center",
        ));
    }

    let row = if input.federation_token.trim().is_empty() {
        sqlx::query_as::<_, SkillCenterDbRow>(&format!(
            "UPDATE skill_centers SET name = $1, url = $2, \
             priority = $3, enabled = $4, updated_at = now() \
             WHERE id = $5 \
             RETURNING {SKILL_CENTER_COLS}"
        ))
        .bind(&input.name)
        .bind(&input.url)
        .bind(input.priority)
        .bind(input.enabled)
        .bind(input.id)
        .fetch_one(pool)
        .await
    } else {
        sqlx::query_as::<_, SkillCenterDbRow>(&format!(
            "UPDATE skill_centers SET name = $1, url = $2, federation_token = $3, \
             priority = $4, enabled = $5, updated_at = now() \
             WHERE id = $6 \
             RETURNING {SKILL_CENTER_COLS}"
        ))
        .bind(&input.name)
        .bind(&input.url)
        .bind(&input.federation_token)
        .bind(input.priority)
        .bind(input.enabled)
        .bind(input.id)
        .fetch_one(pool)
        .await
    };

    row.map(Into::into)
        .map_err(|e| ApiError::internal(format!("update failed: {e}")))
}

/// was: delete_skill_center() in web/components/skill_center_detail.rs
#[api_mcp_dioxus_server(server = "delete_skill_center")]
pub async fn skill_center_delete(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SkillCenterDeleteInput,
) -> Result<(), ApiError> {
    p.require_admin()?;

    if crate::builtin_skill_center::is_builtin(&input.id) {
        return Err(ApiError::bad_request(
            "cannot delete the built-in skill center",
        ));
    }

    sqlx::query("DELETE FROM skill_centers WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::internal(format!("delete failed: {e}")))?;

    Ok(())
}

/// was: get_catalog_summary() in web/components/skill_center_detail.rs
#[api_mcp_dioxus_server(server = "get_catalog_summary")]
pub async fn skill_center_catalog_summary(
    _pool: &sqlx::PgPool,
    p: &Principal,
    input: CatalogSummaryInput,
) -> Result<CatalogSummary, ApiError> {
    p.require_admin()?;

    let cache = crate::skill_center_cache::SkillCenterCache::global()
        .ok_or_else(|| ApiError::internal("cache not available"))?;

    if let Some(cached) = cache.get(&input.id).await {
        Ok(CatalogSummary {
            skill_channels: cached.catalog.skill_channels.len(),
            bundles: cached.catalog.bundles.len(),
            mcp_servers: cached.catalog.mcp_servers.len(),
            mcp_bundles: cached.catalog.mcp_bundles.len(),
            fetched_at: Some(cached.fetched_at.to_rfc3339()),
        })
    } else {
        Ok(CatalogSummary::default())
    }
}

/// was: sync_skill_center_now() in web/components/skill_center_detail.rs
#[api_mcp_dioxus_server(server = "sync_skill_center_now")]
pub async fn skill_center_sync_now(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SkillCenterSyncInput,
) -> Result<CatalogSummary, ApiError> {
    p.require_admin()?;

    let cache = crate::skill_center_cache::SkillCenterCache::global()
        .ok_or_else(|| ApiError::internal("cache not available"))?;

    #[allow(dead_code)]
    #[derive(sqlx::FromRow)]
    struct ScRow {
        id: Uuid,
        url: String,
        federation_token: String,
        name: String,
    }

    let sc: ScRow =
        sqlx::query_as("SELECT id, url, federation_token, name FROM skill_centers WHERE id = $1")
            .bind(input.id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::internal(format!("query failed: {e}")))?
            .ok_or_else(|| ApiError::not_found("skill center not found"))?;

    let catalog = if sc.url.starts_with(crate::builtin_skill_center::BUILTIN_URL) {
        crate::builtin_skill_center::builtin_catalog()
    } else {
        let client = crate::skill_center_client::SkillCenterClient::new(
            sc.url.clone(),
            sc.federation_token.clone(),
        );
        client
            .fetch_catalog()
            .await
            .map_err(|e| ApiError::internal(format!("sync failed: {e}")))?
    };

    let summary = CatalogSummary {
        skill_channels: catalog.skill_channels.len(),
        bundles: catalog.bundles.len(),
        mcp_servers: catalog.mcp_servers.len(),
        mcp_bundles: catalog.mcp_bundles.len(),
        fetched_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    cache.update(sc.id, catalog).await;

    Ok(summary)
}

// ── Registration ────────────────────────────────────────────────────────

#[cfg(feature = "server")]
pub fn register(reg: &mut plan_ai_api_mcp::Registry<sqlx::PgPool>) {
    use plan_ai_api_mcp::{OnItem, Risk};

    let mut s = reg.resource("skill_centers", "skill_center", "Skill centers");
    s.list(
        "List configured skill centers, highest priority first (admin only).",
        |pool: sqlx::PgPool, p, input: SkillCentersListInput| async move {
            skill_center_list(&pool, &p, input).await
        },
    );
    s.get(
        "Get a skill center by id (admin only). The federation token is never returned.",
        |pool: sqlx::PgPool, p, input: SkillCenterGetInput| async move {
            skill_center_get(&pool, &p, input).await
        },
    );
    s.create(
        "Add a skill center (name, url, federation token, priority, enabled); returns its id (admin only).",
        |pool: sqlx::PgPool, p, input: SkillCenterCreateInput| async move {
            skill_center_create(&pool, &p, input).await
        },
    );
    s.update(
        "Update a skill center; an empty federation_token keeps the stored one. The built-in center cannot be edited (admin only).",
        |pool: sqlx::PgPool, p, input: SkillCenterUpdateInput| async move {
            skill_center_update(&pool, &p, input).await
        },
    );
    s.delete(
        "Delete a skill center; the built-in center cannot be deleted (admin only).",
        |pool: sqlx::PgPool, p, input: SkillCenterDeleteInput| async move {
            skill_center_delete(&pool, &p, input).await
        },
    );
    s.custom(
        "catalog_summary",
        Risk::ReadOnly,
        OnItem::Yes,
        "Summarize the skill center's cached catalog (channel/bundle/MCP counts and last fetch time); zeros if never synced (admin only).",
        |pool: sqlx::PgPool, p, input: CatalogSummaryInput| async move {
            skill_center_catalog_summary(&pool, &p, input).await
        },
    );
    s.custom(
        "sync_now",
        Risk::Mutating,
        OnItem::Yes,
        "Fetch the skill center's catalog from its remote URL now, update the cache, and return the fresh summary (admin only).",
        |pool: sqlx::PgPool, p, input: SkillCenterSyncInput| async move {
            skill_center_sync_now(&pool, &p, input).await
        },
    );
}
