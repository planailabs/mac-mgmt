//! Deploy API: upload tarballs and deploy to Cloudflare Pages via wrangler.
//!
//! - `POST /api/v1/deploy/:webspace_id` — upload a tarball, starts background deploy
//! - `GET  /api/v1/deploy/:webspace_id/status` — check latest deployment status
//! - `GET  /api/v1/deploy/whoami` — token info and scoped webspace
//!
//! Auth: Bearer token with kind="deploy" and scopes containing the webspace_id.

use dioxus::fullstack::axum::{
    self as axum, Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
};
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

/// Shared state for the deploy API.
#[derive(Clone)]
pub struct DeployState {
    pub pool: PgPool,
    pub active_deploys: Arc<AtomicUsize>,
}

/// Maximum tarball upload size: 10 GiB. The body is streamed to disk and this
/// cap is enforced as the bytes arrive (plus an early Content-Length check), so
/// no oversized upload is ever buffered in memory.
const MAX_UPLOAD_BYTES: usize = 64 * 1024 * 1024 * 1024;

/// How long a single deploy request may take. Uploading a multi-gigabyte
/// tarball over a slow link can run for many minutes, so allow up to an hour
/// before the request is considered timed out.
const UPLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3600);

pub fn router(state: DeployState) -> Router<()> {
    Router::new()
        .route("/api/v1/deploy/whoami", get(whoami))
        .route("/api/v1/deploy/{webspace_id}", post(upload_deploy))
        .route("/api/v1/deploy/{webspace_id}/status", get(deploy_status))
        // Tarball uploads can run for many minutes; give them up to an hour. The
        // body itself is streamed to disk and size-capped in the handler. Scoped
        // to the deploy routes only — the rest of the app keeps its defaults.
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            UPLOAD_TIMEOUT,
        ))
        .with_state(state)
}

/// Mark any in-flight deployments from a previous server run as failed.
/// Called once at startup before accepting requests.
pub async fn recover_interrupted_deployments(pool: &PgPool) {
    let result = sqlx::query(
        "UPDATE deployments SET status = 'failed', error_message = 'server restarted during deployment', updated_at = now() \
         WHERE status IN ('pending', 'uploading', 'deploying')",
    )
    .execute(pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::warn!(
                "marked {} interrupted deployment(s) as failed",
                r.rows_affected()
            );
        }
        Ok(_) => {}
        Err(e) => tracing::error!("failed to recover interrupted deployments: {e}"),
    }
}

/// Wait for all active deployments to finish, with a timeout.
pub async fn drain_active_deploys(active: &AtomicUsize, timeout: std::time::Duration) {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let count = active.load(Ordering::Relaxed);
        if count == 0 {
            return;
        }
        if tokio::time::Instant::now() > deadline {
            tracing::warn!("{count} deployment(s) still running after timeout, giving up");
            return;
        }
        tracing::info!("waiting for {count} active deployment(s) to finish...");
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

// ── Auth ──────────────────────────────────────────────────────────────

async fn authenticate_deploy(
    pool: &PgPool,
    headers: &HeaderMap,
    webspace_id: Uuid,
) -> Result<(), (StatusCode, String)> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or((StatusCode::UNAUTHORIZED, "missing Bearer token".into()))?;

    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(token.as_bytes()));

    let row = sqlx::query_as::<_, (String, Option<serde_json::Value>)>(
        "SELECT kind, scopes FROM tokens WHERE token_hash = $1 AND NOT revoked AND (expires_at IS NULL OR expires_at > now())",
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::UNAUTHORIZED, "invalid token".into()))?;

    let (kind, scopes) = row;
    if kind != "deploy" {
        return Err((StatusCode::FORBIDDEN, "token kind must be 'deploy'".into()));
    }

    if let Some(scopes) = scopes {
        let allowed = scopes
            .get("webspace_id")
            .and_then(|v| v.as_str())
            .map(|id| id == webspace_id.to_string())
            .unwrap_or(false);
        let is_wildcard = !scopes
            .as_object()
            .map(|o| o.contains_key("webspace_id"))
            .unwrap_or(false);
        if !allowed && !is_wildcard {
            return Err((
                StatusCode::FORBIDDEN,
                "token not scoped to this webspace".into(),
            ));
        }
    }

    Ok(())
}

