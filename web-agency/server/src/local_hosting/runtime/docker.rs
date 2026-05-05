//! Docker runtime stub - managed via systemd-run.

use uuid::Uuid;

/// Start a Docker container for this webspace.
pub async fn start(webspace_id: Uuid, port: u16) -> Result<(), String> {
    // STUB: systemd-run --unit=webspace-{id} docker run -p {port}:{container_port} {image}
    tracing::info!("STUB: would start Docker runtime for webspace {webspace_id} on port {port}");
    Ok(())
}
