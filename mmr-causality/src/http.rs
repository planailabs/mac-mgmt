//! mmrcd HTTP API (axum) with static bearer-token auth.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::api::*;
use crate::orchestrator::{Orchestrator, resolve_images};

#[derive(Clone)]
struct AppState {
    orch: Arc<Orchestrator>,
    token: String,
    // Monotonic run counter seed derived from process start; run ids are
    // counter-based (no Math.random/time needed for uniqueness within a run).
    counter: Arc<std::sync::atomic::AtomicU64>,
}

fn authed(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t == token)
        .unwrap_or(false)
}

fn err(status: StatusCode, msg: impl std::fmt::Display) -> (StatusCode, String) {
    (status, msg.to_string())
}

pub async fn serve(orch: Arc<Orchestrator>) -> anyhow::Result<()> {
    let listen = orch.config().listen.clone();
    let token = orch.config().token.clone();
    let gc_interval = orch.config().gc_interval_secs;

    // Periodic garbage collection of stale run projects (also runs at startup).
    {
        let orch = orch.clone();
        tokio::spawn(async move {
            loop {
                match orch.gc_projects().await {
                    Ok(n) if n > 0 => tracing::info!("gc: reaped {n} stale project(s)"),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("gc sweep failed: {e:#}"),
                }
                tokio::time::sleep(std::time::Duration::from_secs(gc_interval)).await;
            }
        });
    }

    let state = AppState {
        orch,
        token,
        counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
    };
    let app = Router::new()
        .route("/images", get(images))
        .route("/runs", post(create_run))
        .route("/runs/{id}", get(get_run).delete(delete_run))
        .route("/runs/{id}/nodes", post(spawn_nodes))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(&listen).await?;
    tracing::info!("mmrcd listening on {listen}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn images(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ImageStatus>, (StatusCode, String)> {
    if !authed(&headers, &st.token) {
        return Err(err(StatusCode::UNAUTHORIZED, "bad token"));
    }
    let git_ref = st.orch.config().default_ref.clone();
    resolve_images(st.orch.config(), &git_ref)
        .await
        .map(Json)
        .map_err(|e| err(StatusCode::BAD_GATEWAY, e))
}

async fn create_run(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateRunRequest>,
) -> Result<Json<RunInfo>, (StatusCode, String)> {
    if !authed(&headers, &st.token) {
        return Err(err(StatusCode::UNAUTHORIZED, "bad token"));
    }
    let n = st.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let run_id = format!("r{n:04}");
    st.orch
        .create_run(&run_id, &req)
        .await
        .map(Json)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn get_run(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<RunInfo>, (StatusCode, String)> {
    if !authed(&headers, &st.token) {
        return Err(err(StatusCode::UNAUTHORIZED, "bad token"));
    }
    let s = st
        .orch
        .load_state(&id)
        .map_err(|e| err(StatusCode::NOT_FOUND, e))?;
    Ok(Json(RunInfo {
        run_id: s.run_id,
        image_tag: s.image_tag,
        servers: s.servers,
        instances: s.node_instances,
        status: s.status,
    }))
}

async fn delete_run(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    if !authed(&headers, &st.token) {
        return Err(err(StatusCode::UNAUTHORIZED, "bad token"));
    }
    st.orch
        .delete_run(&id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn spawn_nodes(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<SpawnNodesRequest>,
) -> Result<Json<SpawnNodesResponse>, (StatusCode, String)> {
    if !authed(&headers, &st.token) {
        return Err(err(StatusCode::UNAUTHORIZED, "bad token"));
    }
    let names = st
        .orch
        .spawn_nodes(&id, &req)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    // The response carries the container names; the client resolves instance
    // ids from the server's machine list.
    Ok(Json(SpawnNodesResponse {
        instance_ids: names,
    }))
}
