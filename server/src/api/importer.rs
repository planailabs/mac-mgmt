//! Skill importer: import skills from git repos and ClawHub into the
//! skill-center catalog. Imported skills are uploaded to xzar as `noarch`
//! pins and registered in the DB via the normal sync_from_xzar pattern.

use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{State, delete, get, post};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::auth::AdminAuth;

// ── Request / response types ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SourceConfig {
    #[serde(rename = "git")]
    Git {
        repo_url: String,
        #[serde(default)]
        branch: Option<String>,
        /// Glob pattern to match skill directories (e.g. "skills/*").
        glob: String,
    },
    #[serde(rename = "clawhub")]
    ClawHub {
        /// Skill slug on ClawHub.
        slug: String,
        /// Specific version to pin. None = latest.
        #[serde(default)]
        version: Option<String>,
        /// Override the skill slug in our DB (default: use ClawHub slug).
        #[serde(default)]
        skill_slug: Option<String>,
    },
}

#[derive(Deserialize)]
pub struct CreateSourceRequest {
    pub name: String,
    pub source: SourceConfig,
    pub channel: String,
    #[serde(default)]
    pub auto_sync: bool,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct SourceRow {
    pub id: Uuid,
    pub name: String,
    pub source_type: String,
    pub source_config: serde_json::Value,
    pub channel: String,
    pub auto_sync: bool,
    pub last_synced_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct JobRow {
    pub id: Uuid,
    pub source_id: Uuid,
    pub status: String,
    pub skills_imported: i32,
    pub log: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Serialize)]
pub struct SearchHit {
    pub slug: String,
    pub display_name: String,
    pub summary: String,
    pub version: Option<String>,
}

#[derive(Deserialize)]
pub struct DeleteQuery {
    #[serde(default)]
    pub remove_skills: bool,
}

// ── Routes ─────────────────────────────────────────────────────────────

#[post("/admin/import/sources", data = "<body>")]
pub async fn create_source(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    body: Json<CreateSourceRequest>,
) -> Result<Json<SourceRow>, Status> {
    let source_type = match &body.source {
        SourceConfig::Git { .. } => "git",
        SourceConfig::ClawHub { .. } => "clawhub",
    };
    let source_config =
        serde_json::to_value(&body.source).map_err(|_| Status::InternalServerError)?;

    let row: SourceRow = sqlx::query_as(
        "INSERT INTO import_sources (name, source_type, source_config, channel, auto_sync) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING *",
    )
    .bind(&body.name)
    .bind(source_type)
    .bind(&source_config)
    .bind(&body.channel)
    .bind(body.auto_sync)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| {
        tracing::error!("failed to create import source: {e}");
        Status::InternalServerError
    })?;

    Ok(Json(row))
}

