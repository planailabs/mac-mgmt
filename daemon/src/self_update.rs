use anyhow::{Context, Result};
use std::io::Write;

use crate::sentry_ext;

const UPDATE_BASE: &str = env!("UPDATE_BASE_URL");
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const ENVIRONMENT: &str = env!("ENVIRONMENT");
const TARGET: &str = env!("TARGET");

pub fn check_and_apply() {
    tracing::info!("checking for updates (current: {CURRENT_VERSION})");
    sentry_ext::breadcrumb("self-update", "checking for updates", &[("version", CURRENT_VERSION)]);

    if let Err(e) = apply(false) {
        tracing::warn!("update failed: {e}");
        sentry_ext::capture_error(
            &format!("self-update failed: {e}"),
            &[("version", CURRENT_VERSION)],
        );
    }
}

fn version_url() -> String {
    format!("{UPDATE_BASE}/{ENVIRONMENT}/mac-mgmt.version")
}

fn archive_url() -> String {
    format!("{UPDATE_BASE}/{ENVIRONMENT}/mac-mgmt.tar.gz")
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

    if !force && remote_version == CURRENT_VERSION {
        tracing::info!("already up to date ({CURRENT_VERSION})");
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

    // Replace the running binary
    self_replace::self_replace(&new_bin).context("failed to replace binary")?;

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
        let url = version_url();
        assert!(url.starts_with(UPDATE_BASE));
        assert!(url.contains(ENVIRONMENT));
        assert!(url.ends_with("/mac-mgmt.version"));
    }

    #[test]
    fn archive_url_format() {
        let url = archive_url();
        assert!(url.starts_with(UPDATE_BASE));
        assert!(url.contains(ENVIRONMENT));
        assert!(url.ends_with("/mac-mgmt.tar.gz"));
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
