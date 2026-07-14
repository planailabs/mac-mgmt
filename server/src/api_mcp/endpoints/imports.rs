//! Import-source endpoints: CRUD over `import_sources`, background sync
//! triggering, recent import jobs, and ClawHub search. All admin-only.

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

/// A skill import source (git repo or ClawHub package).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportSourceRow {
    pub id: String,
    pub name: String,
    /// `git` or `clawhub`.
    pub source_type: String,
    /// Type-specific configuration (repo_url/branch/glob or slug).
    pub source_config: serde_json::Value,
    pub channel: String,
    pub auto_sync: bool,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// One import job run for a source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportJobRow {
    pub id: String,
    pub source_id: String,
    pub status: String,
    pub skills_imported: i32,
    pub log: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A ClawHub search hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClawHubHit {
    pub slug: String,
    pub display_name: String,
    pub summary: String,
    pub version: Option<String>,
}

#[cfg(feature = "server")]
#[derive(sqlx::FromRow)]
struct SourceDbRow {
    id: Uuid,
    name: String,
    source_type: String,
    source_config: serde_json::Value,
    channel: String,
    auto_sync: bool,
    last_synced_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[cfg(feature = "server")]
impl From<SourceDbRow> for ImportSourceRow {
    fn from(r: SourceDbRow) -> Self {
        ImportSourceRow {
            id: r.id.to_string(),
            name: r.name,
            source_type: r.source_type,
            source_config: r.source_config,
            channel: r.channel,
            auto_sync: r.auto_sync,
            last_synced_at: r.last_synced_at,
            created_at: r.created_at,
        }
    }
}

#[cfg(feature = "server")]
const SOURCE_COLS: &str =
    "id, name, source_type, source_config, channel, auto_sync, last_synced_at, created_at";

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportSourcesListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportSourceGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportSourceCreateInput {
    pub name: String,
    /// `git` or `clawhub`.
    pub source_type: String,
    /// Type-specific configuration (repo_url/branch/glob or slug).
    pub source_config: serde_json::Value,
    pub channel: String,
    pub auto_sync: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportSourceUpdateInput {
    pub id: Uuid,
    pub name: String,
    /// Type-specific configuration (repo_url/branch/glob or slug).
    pub source_config: serde_json::Value,
    pub channel: String,
    pub auto_sync: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportSourceDeleteInput {
    pub id: Uuid,
    /// Also delete the skills previously imported from this source
    /// (defaults to false).
    #[serde(default)]
    pub remove_skills: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TriggerSyncInput {
    /// Import source id.
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportJobsListInput {
    /// Import source id.
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClawHubSearchInput {
    pub query: String,
}

// ── Handlers ────────────────────────────────────────────────────────────

/// was: list_import_sources() in web/components/import_sources.rs
#[api_mcp_dioxus_server(server = "list_import_sources")]
pub async fn import_source_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: ImportSourcesListInput,
) -> Result<Vec<ImportSourceRow>, ApiError> {
    p.require_admin()?;
    let rows = sqlx::query_as::<_, SourceDbRow>(&format!(
        "SELECT {SOURCE_COLS} FROM import_sources ORDER BY created_at DESC"
    ))
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// was: get_import_source() in web/components/import_sources.rs
#[api_mcp_dioxus_server(server = "get_import_source")]
pub async fn import_source_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ImportSourceGetInput,
) -> Result<ImportSourceRow, ApiError> {
    p.require_admin()?;
    let row = sqlx::query_as::<_, SourceDbRow>(&format!(
        "SELECT {SOURCE_COLS} FROM import_sources WHERE id = $1"
    ))
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or_else(|| ApiError::not_found("import source not found"))?;
    Ok(row.into())
}

/// was: create_import_source() in web/components/import_sources.rs
#[api_mcp_dioxus_server(server = "create_import_source")]
pub async fn import_source_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ImportSourceCreateInput,
) -> Result<String, ApiError> {
    p.require_admin()?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO import_sources (name, source_type, source_config, channel, auto_sync) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(&input.name)
    .bind(&input.source_type)
    .bind(&input.source_config)
    .bind(&input.channel)
    .bind(input.auto_sync)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    Ok(id.to_string())
}

/// was: update_import_source() in web/components/import_sources.rs
#[api_mcp_dioxus_server(server = "update_import_source")]
pub async fn import_source_update(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ImportSourceUpdateInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query(
        "UPDATE import_sources SET name = $1, source_config = $2, channel = $3, auto_sync = $4 WHERE id = $5",
    )
    .bind(&input.name)
    .bind(&input.source_config)
    .bind(&input.channel)
    .bind(input.auto_sync)
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// was: delete_import_source() in web/components/import_sources.rs
#[api_mcp_dioxus_server(server = "delete_import_source")]
pub async fn import_source_delete(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ImportSourceDeleteInput,
) -> Result<(), ApiError> {
    p.require_admin()?;

    if input.remove_skills {
        let slugs: Vec<String> =
            sqlx::query_scalar("SELECT skill_slug FROM import_source_skills WHERE source_id = $1")
                .bind(input.id)
                .fetch_all(pool)
                .await
                .map_err(internal)?;
        if !slugs.is_empty() {
            sqlx::query("DELETE FROM skills WHERE slug = ANY($1)")
                .bind(&slugs)
                .execute(pool)
                .await
                .map_err(internal)?;
        }
    }
    sqlx::query("DELETE FROM import_sources WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    if input.remove_skills {
        crate::api::push::notify_federation_global();
    }
    Ok(())
}

/// was: trigger_sync() in web/components/import_sources.rs
#[api_mcp_dioxus_server(server = "trigger_sync")]
pub async fn import_source_trigger_sync(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: TriggerSyncInput,
) -> Result<(), ApiError> {
    p.require_admin()?;

    let job_id: Uuid =
        sqlx::query_scalar("INSERT INTO import_jobs (source_id) VALUES ($1) RETURNING id")
            .bind(input.id)
            .fetch_one(pool)
            .await
            .map_err(internal)?;

    let source: SourceDbRow = sqlx::query_as(&format!(
        "SELECT {SOURCE_COLS} FROM import_sources WHERE id = $1"
    ))
    .bind(input.id)
    .fetch_one(pool)
    .await
    .map_err(internal)?;

    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let source_row = crate::api::importer::SourceRow {
            id: source.id,
            name: source.name,
            source_type: source.source_type,
            source_config: source.source_config,
            channel: source.channel,
            auto_sync: source.auto_sync,
            last_synced_at: source.last_synced_at,
            created_at: source.created_at,
        };
        let result = crate::api::importer::run_sync_public(&pool_clone, &source_row, job_id).await;
        let (status, extra_log) = match result {
            Ok(count) => (
                "done".to_string(),
                format!("\nCompleted: {count} skill(s) imported"),
            ),
            Err(e) => ("failed".to_string(), format!("\nError: {e}")),
        };
        let _ = sqlx::query(
            "UPDATE import_jobs SET status = $1, log = log || $2, updated_at = now() WHERE id = $3",
        )
        .bind(&status)
        .bind(&extra_log)
        .bind(job_id)
        .execute(&pool_clone)
        .await;
    });
    Ok(())
}

/// was: list_import_jobs() in web/components/import_sources.rs
#[api_mcp_dioxus_server(server = "list_import_jobs")]
pub async fn import_source_jobs(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ImportJobsListInput,
) -> Result<Vec<ImportJobRow>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        source_id: Uuid,
        status: String,
        skills_imported: i32,
        log: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, source_id, status, skills_imported, log, created_at, updated_at \
         FROM import_jobs WHERE source_id = $1 ORDER BY created_at DESC LIMIT 20",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| ImportJobRow {
            id: r.id.to_string(),
            source_id: r.source_id.to_string(),
            status: r.status,
            skills_imported: r.skills_imported,
            log: r.log,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect())
}

/// was: search_clawhub() in web/components/import_sources.rs
#[api_mcp_dioxus_server(server = "search_clawhub")]
pub async fn import_source_search_clawhub(
    _pool: &sqlx::PgPool,
    p: &Principal,
    input: ClawHubSearchInput,
) -> Result<Vec<ClawHubHit>, ApiError> {
    p.require_admin()?;
    let cfg = crate::config::config();
    let importer = cfg
        .importer
        .as_ref()
        .ok_or_else(|| ApiError::internal("importer not configured"))?;
    let client = crate::clawhub_client::ClawHubClient::new(&importer.clawhub_url);
    let results = client
        .search(&input.query, 20)
        .await
        .map_err(ApiError::internal)?;
    Ok(results
        .into_iter()
        .map(|r| ClawHubHit {
            slug: r.slug,
            display_name: r.display_name,
            summary: r.summary,
            version: r.version,
        })
        .collect())
}

// ── Registration ────────────────────────────────────────────────────────

#[cfg(feature = "server")]
pub fn register(reg: &mut plan_ai_api_mcp::Registry<sqlx::PgPool>) {
    use plan_ai_api_mcp::{OnItem, Risk};

    let mut s = reg.resource("import_sources", "import_source", "Import sources");
    s.list(
        "List skill import sources, newest first (admin only).",
        |pool: sqlx::PgPool, p, input: ImportSourcesListInput| async move {
            import_source_list(&pool, &p, input).await
        },
    );
    s.get(
        "Get an import source by id (admin only).",
        |pool: sqlx::PgPool, p, input: ImportSourceGetInput| async move {
            import_source_get(&pool, &p, input).await
        },
    );
    s.create(
        "Add an import source (git repo or ClawHub package); returns its id (admin only).",
        |pool: sqlx::PgPool, p, input: ImportSourceCreateInput| async move {
            import_source_create(&pool, &p, input).await
        },
    );
    s.update(
        "Update an import source's name, config, channel and auto-sync flag (admin only).",
        |pool: sqlx::PgPool, p, input: ImportSourceUpdateInput| async move {
            import_source_update(&pool, &p, input).await
        },
    );
    s.delete(
        "Delete an import source; with remove_skills=true also delete the skills it imported (admin only).",
        |pool: sqlx::PgPool, p, input: ImportSourceDeleteInput| async move {
            import_source_delete(&pool, &p, input).await
        },
    );
    s.custom(
        "trigger_sync",
        Risk::Mutating,
        OnItem::Yes,
        "Start a background import sync for the source; creates an import job and returns immediately (admin only).",
        |pool: sqlx::PgPool, p, input: TriggerSyncInput| async move {
            import_source_trigger_sync(&pool, &p, input).await
        },
    );
    s.custom(
        "jobs",
        Risk::ReadOnly,
        OnItem::Yes,
        "List the source's 20 most recent import jobs with status and log (admin only).",
        |pool: sqlx::PgPool, p, input: ImportJobsListInput| async move {
            import_source_jobs(&pool, &p, input).await
        },
    );
    s.custom(
        "search_clawhub",
        Risk::ReadOnly,
        OnItem::No,
        "Search the configured ClawHub registry for importable skill packages (admin only).",
        |pool: sqlx::PgPool, p, input: ClawHubSearchInput| async move {
            import_source_search_clawhub(&pool, &p, input).await
        },
    );
}
