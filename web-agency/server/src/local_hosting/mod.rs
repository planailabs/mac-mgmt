//! Local webspace hosting orchestrator.
//!
//! Manages runtime processes (static, Node.js, Docker) for locally-hosted
//! webspaces. TLS termination and reverse proxying are handled by the
//! separate `web-agency-proxy` binary (Pingora-based).

pub mod runtime;

use std::path::PathBuf;
use uuid::Uuid;

/// Root directory under which static webspace folders are served.
/// Each webspace gets `{webroot}/{webspace_id}/`. Configurable via the
/// `WEB_AGENCY_WEBROOT` env var.
pub fn webroot() -> PathBuf {
    std::env::var("WEB_AGENCY_WEBROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/www/web-agency"))
}

/// Filesystem directory that serves a given webspace's static content.
pub fn webspace_dir(webspace_id: Uuid) -> PathBuf {
    webroot().join(webspace_id.to_string())
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
