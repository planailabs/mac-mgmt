//! Deploy API: upload tarballs and deploy to Cloudflare Pages via wrangler.
//!
//! - `POST /api/v1/deploy/:webspace_id` — upload a tarball, starts background deploy
//! - `GET  /api/v1/deploy/:webspace_id/status` — check latest deployment status
//! - `GET  /api/v1/deploy/whoami` — token info and scoped webspace
//!
//! Auth: Bearer token with kind="deploy" and scopes containing the webspace_id.

use dioxus::fullstack::axum::{
    self as axum,
    Router,
    body::Bytes,
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

pub fn router(state: DeployState) -> Router<()> {
    Router::new()
        .route("/api/v1/deploy/whoami", get(whoami))
        .route("/api/v1/deploy/{webspace_id}", post(upload_deploy))
        .route("/api/v1/deploy/{webspace_id}/status", get(deploy_status))
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
            tracing::warn!("marked {} interrupted deployment(s) as failed", r.rows_affected());
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

async fn authenticate_deploy(pool: &PgPool, headers: &HeaderMap, webspace_id: Uuid) -> Result<(), (StatusCode, String)> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or((StatusCode::UNAUTHORIZED, "missing Bearer token".into()))?;

    use sha2::{Sha256, Digest};
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
        let is_wildcard = !scopes.as_object().map(|o| o.contains_key("webspace_id")).unwrap_or(false);
        if !allowed && !is_wildcard {
            return Err((StatusCode::FORBIDDEN, "token not scoped to this webspace".into()));
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

    use sha2::{Sha256, Digest};
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
    let ws_id = scopes.as_ref()
        .and_then(|s| s.get("webspace_id"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let ws_name = if let Some(ref wid) = ws_id {
        if let Ok(uid) = uuid::Uuid::parse_str(wid) {
            sqlx::query_scalar::<_, String>("SELECT name FROM webspaces WHERE id = $1")
                .bind(uid).fetch_optional(&state.pool).await.ok().flatten()
        } else { None }
    } else { None };

    Ok(Json(WhoamiResponse { kind, webspace_id: ws_id, webspace_name: ws_name }))
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
    body: Bytes,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    authenticate_deploy(&state.pool, &headers, webspace_id).await?;

    let ws = sqlx::query_as::<_, (String, Option<String>, Option<Uuid>)>(
        "SELECT hosting_type, cloudflare_pages_project, cloudflare_credential_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "webspace not found".into()))?;

    let (hosting_type, project_name, cred_id) = ws;
    if hosting_type != "cloudflare_pages" {
        return Err((StatusCode::BAD_REQUEST, "webspace is not a Cloudflare Pages project".into()));
    }
    let project_name = project_name.ok_or((StatusCode::BAD_REQUEST, "Pages project not deployed yet".into()))?;
    let cred_id = cred_id.ok_or((StatusCode::BAD_REQUEST, "no Cloudflare credential linked".into()))?;

    let tarball_size = body.len() as i64;
    tracing::info!(
        webspace_id = %webspace_id,
        project = %project_name,
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

    let cf_token = get_cf_token(&state.pool, cred_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // If no branch specified, fetch the production branch from the CF Pages project
    let branch = if query.branch.is_some() {
        query.branch
    } else {
        let account_id = get_cf_account_id(&state.pool, cred_id).await.unwrap_or_default();
        if !account_id.is_empty() {
            let client = cloudflare_api::Client::new(&cf_token);
            match client.get_pages_project(&account_id, &project_name).await {
                Ok(project) => {
                    let pb = project.production_branch.unwrap_or_else(|| "main".into());
                    tracing::info!(deployment_id = %deployment_id, branch = %pb, "using production branch from CF Pages project");
                    Some(pb)
                }
                Err(e) => {
                    tracing::warn!(deployment_id = %deployment_id, error = %e, "could not fetch production branch, defaulting to 'main'");
                    Some("main".into())
                }
            }
        } else {
            tracing::warn!(deployment_id = %deployment_id, "no account_id in credential, defaulting branch to 'main'");
            Some("main".into())
        }
    };

    let pool = state.pool.clone();
    let active = state.active_deploys.clone();
    active.fetch_add(1, Ordering::Relaxed);
    tokio::spawn(async move {
        run_wrangler_deploy(pool, deployment_id, project_name, cf_token, branch, body).await;
        active.fetch_sub(1, Ordering::Relaxed);
    });

    Ok(Json(UploadResponse { deployment_id, status: "uploading".into() }))
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

    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<i64>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
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
        deployment_id, status, error_message, tarball_size,
        created_at: created_at.to_rfc3339(), updated_at: updated_at.to_rfc3339(),
    }))
}

// ── Background deploy ─────────────────────────────────────────────────

async fn get_cf_token(pool: &PgPool, cred_id: Uuid) -> Result<String, String> {
    let encrypted = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT encrypted_data FROM credentials WHERE id = $1",
    )
    .bind(cred_id).fetch_one(pool).await.map_err(|e| e.to_string())?;

    let decrypted = crate::crypto::decrypt(&encrypted).map_err(|e| format!("decryption: {e}"))?;
    let data: serde_json::Value = serde_json::from_slice(&decrypted).map_err(|e| format!("parse: {e}"))?;
    data["api_token"].as_str().map(String::from).ok_or_else(|| "missing api_token".into())
}

async fn get_cf_account_id(pool: &PgPool, cred_id: Uuid) -> Result<String, String> {
    let encrypted = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT encrypted_data FROM credentials WHERE id = $1",
    )
    .bind(cred_id).fetch_one(pool).await.map_err(|e| e.to_string())?;

    let decrypted = crate::crypto::decrypt(&encrypted).map_err(|e| format!("decryption: {e}"))?;
    let data: serde_json::Value = serde_json::from_slice(&decrypted).map_err(|e| format!("parse: {e}"))?;
    Ok(data["account_id"].as_str().unwrap_or("").to_string())
}

async fn run_wrangler_deploy(
    pool: PgPool,
    deployment_id: Uuid,
    project_name: String,
    cf_token: String,
    branch: Option<String>,
    tarball: Bytes,
) {
    let branch_label = branch.as_deref().unwrap_or("production");
    let tarball_len = tarball.len();
    tracing::info!(
        deployment_id = %deployment_id,
        project = %project_name,
        branch = %branch_label,
        tarball_bytes = tarball_len,
        "starting deployment"
    );

    let _ = sqlx::query("UPDATE deployments SET status = 'deploying', updated_at = now() WHERE id = $1")
        .bind(deployment_id).execute(&pool).await;

    let tmp_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => { set_failed(&pool, deployment_id, &format!("tempdir: {e}")).await; return; }
    };
    tracing::debug!(deployment_id = %deployment_id, dir = %tmp_dir.path().display(), "created temp dir");

    let tarball_path = tmp_dir.path().join("upload.tar.gz");
    if let Err(e) = tokio::fs::write(&tarball_path, &tarball).await {
        set_failed(&pool, deployment_id, &format!("write tarball: {e}")).await;
        return;
    }
    tracing::debug!(deployment_id = %deployment_id, bytes = tarball_len, "wrote tarball to disk");

    let extract_dir = tmp_dir.path().join("site");
    if let Err(e) = tokio::fs::create_dir_all(&extract_dir).await {
        set_failed(&pool, deployment_id, &format!("mkdir: {e}")).await;
        return;
    }

    tracing::info!(deployment_id = %deployment_id, "extracting tarball");
    let tar_start = std::time::Instant::now();
    let tar_status = tokio::process::Command::new("tar")
        .args(["xzf", tarball_path.to_str().unwrap_or("upload.tar.gz"), "-C"])
        .arg(&extract_dir)
        .output()
        .await;

    match tar_status {
        Ok(out) if !out.status.success() => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            tracing::error!(deployment_id = %deployment_id, stderr = %stderr, "tar extraction failed");
            set_failed(&pool, deployment_id, &format!("tar: {stderr}")).await;
            return;
        }
        Err(e) => {
            tracing::error!(deployment_id = %deployment_id, error = %e, "tar command failed to execute");
            set_failed(&pool, deployment_id, &format!("tar: {e}")).await;
            return;
        }
        _ => {
            tracing::info!(
                deployment_id = %deployment_id,
                elapsed_ms = tar_start.elapsed().as_millis(),
                "tarball extracted"
            );
        }
    }

    // Count extracted files for the log
    if let Ok(mut entries) = tokio::fs::read_dir(&extract_dir).await {
        let mut count = 0u32;
        while entries.next_entry().await.ok().flatten().is_some() {
            count += 1;
        }
        tracing::info!(deployment_id = %deployment_id, files = count, "extracted files in root");
    }

    let mut wrangler_args = vec![
        "pages".to_string(), "deploy".to_string(),
        ".".to_string(), format!("--project-name={project_name}"),
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
            let _ = sqlx::query("UPDATE deployments SET status = 'success', updated_at = now() WHERE id = $1")
                .bind(deployment_id).execute(&pool).await;
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
            set_failed(&pool, deployment_id, &format!("wrangler exit {exit_code}: {}", stderr.trim())).await;
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

async fn set_failed(pool: &PgPool, deployment_id: Uuid, msg: &str) {
    tracing::error!(deployment_id = %deployment_id, error = msg, "deployment failed");
    let _ = sqlx::query("UPDATE deployments SET status = 'failed', error_message = $1, updated_at = now() WHERE id = $2")
        .bind(msg).bind(deployment_id).execute(pool).await;
}
