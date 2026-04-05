use anyhow::{Context, Result};
use std::io::Write;

use crate::sentry_ext;

const UPDATE_BASE: &str = env!("UPDATE_BASE_URL");
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const ENVIRONMENT: &str = env!("ENVIRONMENT");
const TARGET: &str = env!("TARGET");

/// Effective update base URL: runtime `MAC_MGMT_UPDATE_URL` overrides the
/// compile-time default.  This lets integration tests point at a local server.
fn update_base() -> String {
    std::env::var("MAC_MGMT_UPDATE_URL").unwrap_or_else(|_| UPDATE_BASE.to_string())
}

/// Effective environment: if a runtime override is stored (fetched from
/// server), use it; otherwise fall back to compile-time ENVIRONMENT.
static RUNTIME_ENVIRONMENT: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

/// Set the runtime environment override (called from daemon after fetching from server).
pub fn set_environment(env: String) {
    *RUNTIME_ENVIRONMENT.write().unwrap() = Some(env);
}

static PINNED_VERSION: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

/// Set the pinned target version (from server rollout).
/// If set, self-update will only apply this exact version (no downgrades).
pub fn set_pinned_version(ver: String) {
    *PINNED_VERSION.write().unwrap() = Some(ver);
}

fn pinned_version() -> Option<String> {
    PINNED_VERSION.read().unwrap().clone()
}

fn effective_environment() -> String {
    RUNTIME_ENVIRONMENT
        .read()
        .unwrap()
        .clone()
        .unwrap_or_else(|| ENVIRONMENT.to_string())
}

pub fn check_and_apply() {
    let env = effective_environment();
    tracing::info!("checking for updates (current: {CURRENT_VERSION}, env: {env})");
    sentry_ext::breadcrumb("self-update", "checking for updates", &[
        ("version", CURRENT_VERSION),
        ("environment", &env),
    ]);

    if let Err(e) = apply(false) {
        tracing::warn!("update failed: {e}");
        sentry_ext::capture_error(
            &format!("self-update failed: {e}"),
            &[("version", CURRENT_VERSION)],
        );
    }
}

fn version_url() -> String {
    let base = update_base();
    let env = effective_environment();
    format!("{base}/{env}/mac-mgmt.version")
}

fn archive_url() -> String {
    let base = update_base();
    let env = effective_environment();
    format!("{base}/{env}/mac-mgmt.tar.gz")
}

fn fetch_remote_version() -> Result<String> {
    let url = version_url();
    let mut body = Vec::new();
    let mut download = self_update::Download::from_url(&url);
    download.show_progress(false);
    download.download_to(&mut body)?;
    let version = String::from_utf8(body)
        .context("invalid UTF-8 in version file")?
        .trim()
        .to_string();
    Ok(version)
}

