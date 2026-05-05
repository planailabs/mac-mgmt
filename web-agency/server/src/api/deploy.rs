//! Deploy API: upload tarballs and deploy to Cloudflare Pages via wrangler.
//!
//! - `POST /api/v1/deploy/:webspace_id` — upload a tarball, starts background deploy
//! - `GET  /api/v1/deploy/:webspace_id/status` — check latest deployment status
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
use uuid::Uuid;

/// Shared state for the deploy API.
#[derive(Clone)]
pub struct DeployState {
    pub pool: PgPool,
}

pub fn router(state: DeployState) -> Router<()> {
    Router::new()
        .route("/api/v1/deploy/whoami", get(whoami))
        .route("/api/v1/deploy/{webspace_id}", post(upload_deploy))
        .route("/api/v1/deploy/{webspace_id}/status", get(deploy_status))
        .with_state(state)
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

    // Check webspace_id scope
    if let Some(scopes) = scopes {
        let allowed = scopes
            .get("webspace_id")
            .and_then(|v| v.as_str())
            .map(|id| id == webspace_id.to_string())
            .unwrap_or(false);
        // Also allow wildcard (no webspace_id in scopes = all webspaces)
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

#[derive(Serialize)]
struct UploadResponse {
    deployment_id: Uuid,
    status: String,
}

#[derive(serde::Deserialize, Default)]
struct UploadQuery {
    #[serde(default)]
    branch: Option<String>,
}

async fn upload_deploy(
    State(state): State<DeployState>,
    Path(webspace_id): Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<UploadQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    authenticate_deploy(&state.pool, &headers, webspace_id).await?;

    // Verify webspace exists and is a direct-upload Pages project
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
    let project_name = project_name
        .ok_or((StatusCode::BAD_REQUEST, "Pages project not deployed yet".into()))?;
    let cred_id = cred_id
        .ok_or((StatusCode::BAD_REQUEST, "no Cloudflare credential linked".into()))?;

    let tarball_size = body.len() as i64;

    // Create deployment record
    let deployment_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO deployments (webspace_id, status, tarball_size) VALUES ($1, 'uploading', $2) RETURNING id",
    )
    .bind(webspace_id)
    .bind(tarball_size)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Get CF credential for wrangler
    let cf_token = get_cf_token(&state.pool, cred_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Spawn background deploy task
    let pool = state.pool.clone();
    let branch = query.branch;
    tokio::spawn(async move {
        run_wrangler_deploy(pool, deployment_id, project_name, cf_token, branch, body).await;
    });

    Ok(Json(UploadResponse {
        deployment_id,
        status: "uploading".into(),
    }))
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
    let encrypted = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT encrypted_data FROM credentials WHERE id = $1",
    )
    .bind(cred_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let decrypted = crate::crypto::decrypt(&encrypted)
        .map_err(|e| format!("decryption: {e}"))?;
    let data: serde_json::Value = serde_json::from_slice(&decrypted)
        .map_err(|e| format!("parse: {e}"))?;

    data["api_token"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| "missing api_token".into())
}

async fn run_wrangler_deploy(
    pool: PgPool,
    deployment_id: Uuid,
    project_name: String,
    cf_token: String,
    branch: Option<String>,
    tarball: Bytes,
) {
    // Update status to deploying
    let _ = sqlx::query("UPDATE deployments SET status = 'deploying', updated_at = now() WHERE id = $1")
        .bind(deployment_id).execute(&pool).await;

    // Write tarball to temp dir and extract
    let tmp_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            set_failed(&pool, deployment_id, &format!("tempdir: {e}")).await;
            return;
        }
    };

    let tarball_path = tmp_dir.path().join("upload.tar.gz");
    if let Err(e) = tokio::fs::write(&tarball_path, &tarball).await {
        set_failed(&pool, deployment_id, &format!("write tarball: {e}")).await;
        return;
    }

    let extract_dir = tmp_dir.path().join("site");
    if let Err(e) = tokio::fs::create_dir_all(&extract_dir).await {
        set_failed(&pool, deployment_id, &format!("mkdir: {e}")).await;
        return;
    }

    // Extract tarball
    let tar_status = tokio::process::Command::new("tar")
        .args(["xzf", tarball_path.to_str().unwrap_or("upload.tar.gz"), "-C"])
        .arg(&extract_dir)
        .output()
        .await;

    match tar_status {
        Ok(out) if !out.status.success() => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            set_failed(&pool, deployment_id, &format!("tar extract failed: {stderr}")).await;
            return;
        }
        Err(e) => {
            set_failed(&pool, deployment_id, &format!("tar: {e}")).await;
            return;
        }
        _ => {}
    }

    // Run wrangler pages deploy
    let mut wrangler_args = vec![
        "wrangler".to_string(),
        "pages".to_string(),
        "deploy".to_string(),
        ".".to_string(),
        format!("--project-name={project_name}"),
    ];
    if let Some(ref branch) = branch {
        wrangler_args.push(format!("--branch={branch}"));
    }
    let wrangler_result = tokio::process::Command::new("npx")
        .args(&wrangler_args)
        .current_dir(&extract_dir)
        .env("CLOUDFLARE_API_TOKEN", &cf_token)
        .output()
        .await;

    match wrangler_result {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            tracing::info!("wrangler deploy success for {project_name}: {stdout}");
            let _ = sqlx::query("UPDATE deployments SET status = 'success', updated_at = now() WHERE id = $1")
                .bind(deployment_id).execute(&pool).await;
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            set_failed(&pool, deployment_id, &format!("wrangler failed: {stderr}\n{stdout}")).await;
        }
        Err(e) => {
            set_failed(&pool, deployment_id, &format!("wrangler exec: {e}")).await;
        }
    }
}

async fn set_failed(pool: &PgPool, deployment_id: Uuid, msg: &str) {
    tracing::error!("deployment {deployment_id} failed: {msg}");
    let _ = sqlx::query("UPDATE deployments SET status = 'failed', error_message = $1, updated_at = now() WHERE id = $2")
        .bind(msg).bind(deployment_id).execute(pool).await;
}
