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

/// The version the server wants us to run. Set by the daemon after
/// fetching from the server's `/api/update` endpoint.
static TARGET_VERSION: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

pub fn set_target_version(ver: String) {
    *TARGET_VERSION.write().unwrap() = Some(ver);
}

fn target_version() -> Option<String> {
    TARGET_VERSION.read().unwrap().clone()
}

pub fn current_version() -> &'static str {
    CURRENT_VERSION
}

/// Check if the server has assigned a target version and apply it.
pub fn check_and_apply() {
    let target = match target_version() {
        Some(v) => v,
        None => {
            tracing::debug!("no target version set, skipping self-update");
            return;
        }
    };

    if target == CURRENT_VERSION {
        tracing::info!("already at target version {CURRENT_VERSION}");
        return;
    }

    // Prevent downgrades
    if version_cmp(&target) < 0 {
        tracing::warn!(
            "target version {target} is older than current {CURRENT_VERSION}, refusing downgrade"
        );
        return;
    }

    tracing::info!("updating: {CURRENT_VERSION} -> {target}");
    sentry_ext::breadcrumb("self-update", "updating", &[
        ("from", CURRENT_VERSION),
        ("to", &target),
    ]);

    if let Err(e) = apply_version(&target) {
        tracing::warn!("update to {target} failed: {e}");
        sentry_ext::capture_error(
            &format!("self-update to {target} failed: {e}"),
            &[("from", CURRENT_VERSION), ("to", &target)],
        );
    }
}

/// Compare a version string against CURRENT_VERSION.
/// Returns -1 if ver < current, 0 if equal, 1 if ver > current.
fn version_cmp(ver: &str) -> i32 {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.').filter_map(|p| p.parse().ok()).collect()
    };
    let a = parse(ver);
    let b = parse(CURRENT_VERSION);
    a.cmp(&b) as i32
}

/// Download `{UPDATE_BASE}/{ENVIRONMENT}/{version}/mac-mgmt.tar.gz` and replace the binary.
fn apply_version(version: &str) -> Result<()> {
    let base = update_base();
    let url = format!("{base}/{ENVIRONMENT}/{version}/mac-mgmt.tar.gz");

    let mut tmp_archive = tempfile::Builder::new()
        .suffix(".tar.gz")
        .tempfile()
        .context("failed to create temp file")?;

    tracing::info!("downloading {url}");
    let mut download = self_update::Download::from_url(&url);
    download.show_progress(false);
    let mut body = Vec::new();
    download.download_to(&mut body)?;
    tmp_archive.write_all(&body)?;
    tmp_archive.flush()?;

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

    if let Err(e) = self_replace::self_replace(&new_bin) {
        tracing::warn!("self_replace failed ({e}), falling back to rename-over");
        let current_exe = std::env::current_exe().context("failed to get current exe path")?;
        let parent = current_exe.parent().context("current exe has no parent dir")?;
        let tmp_target = parent.join(".mac-mgmt.update");
        std::fs::copy(&new_bin, &tmp_target)
            .context("failed to copy new binary to temp location")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp_target, std::fs::Permissions::from_mode(0o755))
                .context("failed to set permissions on new binary")?;
        }

        std::fs::rename(&tmp_target, &current_exe)
            .context("failed to rename new binary over current")?;
    }

    tracing::info!("binary updated to {version}");
    sentry_ext::breadcrumb("self-update", "binary updated", &[
        ("from", CURRENT_VERSION),
        ("to", version),
    ]);
    Ok(())
}

/// CLI: force-apply from the default update channel (no server needed).
pub fn apply(force: bool) -> Result<()> {
    let base = update_base();
    let url = format!("{base}/{ENVIRONMENT}/mac-mgmt.version");
    let mut body = Vec::new();
    let mut download = self_update::Download::from_url(&url);
    download.show_progress(false);
    download.download_to(&mut body)?;
    let remote_version = String::from_utf8(body)
        .context("invalid UTF-8 in version file")?
        .trim()
        .to_string();

    if !force && remote_version == CURRENT_VERSION {
        println!("already up to date ({CURRENT_VERSION})");
        return Ok(());
    }

    if !force && version_cmp(&remote_version) < 0 {
        anyhow::bail!("remote version {remote_version} < current {CURRENT_VERSION}, use --force to downgrade");
    }

    println!("updating: {CURRENT_VERSION} -> {remote_version}");
    apply_version(&remote_version)
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
        assert_eq!(UPDATE_BASE, "https://update.plan.ai");
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

    #[test]
    fn version_cmp_works() {
        // These compare against CURRENT_VERSION (0.1.5)
        assert!(version_cmp("0.1.5") == 0);
        assert!(version_cmp("0.1.6") > 0);
        assert!(version_cmp("0.1.4") < 0);
        assert!(version_cmp("0.2.0") > 0);
        assert!(version_cmp("1.0.0") > 0);
    }
}
