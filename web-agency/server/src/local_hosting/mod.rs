//! Local webspace hosting orchestrator.
//!
//! Manages runtime processes (static, Node.js, Docker) for locally-hosted
//! webspaces. TLS termination and reverse proxying are handled by the
//! separate `web-agency-proxy` binary (Pingora-based).

pub mod runtime;

use uuid::Uuid;

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