#[get("/admin/import/sources")]
pub async fn list_sources(
    _auth: AdminAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<SourceRow>>, Status> {
    let rows: Vec<SourceRow> =
        sqlx::query_as("SELECT * FROM import_sources ORDER BY created_at DESC")
            .fetch_all(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?;

    Ok(Json(rows))
}

#[delete("/admin/import/sources/<id>?<remove_skills>")]
pub async fn delete_source(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    id: &str,
    remove_skills: Option<bool>,
) -> Result<Status, Status> {
    let id: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    let remove = remove_skills.unwrap_or(false);

    if remove {
        // Delete skills that were imported by this source
        let slugs: Vec<String> =
            sqlx::query_scalar("SELECT skill_slug FROM import_source_skills WHERE source_id = $1")
                .bind(id)
                .fetch_all(pool.inner())
                .await
                .map_err(|_| Status::InternalServerError)?;

        if !slugs.is_empty() {
            // Delete skill_channels and skills for these slugs
            // (CASCADE will handle cluster_skills, bundle_items, etc.)
            sqlx::query("DELETE FROM skills WHERE slug = ANY($1)")
                .bind(&slugs)
                .execute(pool.inner())
                .await
                .map_err(|_| Status::InternalServerError)?;
        }
    }

    // Delete source (cascades to import_jobs and import_source_skills)
    let deleted = sqlx::query("DELETE FROM import_sources WHERE id = $1")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?
        .rows_affected();

    if deleted == 0 {
        return Err(Status::NotFound);
    }

    if remove {
        super::push::notify_federation_global();
    }

    Ok(Status::NoContent)
}

#[post("/admin/import/sources/<id>/sync")]
pub async fn sync_source(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Json<JobRow>, Status> {
    let id: Uuid = id.parse().map_err(|_| Status::BadRequest)?;

    // Verify source exists
    let source: SourceRow = sqlx::query_as("SELECT * FROM import_sources WHERE id = $1")
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;

    // Create job
    let job: JobRow = sqlx::query_as("INSERT INTO import_jobs (source_id) VALUES ($1) RETURNING *")
        .bind(id)
        .fetch_one(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    let job_id = job.id;
    let pool = pool.inner().clone();

    // Spawn background task
    tokio::spawn(async move {
        let result = run_sync(&pool, &source, job_id).await;
        let (status, extra_log) = match result {
            Ok(count) => (
                "done".to_string(),
                format!("\nCompleted: {count} skill(s) imported"),
            ),
            Err(e) => ("failed".to_string(), format!("\nError: {e}")),
        };

        let _ = sqlx::query(
            "UPDATE import_jobs SET status = $1, log = log || $2, \
             updated_at = now() WHERE id = $3",
        )
        .bind(&status)
        .bind(&extra_log)
        .bind(job_id)
        .execute(&pool)
        .await;
    });

    Ok(Json(job))
}

#[get("/admin/import/jobs")]
pub async fn list_jobs(
    _auth: AdminAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<JobRow>>, Status> {
    let rows: Vec<JobRow> =
        sqlx::query_as("SELECT * FROM import_jobs ORDER BY created_at DESC LIMIT 50")
            .fetch_all(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?;

    Ok(Json(rows))
}

#[get("/admin/import/jobs/<id>")]
pub async fn get_job(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Json<JobRow>, Status> {
    let id: Uuid = id.parse().map_err(|_| Status::BadRequest)?;

    let row: JobRow = sqlx::query_as("SELECT * FROM import_jobs WHERE id = $1")
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;

    Ok(Json(row))
}

#[get("/admin/import/search/clawhub?<q>")]
pub async fn search_clawhub(_auth: AdminAuth, q: &str) -> Result<Json<Vec<SearchHit>>, Status> {
    let cfg = crate::config::config();
    let importer = cfg.importer.as_ref().ok_or(Status::ServiceUnavailable)?;
    let client = crate::clawhub_client::ClawHubClient::new(&importer.clawhub_url);
    let results = client.search(q, 20).await.map_err(|e| {
        tracing::error!("clawhub search failed: {e}");
        Status::BadGateway
    })?;

    let hits: Vec<SearchHit> = results
        .into_iter()
        .map(|r| SearchHit {
            slug: r.slug,
            display_name: r.display_name,
            summary: r.summary,
            version: r.version,
        })
        .collect();

    Ok(Json(hits))
}

// ── Sync logic ─────────────────────────────────────────────────────────

/// Run a sync for a source. Public so the web UI can trigger it.
pub async fn run_sync_public(
    pool: &PgPool,
    source: &SourceRow,
    job_id: Uuid,
) -> Result<u32, String> {
    run_sync(pool, source, job_id).await
}

async fn run_sync(pool: &PgPool, source: &SourceRow, job_id: Uuid) -> Result<u32, String> {
    // Mark job as running
    let _ =
        sqlx::query("UPDATE import_jobs SET status = 'running', updated_at = now() WHERE id = $1")
            .bind(job_id)
            .execute(pool)
            .await;

    let cfg = crate::config::config();
    let xzar = cfg.xzar.as_ref().ok_or("xzar not configured")?;
    let importer = cfg.importer.as_ref().ok_or("importer not configured")?;

    let source_config: SourceConfig = serde_json::from_value(source.source_config.clone())
        .map_err(|e| format!("invalid source config: {e}"))?;

    let count = match source_config {
        SourceConfig::Git {
            repo_url,
            branch,
            glob,
        } => {
            run_git_sync(
                pool,
                source.id,
                job_id,
                &xzar.url,
                &xzar.token,
                &importer.work_dir,
                &repo_url,
                branch.as_deref(),
                &glob,
                &source.channel,
            )
            .await?
        }
        SourceConfig::ClawHub {
            slug,
            version,
            skill_slug,
        } => {
            run_clawhub_sync(
                pool,
                source.id,
                job_id,
                &xzar.url,
                &xzar.token,
                &importer.work_dir,
                &importer.clawhub_url,
                &slug,
                version.as_deref(),
                skill_slug.as_deref(),
                &source.channel,
            )
            .await?
        }
    };

    // Sync xzar pins → DB (upserts new skills, removes stale ones, notifies federation)
    append_log(pool, job_id, "Syncing skills from xzar pins to DB...").await;
    match crate::xzar::sync_skills_db(pool).await {
        Ok(sync) => {
            append_log(
                pool,
                job_id,
                &format!(
                    "DB sync: +{} skills, +{} channels, -{} channels, -{} skills",
                    sync.created_skills,
                    sync.created_channels,
                    sync.removed_channels,
                    sync.removed_skills,
                ),
            )
            .await;
        }
        Err(e) => {
            append_log(pool, job_id, &format!("WARN: DB sync failed: {e}")).await;
        }
    }

    // Update source last_synced_at
    let _ = sqlx::query("UPDATE import_sources SET last_synced_at = now() WHERE id = $1")
        .bind(source.id)
        .execute(pool)
        .await;

    // Update job skills_imported count
    let _ = sqlx::query(
        "UPDATE import_jobs SET skills_imported = $1, updated_at = now() WHERE id = $2",
    )
    .bind(count as i32)
    .bind(job_id)
    .execute(pool)
    .await;

    Ok(count)
}

async fn append_log(pool: &PgPool, job_id: Uuid, msg: &str) {
    let line = format!("{msg}\n");
    let _ = sqlx::query("UPDATE import_jobs SET log = log || $1, updated_at = now() WHERE id = $2")
        .bind(&line)
        .bind(job_id)
        .execute(pool)
        .await;
}

async fn run_git_sync(
    pool: &PgPool,
    source_id: Uuid,
    job_id: Uuid,
    xzar_url: &str,
    xzar_token: &str,
    work_dir: &str,
    repo_url: &str,
    branch: Option<&str>,
    glob_pattern: &str,
    channel: &str,
) -> Result<u32, String> {
    let clone_dir = format!("{work_dir}/{job_id}");

    // Ensure work dir exists
    tokio::fs::create_dir_all(&clone_dir)
        .await
        .map_err(|e| format!("failed to create work dir: {e}"))?;

    // Clone repo
    append_log(pool, job_id, &format!("Cloning {repo_url}...")).await;
    let mut args = vec!["clone", "--depth", "1"];
    if let Some(b) = branch {
        args.extend(["--branch", b]);
    }
    args.extend([repo_url, &clone_dir]);

    let output = tokio::process::Command::new("git")
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("git clone failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = tokio::fs::remove_dir_all(&clone_dir).await;
        return Err(format!("git clone failed: {stderr}"));
    }

    // Resolve glob
    let full_pattern = format!("{clone_dir}/{glob_pattern}");
    let matched: Vec<std::path::PathBuf> = glob::glob(&full_pattern)
        .map_err(|e| format!("invalid glob pattern: {e}"))?
        .filter_map(|entry| entry.ok())
        .filter(|p| p.is_dir())
        .collect();

    append_log(
        pool,
        job_id,
        &format!("Glob matched {} directories", matched.len()),
    )
    .await;

    let mut imported = 0u32;
    for dir in &matched {
        let slug = dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "invalid directory name".to_string())?;

        append_log(pool, job_id, &format!("Processing skill: {slug}")).await;

        // nix-store --add to get store path
        let output = tokio::process::Command::new("nix-store")
            .args(["--add", &dir.to_string_lossy()])
            .output()
            .await
            .map_err(|e| format!("nix-store --add failed for {slug}: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            append_log(
                pool,
                job_id,
                &format!("  SKIP {slug}: nix-store --add failed: {stderr}"),
            )
            .await;
            continue;
        }

        let store_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Check content hash for change detection
        let content_hash = nix_content_hash(&store_path).await.unwrap_or_default();
        let existing_hash: Option<String> = sqlx::query_scalar(
            "SELECT content_hash FROM import_source_skills \
             WHERE source_id = $1 AND skill_slug = $2",
        )
        .bind(source_id)
        .bind(slug)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        if existing_hash.as_deref() == Some(&content_hash) && !content_hash.is_empty() {
            append_log(pool, job_id, &format!("  {slug}: unchanged, skipping")).await;
            continue;
        }

        // Upload to xzar as noarch
        let pin_name = format!("skill/{slug}/{channel}/noarch");
        append_log(pool, job_id, &format!("  Uploading {pin_name}...")).await;

        if let Err(e) = crate::xzar_upload::upload_and_pin(
            xzar_url,
            xzar_token,
            &store_path,
            &pin_name,
            Some(&format!("Imported from {repo_url}")),
        )
        .await
        {
            append_log(pool, job_id, &format!("  WARN {slug}: upload failed: {e}")).await;
            continue;
        }

        // Track in import_source_skills
        sqlx::query(
            "INSERT INTO import_source_skills (source_id, skill_slug, content_hash) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (source_id, skill_slug) DO UPDATE SET content_hash = $3",
        )
        .bind(source_id)
        .bind(slug)
        .bind(&content_hash)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to track skill: {e}"))?;

        append_log(pool, job_id, &format!("  {slug}: done")).await;
        imported += 1;
    }

    // Cleanup clone dir
    let _ = tokio::fs::remove_dir_all(&clone_dir).await;

    Ok(imported)
}

async fn run_clawhub_sync(
    pool: &PgPool,
    source_id: Uuid,
    job_id: Uuid,
    xzar_url: &str,
    xzar_token: &str,
    work_dir: &str,
    clawhub_url: &str,
    clawhub_slug: &str,
    version: Option<&str>,
    skill_slug_override: Option<&str>,
    channel: &str,
) -> Result<u32, String> {
    let extract_dir = format!("{work_dir}/{job_id}");

    tokio::fs::create_dir_all(&extract_dir)
        .await
        .map_err(|e| format!("failed to create work dir: {e}"))?;

    // Download zip from ClawHub
    append_log(
        pool,
        job_id,
        &format!("Downloading {clawhub_slug} from ClawHub..."),
    )
    .await;
    let client = crate::clawhub_client::ClawHubClient::new(clawhub_url);
    let zip_bytes = client.download(clawhub_slug, version).await?;

    // Extract zip
    append_log(pool, job_id, "Extracting archive...").await;
    let skill_slug = skill_slug_override.unwrap_or(clawhub_slug);
    let skill_dir = format!("{extract_dir}/{skill_slug}");

    tokio::fs::create_dir_all(&skill_dir)
        .await
        .map_err(|e| format!("failed to create skill dir: {e}"))?;

    // Extract zip in blocking task
    let skill_dir_clone = skill_dir.clone();
    tokio::task::spawn_blocking(move || {
        let cursor = std::io::Cursor::new(zip_bytes);
        let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("invalid zip: {e}"))?;
        archive
            .extract(&skill_dir_clone)
            .map_err(|e| format!("zip extraction failed: {e}"))?;
        Ok::<_, String>(())
    })
    .await
    .map_err(|e| format!("zip extraction task panicked: {e}"))??;

    // nix-store --add
    append_log(
        pool,
        job_id,
        &format!("Adding {skill_slug} to nix store..."),
    )
    .await;
    let output = tokio::process::Command::new("nix-store")
        .args(["--add", &skill_dir])
        .output()
        .await
        .map_err(|e| format!("nix-store --add failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = tokio::fs::remove_dir_all(&extract_dir).await;
        return Err(format!("nix-store --add failed: {stderr}"));
    }

    let store_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Check content hash
    let content_hash = nix_content_hash(&store_path).await.unwrap_or_default();
    let existing_hash: Option<String> = sqlx::query_scalar(
        "SELECT content_hash FROM import_source_skills \
         WHERE source_id = $1 AND skill_slug = $2",
    )
    .bind(source_id)
    .bind(skill_slug)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if existing_hash.as_deref() == Some(&content_hash) && !content_hash.is_empty() {
        append_log(pool, job_id, &format!("{skill_slug}: unchanged, skipping")).await;
        let _ = tokio::fs::remove_dir_all(&extract_dir).await;
        return Ok(0);
    }

    // Upload to xzar
    let pin_name = format!("skill/{skill_slug}/{channel}/noarch");
    append_log(pool, job_id, &format!("Uploading {pin_name}...")).await;

    crate::xzar_upload::upload_and_pin(
        xzar_url,
        xzar_token,
        &store_path,
        &pin_name,
        Some(&format!("Imported from ClawHub: {clawhub_slug}")),
    )
    .await?;

    // Track
    sqlx::query(
        "INSERT INTO import_source_skills (source_id, skill_slug, content_hash) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (source_id, skill_slug) DO UPDATE SET content_hash = $3",
    )
    .bind(source_id)
    .bind(skill_slug)
    .bind(&content_hash)
    .execute(pool)
    .await
    .map_err(|e| format!("failed to track skill: {e}"))?;

    append_log(pool, job_id, &format!("{skill_slug}: done")).await;

    // Cleanup
    let _ = tokio::fs::remove_dir_all(&extract_dir).await;

    Ok(1)
}

// ── Helpers ────────────────────────────────────────────────────────────

async fn nix_content_hash(store_path: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("nix-store")
        .args(["--query", "--hash", store_path])
        .output()
        .await
        .map_err(|e| format!("nix-store --query --hash failed: {e}"))?;

    if !output.status.success() {
        return Err("nix-store --query --hash failed".into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ── Periodic sync loop ─────────────────────────────────────────────────

/// Background loop that periodically syncs auto_sync sources.
/// Spawned on server startup when the `skill-importer` feature is enabled.
pub async fn sync_loop(pool: PgPool) {
    let cfg = crate::config::config();
    let interval_secs = cfg
        .importer
        .as_ref()
        .map(|i| i.sync_interval_secs)
        .unwrap_or(3600);

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));

    // Skip the immediate first tick
    interval.tick().await;

    loop {
        interval.tick().await;

        tracing::debug!("import sync loop: checking auto-sync sources");

        let sources: Vec<SourceRow> =
            match sqlx::query_as("SELECT * FROM import_sources WHERE auto_sync = true")
                .fetch_all(&pool)
                .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::error!("import sync loop: failed to fetch sources: {e}");
                    continue;
                }
            };

        for source in sources {
            // Skip if a job is already running for this source
            let running: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM import_jobs WHERE source_id = $1 AND status IN ('pending', 'running'))",
            )
            .bind(source.id)
            .fetch_one(&pool)
            .await
            .unwrap_or(true);

            if running {
                tracing::debug!(
                    "import sync loop: skipping {} (job in progress)",
                    source.name
                );
                continue;
            }

            // Create job and run sync
            let job: Result<JobRow, _> =
                sqlx::query_as("INSERT INTO import_jobs (source_id) VALUES ($1) RETURNING *")
                    .bind(source.id)
                    .fetch_one(&pool)
                    .await;

            let job = match job {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!(
                        "import sync loop: failed to create job for {}: {e}",
                        source.name
                    );
                    continue;
                }
            };

            let job_id = job.id;
            let pool_clone = pool.clone();

            tokio::spawn(async move {
                let result = run_sync(&pool_clone, &source, job_id).await;
                let (status, extra_log) = match result {
                    Ok(count) => (
                        "done".to_string(),
                        format!("\nCompleted: {count} skill(s) imported"),
                    ),
                    Err(e) => ("failed".to_string(), format!("\nError: {e}")),
                };

                let _ = sqlx::query(
                    "UPDATE import_jobs SET status = $1, log = log || $2, \
                     updated_at = now() WHERE id = $3",
                )
                .bind(&status)
                .bind(&extra_log)
                .bind(job_id)
                .execute(&pool_clone)
                .await;
            });
        }
    }
}