// ── Whoami ────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct WhoamiResponse {
    kind: String,
    webspace_id: Option<String>,
    webspace_name: Option<String>,
}

async fn whoami(
    State(state): State<DeployState>,
    headers: HeaderMap,
) -> Result<Json<WhoamiResponse>, (StatusCode, String)> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or((StatusCode::UNAUTHORIZED, "missing Bearer token".into()))?;

    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(token.as_bytes()));

    let row = sqlx::query_as::<_, (String, Option<serde_json::Value>)>(
        "SELECT kind, scopes FROM tokens WHERE token_hash = $1 AND NOT revoked AND (expires_at IS NULL OR expires_at > now())",
    )
    .bind(&hash)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::UNAUTHORIZED, "invalid token".into()))?;

    let (kind, scopes) = row;
    let ws_id = scopes
        .as_ref()
        .and_then(|s| s.get("webspace_id"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let ws_name = if let Some(ref wid) = ws_id {
        if let Ok(uid) = uuid::Uuid::parse_str(wid) {
            sqlx::query_scalar::<_, String>("SELECT name FROM webspaces WHERE id = $1")
                .bind(uid)
                .fetch_optional(&state.pool)
                .await
                .ok()
                .flatten()
        } else {
            None
        }
    } else {
        None
    };

    Ok(Json(WhoamiResponse {
        kind,
        webspace_id: ws_id,
        webspace_name: ws_name,
    }))
}

// ── Handlers ──────────────────────────────────────────────────────────

#[derive(serde::Deserialize, Default)]
struct UploadQuery {
    #[serde(default)]
    branch: Option<String>,
}

#[derive(Serialize)]
struct UploadResponse {
    deployment_id: Uuid,
    status: String,
}

async fn upload_deploy(
    State(state): State<DeployState>,
    Path(webspace_id): Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<UploadQuery>,
    headers: HeaderMap,
    body: Body,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    authenticate_deploy(&state.pool, &headers, webspace_id).await?;

    let ws = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<Uuid>)>(
        "SELECT hosting_type, runtime, cloudflare_pages_project, cloudflare_credential_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "webspace not found".into()))?;

    let (hosting_type, runtime, project_name, cred_id) = ws;

    // Determine the deploy target. Tarball upload is supported for Cloudflare
    // Pages (via wrangler) and local static webspaces (extracted to disk).
    let target = match hosting_type.as_str() {
        "cloudflare_pages" => {
            let project_name = project_name.ok_or((
                StatusCode::BAD_REQUEST,
                "Pages project not deployed yet".into(),
            ))?;
            let cred_id = cred_id.ok_or((
                StatusCode::BAD_REQUEST,
                "no Cloudflare credential linked".into(),
            ))?;
            DeployTarget::CloudflarePages {
                project_name,
                cred_id,
            }
        }
        "local" if runtime.as_deref() == Some("static") => DeployTarget::LocalStatic,
        "local" => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "tarball deploy is only supported for the 'static' runtime (this folder uses '{}')",
                    runtime.as_deref().unwrap_or("none")
                ),
            ));
        }
        other => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("hosting type '{other}' does not support tarball deploy"),
            ));
        }
    };

    // Reject obviously-oversized uploads up front via Content-Length, then
    // stream the body straight to a temp file on disk — the full multi-gigabyte
    // tarball is never held in memory.
    if let Some(len) = headers
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
    {
        if len > MAX_UPLOAD_BYTES as u64 {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("upload exceeds the {MAX_UPLOAD_BYTES} byte limit"),
            ));
        }
    }

    let upload = tempfile::Builder::new()
        .prefix("web-agency-deploy-")
        .suffix(".tar.gz")
        .tempfile()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("tempfile: {e}")))?;
    let (std_file, upload_path) = upload.into_parts();
    let mut file = tokio::fs::File::from_std(std_file);

    let mut tarball_size: i64 = 0;
    {
        use futures_core::Stream as _;
        use tokio::io::AsyncWriteExt as _;
        let mut stream = std::pin::pin!(body.into_data_stream());
        loop {
            match std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await {
                Some(Ok(chunk)) => {
                    tarball_size += chunk.len() as i64;
                    if tarball_size > MAX_UPLOAD_BYTES as i64 {
                        return Err((
                            StatusCode::PAYLOAD_TOO_LARGE,
                            format!("upload exceeds the {MAX_UPLOAD_BYTES} byte limit"),
                        ));
                    }
                    file.write_all(&chunk).await.map_err(|e| {
                        (StatusCode::INTERNAL_SERVER_ERROR, format!("write upload: {e}"))
                    })?;
                }
                Some(Err(e)) => {
                    return Err((StatusCode::BAD_REQUEST, format!("upload stream error: {e}")));
                }
                None => break,
            }
        }
        file.flush()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("flush upload: {e}")))?;
    }
    // Close the write handle so the background deploy can re-open the file.
    drop(file);

    tracing::info!(
        webspace_id = %webspace_id,
        target = target.label(),
        tarball_bytes = tarball_size,
        branch = ?query.branch,
        "received deploy upload"
    );

    let deployment_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO deployments (webspace_id, status, tarball_size) VALUES ($1, 'uploading', $2) RETURNING id",
    )
    .bind(webspace_id).bind(tarball_size)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let pool = state.pool.clone();
    let active = state.active_deploys.clone();
    active.fetch_add(1, Ordering::Relaxed);

    match target {
        DeployTarget::CloudflarePages {
            project_name,
            cred_id,
        } => {
            let cf_token = match get_cf_token(&state.pool, cred_id).await {
                Ok(t) => t,
                Err(e) => {
                    active.fetch_sub(1, Ordering::Relaxed);
                    set_failed(&state.pool, deployment_id, &e).await;
                    // `upload_path` drops here, deleting the staged tarball.
                    return Err((StatusCode::INTERNAL_SERVER_ERROR, e));
                }
            };
            let account_id = get_cf_account_id(&state.pool, cred_id)
                .await
                .unwrap_or_default();

            // If no branch specified, fetch the production branch from the CF Pages project
            let branch = if query.branch.is_some() {
                query.branch
            } else if !account_id.is_empty() {
                let client = cloudflare_api::compat::SimpleClient::new(&cf_token);
                match client.get_pages_project(&account_id, &project_name).await {
                    Ok(project) => Some(project.production_branch.unwrap_or_else(|| "main".into())),
                    Err(e) => {
                        tracing::warn!(deployment_id = %deployment_id, error = %e, "could not fetch production branch, defaulting to 'main'");
                        Some("main".into())
                    }
                }
            } else {
                Some("main".into())
            };

            tokio::spawn(async move {
                run_wrangler_deploy(
                    pool,
                    deployment_id,
                    project_name,
                    cf_token,
                    account_id,
                    branch,
                    upload_path.to_path_buf(),
                )
                .await;
                let _ = upload_path.close(); // delete the staged tarball
                active.fetch_sub(1, Ordering::Relaxed);
            });
        }
        DeployTarget::LocalStatic => {
            tokio::spawn(async move {
                run_static_deploy(pool, deployment_id, webspace_id, upload_path.to_path_buf())
                    .await;
                let _ = upload_path.close(); // delete the staged tarball
                active.fetch_sub(1, Ordering::Relaxed);
            });
        }
    }

    Ok(Json(UploadResponse {
        deployment_id,
        status: "uploading".into(),
    }))
}

