//! Local webspace hosting orchestrator.
//!
//! Manages runtime processes (static, Node.js, Docker) for locally-hosted
//! webspaces. TLS termination and reverse proxying are handled by the
//! separate `web-agency-proxy` binary (Pingora-based).

pub mod runtime;

use std::path::PathBuf;
use uuid::Uuid;

/// Root directory under which static webspace folders are served.
/// Each webspace gets `{webroot}/{webspace_id}/`. Set via `[local_hosting] dir`
/// in the server config (required).
pub fn webroot() -> PathBuf {
    PathBuf::from(&crate::config::config().local_hosting.dir)
}

/// Filesystem directory that serves a given webspace's static content.
pub fn webspace_dir(webspace_id: Uuid) -> PathBuf {
    webroot().join(webspace_id.to_string())
}

/// Ensure the webroot and a directory for every local webspace exist. Called
/// once at startup so static folders have a directory to serve even before
/// their first deploy.
pub async fn ensure_webspace_dirs(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let root = webroot();
    tokio::fs::create_dir_all(&root).await?;

    let ids =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM webspaces WHERE hosting_type = 'local'")
            .fetch_all(pool)
            .await?;
    for id in &ids {
        if let Err(e) = tokio::fs::create_dir_all(webspace_dir(*id)).await {
            tracing::warn!(webspace_id = %id, "failed to create webspace dir: {e}");
        }
    }
    tracing::info!(root = %root.display(), count = ids.len(), "ensured local webspace dirs");
    Ok(())
}

/// Provision a local webspace: start the runtime process.
/// The proxy discovers the new route via its SSE connection to the server.
pub async fn provision(webspace_id: Uuid, runtime: &str, port: u16) -> Result<(), String> {
    runtime::start(webspace_id, runtime, port).await
}

/// Stop a local webspace runtime.
pub async fn stop(webspace_id: Uuid) -> Result<(), String> {
    runtime::stop(webspace_id).await
}

/// Destroy a local webspace: stop runtime.
/// The proxy drops the route on its next refresh.
pub async fn destroy(webspace_id: Uuid) -> Result<(), String> {
    let _ = runtime::stop(webspace_id).await;
    Ok(())
}
