//! Runtime management for local webspaces.
//!
//! Runtimes are managed via systemd-run for process lifecycle.

pub mod docker;
pub mod nodejs;
pub mod static_files;

use uuid::Uuid;

/// Start a runtime for a webspace.
pub async fn start(webspace_id: Uuid, runtime: &str, port: u16) -> Result<(), String> {
    match runtime {
        "static" => {
            static_files::configure(webspace_id)?;
            Ok(())
        }
        "nodejs" => nodejs::start(webspace_id, port).await,
        "docker" => docker::start(webspace_id, port).await,
        _ => Err(format!("unknown runtime: {runtime}")),
    }
}

/// Stop a runtime for a webspace.
pub async fn stop(webspace_id: Uuid) -> Result<(), String> {
    // STUB: systemctl stop webspace-{id}
    tracing::info!("STUB: would stop runtime for webspace {webspace_id}");
    Ok(())
}