pub fn apply(force: bool) -> Result<()> {
    let remote_version = fetch_remote_version()?;

    // If a pinned version is set, only update to that exact version
    if let Some(ref pinned) = pinned_version() {
        if &remote_version != pinned {
            tracing::info!(
                "remote version {remote_version} != pinned {pinned}, skipping"
            );
            return Ok(());
        }
    }

    if !force && remote_version == CURRENT_VERSION {
        tracing::info!("already up to date ({CURRENT_VERSION})");
        return Ok(());
    }

    // Prevent downgrades
    if !force && remote_version < *CURRENT_VERSION {
        tracing::info!(
            "remote version {remote_version} is older than current {CURRENT_VERSION}, skipping"
        );
        return Ok(());
    }

    tracing::info!("update available: {CURRENT_VERSION} -> {remote_version}");
    sentry_ext::breadcrumb("self-update", "downloading update", &[
        ("from", CURRENT_VERSION),
        ("to", &remote_version),
    ]);

    let url = archive_url();
    let mut tmp_archive = tempfile::Builder::new()
        .suffix(".tar.gz")
        .tempfile()
        .context("failed to create temp file")?;

    // Download the tarball
    tracing::info!("downloading {url}");
    let mut download = self_update::Download::from_url(&url);
    download.show_progress(false);
    let mut body = Vec::new();
    download.download_to(&mut body)?;
    tmp_archive.write_all(&body)?;
    tmp_archive.flush()?;

    // Extract the binary from the archive
    let tmp_dir = tempfile::tempdir().context("failed to create temp dir")?;
    self_update::Extract::from_source(tmp_archive.path())
        .archive(self_update::ArchiveKind::Tar(Some(
            self_update::Compression::Gz,
        )))
        .extract_into(tmp_dir.path())?;

    let bin_name = format!("mac-mgmt-{TARGET}");
    let new_bin = tmp_dir.path().join(&bin_name);
    if !new_bin.exists() {
        anyhow::bail!("binary '{bin_name}' not found in archive");
    }

    // Replace the running binary.
    // self_replace can fail with "text file busy" on some systems, so fall back
    // to a manual rename-over approach: copy new binary next to the current one
    // with a temp name, then atomically rename over it.
    if let Err(e) = self_replace::self_replace(&new_bin) {
        tracing::warn!("self_replace failed ({e}), falling back to rename-over");
        let current_exe = std::env::current_exe().context("failed to get current exe path")?;
        let parent = current_exe.parent().context("current exe has no parent dir")?;
        let tmp_target = parent.join(".mac-mgmt.update");
        std::fs::copy(&new_bin, &tmp_target)
            .context("failed to copy new binary to temp location")?;

        // Set executable permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp_target, std::fs::Permissions::from_mode(0o755))
                .context("failed to set permissions on new binary")?;
        }

        std::fs::rename(&tmp_target, &current_exe)
            .context("failed to rename new binary over current")?;
    }

    tracing::info!("binary updated successfully");
    sentry_ext::breadcrumb("self-update", "binary updated", &[
        ("from", CURRENT_VERSION),
        ("to", &remote_version),
    ]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_base_is_set() {
        assert!(!UPDATE_BASE.is_empty(), "UPDATE_BASE_URL must be set at build time");
    }

    #[test]
    fn default_update_base_is_plan_ai() {
        // When built without overriding UPDATE_BASE_URL, the default should be used
        assert_eq!(UPDATE_BASE, "https://update.plan.ai");
    }

    #[test]
    fn version_url_format() {
        // Ensure no runtime override interferes
        unsafe { std::env::remove_var("MAC_MGMT_UPDATE_URL") };
        let url = version_url();
        assert!(url.starts_with(UPDATE_BASE));
        assert!(url.contains(ENVIRONMENT));
        assert!(url.ends_with("/mac-mgmt.version"));
    }

    #[test]
    fn archive_url_format() {
        unsafe { std::env::remove_var("MAC_MGMT_UPDATE_URL") };
        let url = archive_url();
        assert!(url.starts_with(UPDATE_BASE));
        assert!(url.contains(ENVIRONMENT));
        assert!(url.ends_with("/mac-mgmt.tar.gz"));
    }

    #[test]
    fn runtime_override_takes_precedence() {
        unsafe { std::env::set_var("MAC_MGMT_UPDATE_URL", "http://localhost:9999") };
        assert_eq!(update_base(), "http://localhost:9999");
        let url = version_url();
        assert!(url.starts_with("http://localhost:9999"));
        unsafe { std::env::remove_var("MAC_MGMT_UPDATE_URL") };
    }

    #[test]
    fn current_version_is_valid_semver() {
        assert!(
            CURRENT_VERSION.split('.').count() >= 3,
            "CURRENT_VERSION should be semver: {CURRENT_VERSION}"
        );
    }

    #[test]
    fn environment_is_set() {
        assert!(!ENVIRONMENT.is_empty());
    }

    #[test]
    fn target_is_set() {
        assert!(!TARGET.is_empty());
    }
}