/// Where a tarball upload should be deployed.
enum DeployTarget {
    CloudflarePages { project_name: String, cred_id: Uuid },
    LocalStatic,
}

impl DeployTarget {
    fn label(&self) -> &'static str {
        match self {
            DeployTarget::CloudflarePages { .. } => "cloudflare_pages",
            DeployTarget::LocalStatic => "local_static",
        }
    }
}

#[derive(Serialize)]
struct StatusResponse {
    deployment_id: Uuid,
    status: String,
    error_message: Option<String>,
    tarball_size: Option<i64>,
    created_at: String,
    updated_at: String,
}

async fn deploy_status(
    State(state): State<DeployState>,
    Path(webspace_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<StatusResponse>, (StatusCode, String)> {
    authenticate_deploy(&state.pool, &headers, webspace_id).await?;

    let row = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            Option<String>,
            Option<i64>,
            chrono::DateTime<chrono::Utc>,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        "SELECT id, status, error_message, tarball_size, created_at, updated_at \
         FROM deployments WHERE webspace_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(webspace_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "no deployments found".into()))?;

    let (deployment_id, status, error_message, tarball_size, created_at, updated_at) = row;
    Ok(Json(StatusResponse {
        deployment_id,
        status,
        error_message,
        tarball_size,
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    }))
}

// ── Background deploy ─────────────────────────────────────────────────

async fn get_cf_token(pool: &PgPool, cred_id: Uuid) -> Result<String, String> {
    let data = crate::credentials::credential_json(pool, cred_id, "cloudflare")
        .await
        .map_err(|e| e.to_string())?;
    data["api_token"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| "missing api_token".into())
}

async fn get_cf_account_id(pool: &PgPool, cred_id: Uuid) -> Result<String, String> {
    let (_, account_id) = crate::credentials::cf_client_with_account(pool, cred_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(account_id)
}

async fn run_wrangler_deploy(
    pool: PgPool,
    deployment_id: Uuid,
    project_name: String,
    cf_token: String,
    account_id: String,
    branch: Option<String>,
    tarball_path: std::path::PathBuf,
) {
    let branch_label = branch.as_deref().unwrap_or("production");
    tracing::info!(
        deployment_id = %deployment_id,
        project = %project_name,
        branch = %branch_label,
        "starting deployment"
    );

    let _ = sqlx::query(
        "UPDATE deployments SET status = 'deploying', updated_at = now() WHERE id = $1",
    )
    .bind(deployment_id)
    .execute(&pool)
    .await;

    let tmp_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            set_failed(&pool, deployment_id, &format!("tempdir: {e}")).await;
            return;
        }
    };
    tracing::debug!(deployment_id = %deployment_id, dir = %tmp_dir.path().display(), "created temp dir");

    let extract_dir = tmp_dir.path().join("site");

    tracing::info!(deployment_id = %deployment_id, "extracting tarball");
    if let Err(e) = extract_tarball(tarball_path, extract_dir.clone()).await {
        tracing::error!(deployment_id = %deployment_id, error = %e, "tar extraction failed");
        set_failed(&pool, deployment_id, &format!("tar: {e}")).await;
        return;
    }

    // Inject .well-known/web-agency.json so reachability checks can verify the site
    inject_well_known(&extract_dir, "web-agency-pages").await;

    let mut wrangler_args = vec![
        "pages".to_string(),
        "deploy".to_string(),
        ".".to_string(),
        format!("--project-name={project_name}"),
    ];
    if let Some(ref branch) = branch {
        wrangler_args.push(format!("--branch={branch}"));
    }

    tracing::info!(
        deployment_id = %deployment_id,
        project = %project_name,
        branch = %branch_label,
        args = ?wrangler_args,
        "running wrangler pages deploy"
    );
    let wrangler_start = std::time::Instant::now();

    let wrangler_result = tokio::process::Command::new("wrangler")
        .args(&wrangler_args)
        .current_dir(&extract_dir)
        .env("CLOUDFLARE_API_TOKEN", &cf_token)
        .env("CLOUDFLARE_ACCOUNT_ID", &account_id)
        .output()
        .await;

    let wrangler_elapsed = wrangler_start.elapsed();

    match wrangler_result {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            tracing::info!(
                deployment_id = %deployment_id,
                project = %project_name,
                elapsed_ms = wrangler_elapsed.as_millis(),
                stdout = %stdout.trim(),
                "deployment succeeded"
            );
            let _ = sqlx::query(
                "UPDATE deployments SET status = 'success', updated_at = now() WHERE id = $1",
            )
            .bind(deployment_id)
            .execute(&pool)
            .await;
            super::counters::COUNTERS
                .deploy_success
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let exit_code = out.status.code().unwrap_or(-1);
            tracing::error!(
                deployment_id = %deployment_id,
                project = %project_name,
                exit_code = exit_code,
                elapsed_ms = wrangler_elapsed.as_millis(),
                stderr = %stderr.trim(),
                stdout = %stdout.trim(),
                "wrangler deployment failed"
            );
            set_failed(
                &pool,
                deployment_id,
                &format!("wrangler exit {exit_code}: {}", stderr.trim()),
            )
            .await;
        }
        Err(e) => {
            tracing::error!(
                deployment_id = %deployment_id,
                error = %e,
                "failed to execute wrangler (is wrangler installed and in PATH?)"
            );
            set_failed(&pool, deployment_id, &format!("wrangler exec: {e}")).await;
        }
    }

    tracing::debug!(deployment_id = %deployment_id, "cleaning up temp dir");
    // tmp_dir dropped here, auto-cleaned
}

