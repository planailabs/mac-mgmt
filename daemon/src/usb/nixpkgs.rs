//! Pinned nixpkgs tarball for the USB build.
//!
//! The sovereign-AI stack evaluates against a locally-downloaded nixpkgs
//! tarball pinned to a known revision (seeded from the workspace `flake.lock`),
//! with no dependency on the mac-mgmt server's archive endpoint. The cached
//! tarball's `file://` path is fed to nix via `MAC_MGMT_NIXPKGS_TARBALL`
//! (see `daemon/src/nix.rs`).

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Default pinned nixpkgs revision (kept in sync with the workspace
/// `flake.lock` `nixpkgs` input).
pub const DEFAULT_REV: &str = "d99b013d5d1931ad77fe3912ed218170dec5d9a4";

/// GitHub tarball URL for a nixpkgs revision.
pub fn tarball_url(rev: &str) -> String {
    format!("https://github.com/NixOS/nixpkgs/archive/{rev}.tar.gz")
}

/// Cached tarball path under `home` for a given revision.
pub fn cached_path(home: &Path, rev: &str) -> PathBuf {
    home.join(".cache/nixpkgs")
        .join(format!("nixpkgs-{rev}.tar.gz"))
}

/// Ensure the pinned nixpkgs tarball is cached under `home`, downloading it
/// when missing. `allow_network` gates the download; offline + missing fails
/// loud. Returns the cached tarball path. Uses blocking reqwest so it is safe
/// to call during single-threaded pre-runtime setup.
pub fn ensure_blocking(home: &Path, rev: &str, allow_network: bool) -> Result<PathBuf> {
    let dest = cached_path(home, rev);
    if dest.exists() {
        return Ok(dest);
    }
    if !allow_network {
        anyhow::bail!(
            "nixpkgs tarball for {rev} missing at {} and running offline — run `usb prefetch` online first",
            dest.display()
        );
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let url = tarball_url(rev);
    tracing::info!("downloading pinned nixpkgs from {url}");
    let bytes = reqwest::blocking::Client::new()
        .get(&url)
        .send()
        .with_context(|| format!("failed to GET {url}"))?
        .error_for_status()
        .with_context(|| format!("bad status from {url}"))?
        .bytes()
        .context("failed to read nixpkgs tarball body")?;

    let tmp = dest.with_extension("tmp");
    std::fs::write(&tmp, &bytes).with_context(|| format!("failed to write {}", tmp.display()))?;
    std::fs::rename(&tmp, &dest)
        .with_context(|| format!("failed to install {}", dest.display()))?;
    tracing::info!("nixpkgs tarball cached at {}", dest.display());
    Ok(dest)
}

/// `file://` flakeref base for a cached nixpkgs tarball, suitable for
/// `MAC_MGMT_NIXPKGS_TARBALL` / nix `<base>#pkg` evaluation.
pub fn flakeref_base(path: &Path) -> String {
    format!("file://{}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_uses_github_archive() {
        assert_eq!(
            tarball_url("abc123"),
            "https://github.com/NixOS/nixpkgs/archive/abc123.tar.gz"
        );
    }

    #[test]
    fn cached_path_is_under_home_cache() {
        let p = cached_path(Path::new("/stick/home"), "deadbeef");
        assert_eq!(
            p,
            Path::new("/stick/home/.cache/nixpkgs/nixpkgs-deadbeef.tar.gz")
        );
    }

    #[test]
    fn flakeref_base_is_file_url() {
        assert_eq!(
            flakeref_base(Path::new("/stick/home/np.tar.gz")),
            "file:///stick/home/np.tar.gz"
        );
    }
}
