//! Static file serving runtime - nginx serves files directly.

use uuid::Uuid;

/// Configure static file serving for this webspace.
///
/// Content is deployed (via tarball upload) to `local_hosting::webspace_dir`.
/// Wiring an actual file server / nginx vhost to that directory is still a stub.
pub fn configure(webspace_id: Uuid) -> Result<(), String> {
    let dir = crate::local_hosting::webspace_dir(webspace_id);
    tracing::info!(dir = %dir.display(), "STUB: would serve static files for webspace {webspace_id}");
    Ok(())
}
