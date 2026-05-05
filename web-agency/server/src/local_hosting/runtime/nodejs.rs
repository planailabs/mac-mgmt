//! Node.js runtime stub - managed via systemd-run.

use uuid::Uuid;

/// Start a Node.js application for this webspace.
pub async fn start(webspace_id: Uuid, port: u16) -> Result<(), String> {
    // STUB: systemd-run --unit=webspace-{id} node {entry} with PORT={port}
    tracing::info!("STUB: would start Node.js runtime for webspace {webspace_id} on port {port}");
    Ok(())
}
