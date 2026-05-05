//! Static file serving runtime - nginx serves files directly.

use uuid::Uuid;

/// Configure nginx to serve static files for this webspace.
pub fn configure(webspace_id: Uuid) -> Result<(), String> {
    // STUB: nginx root directive points to /var/www/webspace-{id}/
    tracing::info!("STUB: would configure static file serving for webspace {webspace_id}");
    Ok(())
}