/// Never let extraction drive the target filesystem below this much free space,
/// so a runaway/oversized deploy can't fill the disk and take the host down.
// ponytail: fixed floor; make configurable if deploy sizes vary widely.
const EXTRACT_MIN_FREE_BYTES: u64 = 256 * 1024 * 1024; // 256 MiB

/// Extract a gzipped tarball file into `dest` (created if missing). The archive
/// is read and decompressed straight from disk so it is never buffered in RAM.
///
/// Disk-space guarded: fails early if the (estimated uncompressed) archive won't
/// fit in the free space at `dest`, and aborts mid-extraction if free space falls
/// below [`EXTRACT_MIN_FREE_BYTES`] (checked periodically as entries are written).
async fn extract_tarball(src: std::path::PathBuf, dest: std::path::PathBuf) -> Result<(), String> {
    tokio::fs::create_dir_all(&dest)
        .await
        .map_err(|e| format!("mkdir: {e}"))?;

    // Early bail before doing any real work. gzip records the uncompressed size
    // in its 4-byte ISIZE trailer, so we get a tight estimate for O(1) — no need
    // to iterate/decompress the archive twice. ISIZE is mod 2^32, so it under-
    // reports for archives >4 GiB; fall back to the compressed size (always a
    // lower bound) via max(), and the in-loop guard below is the real backstop.
    let compressed_size = tokio::fs::metadata(&src)
        .await
        .map_err(|e| format!("stat tarball: {e}"))?
        .len();
    let needed = gzip_isize(&src).await.unwrap_or(0).max(compressed_size);
    let avail = fs4::available_space(&dest)
        .map_err(|e| format!("statvfs {}: {e}", dest.display()))?;
    // Require the estimate plus the same headroom floor we enforce mid-extraction.
    if needed.saturating_add(EXTRACT_MIN_FREE_BYTES) > avail {
        return Err(format!(
            "insufficient disk space: archive needs ~{needed} bytes (+{EXTRACT_MIN_FREE_BYTES} headroom) but only {avail} bytes free at {}",
            dest.display()
        ));
    }

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let f = std::fs::File::open(&src).map_err(|e| format!("open tarball: {e}"))?;
        let gz = flate2::read::GzDecoder::new(std::io::BufReader::new(f));
        let mut archive = tar::Archive::new(gz);
        archive.set_overwrite(true);
        // Unpack entry-by-entry (equivalent to Archive::unpack) so we can poll
        // free space as the tree grows and abort before filling the disk.
        for (i, entry) in archive
            .entries()
            .map_err(|e| format!("read tar entries: {e}"))?
            .enumerate()
        {
            let mut entry = entry.map_err(|e| format!("read tar entry: {e}"))?;
            // statvfs isn't free; poll every 32 entries rather than per file.
            if i % 32 == 0 {
                let avail = fs4::available_space(&dest)
                    .map_err(|e| format!("statvfs {}: {e}", dest.display()))?;
                if avail < EXTRACT_MIN_FREE_BYTES {
                    return Err(format!(
                        "disk space critical during extraction: {avail} bytes free at {} (below {EXTRACT_MIN_FREE_BYTES} floor), aborting",
                        dest.display()
                    ));
                }
            }
            entry
                .unpack_in(&dest)
                .map_err(|e| format!("unpack tar entry: {e}"))?;
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("extract task panicked: {e}"))?
}

