//! Local webspace hosting orchestrator.
//!
//! Manages nginx reverse proxy, ACME certificate provisioning, and
//! runtime processes (static, Node.js, Docker) for locally-hosted webspaces.

pub mod acme;
pub mod nginx;
pub mod runtime;

use uuid::Uuid;

/// Provision a local webspace: allocate a port, generate nginx config, request ACME cert.
pub async fn provision(webspace_id: Uuid, domains: &[String], runtime: &str, port: u16) -> Result<(), String> {
    nginx::generate_config(webspace_id, domains, port)?;
    acme::request_certificate(domains)?;
    runtime::start(webspace_id, runtime, port).await?;
    Ok(())
}

/// Stop a local webspace runtime.
pub async fn stop(webspace_id: Uuid) -> Result<(), String> {
    runtime::stop(webspace_id).await
}

/// Destroy a local webspace: stop runtime, remove nginx config.
pub async fn destroy(webspace_id: Uuid) -> Result<(), String> {
    let _ = runtime::stop(webspace_id).await;
    nginx::remove_config(webspace_id)?;
    Ok(())
}
