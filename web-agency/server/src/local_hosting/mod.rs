//! Local webspace hosting: filesystem layout for locally-served webspaces.
//!
//! Static content is deployed to per-webspace directories under the
//! configured webroot. TLS termination and reverse proxying are handled by
//! the separate `web-agency-proxy` binary (Pingora-based). Process runtimes
//! (Node.js, Docker) were unwired stubs and have been removed; restore from
//! git history if that feature lands.

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