/// Read a gzip file's uncompressed-size estimate from its ISIZE trailer (last 4
/// bytes, little-endian, mod 2^32). Returns `None` if the file is too short or
/// unreadable — callers should treat that as "unknown", not "empty".
async fn gzip_isize(path: &std::path::Path) -> Option<u64> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    let mut f = tokio::fs::File::open(path).await.ok()?;
    f.seek(std::io::SeekFrom::End(-4)).await.ok()?;
    let mut buf = [0u8; 4];
    f.read_exact(&mut buf).await.ok()?;
    Some(u32::from_le_bytes(buf) as u64)
}

/// Write the `.well-known/web-agency.json` marker so reachability checks can
/// verify the served site reports the expected service.
async fn inject_well_known(root: &std::path::Path, service: &str) {
    let dir = root.join(".well-known");
    let _ = tokio::fs::create_dir_all(&dir).await;
    let _ = tokio::fs::write(
        dir.join("web-agency.json"),
        format!("{{\"service\":\"{service}\"}}").into_bytes(),
    )
    .await;
}

/// Deploy a tarball to a local static webspace: extract into a staging dir
/// alongside the live directory, then atomically swap it into place.
async fn run_static_deploy(
    pool: PgPool,
    deployment_id: Uuid,
    webspace_id: Uuid,
    tarball_path: std::path::PathBuf,
) {
    tracing::info!(deployment_id = %deployment_id, webspace_id = %webspace_id, "starting static deploy");
    let _ = sqlx::query("UPDATE deployments SET status = 'deploying', updated_at = now() WHERE id = $1")
        .bind(deployment_id)
        .execute(&pool)
        .await;

    let dest = crate::local_hosting::webspace_dir(webspace_id);
    let parent = crate::local_hosting::webroot();
    // Staging dir sits under the same root as `dest` so the final rename is
    // atomic (same filesystem) rather than a cross-device move.
    let staging = parent.join(format!("{webspace_id}.tmp-{deployment_id}"));

    if let Err(e) = tokio::fs::create_dir_all(&parent).await {
        set_failed(&pool, deployment_id, &format!("mkdir webroot: {e}")).await;
        return;
    }
    let _ = tokio::fs::remove_dir_all(&staging).await;

    if let Err(e) = extract_tarball(tarball_path, staging.clone()).await {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        set_failed(&pool, deployment_id, &format!("tar: {e}")).await;
        return;
    }

    inject_well_known(&staging, "web-agency-proxy").await;

    // Atomic swap: move the live dir aside, promote staging, then drop the old.
    let dest2 = dest.clone();
    let staging2 = staging.clone();
    let swap = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let backup = dest2.with_extension("old");
        let _ = std::fs::remove_dir_all(&backup);
        if dest2.exists() {
            std::fs::rename(&dest2, &backup).map_err(|e| format!("rename live aside: {e}"))?;
        }
        if let Err(e) = std::fs::rename(&staging2, &dest2) {
            // Roll back the live directory if promotion failed.
            if backup.exists() {
                let _ = std::fs::rename(&backup, &dest2);
            }
            return Err(format!("promote staging: {e}"));
        }
        let _ = std::fs::remove_dir_all(&backup);
        Ok(())
    })
    .await;

    match swap {
        Ok(Ok(())) => {
            tracing::info!(deployment_id = %deployment_id, dir = %dest.display(), "static deploy succeeded");
            let _ = sqlx::query("UPDATE deployments SET status = 'success', updated_at = now() WHERE id = $1")
                .bind(deployment_id)
                .execute(&pool)
                .await;
            super::counters::COUNTERS
                .deploy_success
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(Err(e)) => {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            set_failed(&pool, deployment_id, &e).await;
        }
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            set_failed(&pool, deployment_id, &format!("swap task panicked: {e}")).await;
        }
    }
}

async fn set_failed(pool: &PgPool, deployment_id: Uuid, msg: &str) {
    tracing::error!(deployment_id = %deployment_id, error = msg, "deployment failed");
    let _ = sqlx::query("UPDATE deployments SET status = 'failed', error_message = $1, updated_at = now() WHERE id = $2")
        .bind(msg).bind(deployment_id).execute(pool).await;
    super::counters::COUNTERS
        .deploy_failed
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}
