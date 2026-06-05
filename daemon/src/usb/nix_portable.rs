//! Bootstrap [nix-portable](https://github.com/DavHau/nix-portable): a static,
//! permissionless nix. On the USB build we download the release asset matching
//! the host architecture into the stick's `HOME`, verify it against a pinned
//! SHA-256, and expose a path to invoke `nix` through it. Linux only — macOS
//! uses native nix (see plan).

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The nix-portable release tag the USB build pins to. Bumped deliberately;
/// the per-asset SHA-256 digests below must be updated in lockstep.
pub const PINNED_TAG: &str = "v012";

/// A nix-portable release asset for a given host architecture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    /// Release asset file name, e.g. `nix-portable-x86_64`.
    pub file_name: &'static str,
    /// Rust target triple this asset is for (used for diagnostics).
    pub rust_target: &'static str,
    /// Pinned lowercase hex SHA-256 of the asset at [`PINNED_TAG`].
    pub sha256: &'static str,
}

/// Resolve the nix-portable asset for a host arch string (as produced by
/// [`host_arch`]). Returns `None` for unsupported arches (e.g. macOS, where
/// nix-portable is not available upstream).
pub fn asset_for_arch(arch: &str) -> Option<Asset> {
    match arch {
        "x86_64-linux" => Some(Asset {
            file_name: "nix-portable-x86_64",
            rust_target: "x86_64-unknown-linux-musl",
            sha256: "b409c55904c909ac3aeda3fb1253319f86a89ddd1ba31a5dec33d4a06414c72a",
        }),
        "aarch64-linux" => Some(Asset {
            file_name: "nix-portable-aarch64",
            rust_target: "aarch64-unknown-linux-musl",
            sha256: "af41d8defdb9fa17ee361220ee05a0c758d3e6231384a3f969a314f9133744ea",
        }),
        _ => None,
    }
}

/// All supported nix-portable assets (used by `prefetch --all-arches`).
pub fn all_assets() -> Vec<(&'static str, Asset)> {
    ["x86_64-linux", "aarch64-linux"]
        .into_iter()
        .filter_map(|a| asset_for_arch(a).map(|asset| (a, asset)))
        .collect()
}

/// Host architecture token (`<arch>-<os>`), e.g. `x86_64-linux`.
pub fn host_arch() -> String {
    let arch = std::env::consts::ARCH; // "x86_64", "aarch64", ...
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    format!("{arch}-{os}")
}

/// Download URL for a nix-portable asset at a given release tag.
pub fn asset_url(tag: &str, asset: &Asset) -> String {
    format!(
        "https://github.com/DavHau/nix-portable/releases/download/{tag}/{}",
        asset.file_name
    )
}

/// Path where the nix-portable binary is cached under `home`.
pub fn cached_path(home: &Path) -> PathBuf {
    home.join(".local/bin/nix-portable")
}

/// Path where a per-arch nix-portable binary is cached (used by prefetch so one
/// stick can carry binaries for multiple arches; the host arch is symlinked /
/// copied to [`cached_path`] at run time).
pub fn cached_path_for_arch(home: &Path, arch: &str) -> PathBuf {
    home.join(".local/bin").join(format!("nix-portable-{arch}"))
}

/// Ensure nix-portable is present under `home`, downloading + verifying it when
/// missing. `allow_network` gates whether a download may be attempted; when
/// `false` and the binary is absent, this fails loud.
///
/// Returns the path to the executable nix-portable binary.
pub async fn ensure(home: &Path, allow_network: bool) -> Result<PathBuf> {
    let dest = cached_path(home);
    if dest.exists() {
        return Ok(dest);
    }
    let arch = host_arch();
    let asset = asset_for_arch(&arch)
        .with_context(|| format!("no nix-portable asset for host arch {arch}"))?;

    // A per-arch cached copy (e.g. from `prefetch --all-arches`) counts as
    // present even when the canonical path is missing.
    let per_arch = cached_path_for_arch(home, &arch);
    if per_arch.exists() {
        copy_executable(&per_arch, &dest)?;
        return Ok(dest);
    }

    if !allow_network {
        anyhow::bail!(
            "nix-portable missing at {} and running offline — run `usb prefetch` online first",
            dest.display()
        );
    }
    download_and_verify(&asset, &dest)
        .await
        .with_context(|| format!("failed to bootstrap nix-portable for {arch}"))?;
    Ok(dest)
}

/// Download `asset` to `dest`, verify its SHA-256, and mark it executable.
/// Written atomically via a temp file + rename so a partial download is never
/// left at `dest`.
pub async fn download_and_verify(asset: &Asset, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let url = asset_url(PINNED_TAG, asset);
    tracing::info!("downloading nix-portable from {url}");
    let bytes = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .with_context(|| format!("failed to GET {url}"))?
        .error_for_status()
        .with_context(|| format!("bad status from {url}"))?
        .bytes()
        .await
        .context("failed to read nix-portable body")?;

    verify_sha256(&bytes, asset.sha256)
        .with_context(|| format!("nix-portable {} checksum mismatch", asset.file_name))?;

    write_executable_atomic(dest, &bytes)?;
    tracing::info!("nix-portable ready at {}", dest.display());
    Ok(())
}

/// Verify `bytes` hash to the expected lowercase-hex SHA-256.
fn verify_sha256(bytes: &[u8], expected: &str) -> Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let got = hex::encode(hasher.finalize());
    if !got.eq_ignore_ascii_case(expected) {
        anyhow::bail!("sha256 mismatch: expected {expected}, got {got}");
    }
    Ok(())
}

/// Write `bytes` to `dest` atomically (temp + rename) with mode 0755.
fn write_executable_atomic(dest: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = dest.with_extension("download.tmp");
    std::fs::write(&tmp, bytes).with_context(|| format!("failed to write {}", tmp.display()))?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("failed to chmod {}", tmp.display()))?;
    std::fs::rename(&tmp, dest)
        .with_context(|| format!("failed to install {}", dest.display()))?;
    Ok(())
}

/// Copy an existing nix-portable binary to `dest`, preserving the executable bit.
fn copy_executable(src: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::copy(src, dest)
        .with_context(|| format!("failed to copy {} -> {}", src.display(), dest.display()))?;
    std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("failed to chmod {}", dest.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_lookup_for_supported_arches() {
        assert_eq!(
            asset_for_arch("x86_64-linux").unwrap().file_name,
            "nix-portable-x86_64"
        );
        assert_eq!(
            asset_for_arch("aarch64-linux").unwrap().file_name,
            "nix-portable-aarch64"
        );
        assert!(asset_for_arch("aarch64-darwin").is_none());
        assert!(asset_for_arch("x86_64-darwin").is_none());
        assert!(asset_for_arch("riscv64-linux").is_none());
    }

    #[test]
    fn url_is_well_formed() {
        let asset = asset_for_arch("x86_64-linux").unwrap();
        assert_eq!(
            asset_url(PINNED_TAG, &asset),
            "https://github.com/DavHau/nix-portable/releases/download/v012/nix-portable-x86_64"
        );
    }

    #[test]
    fn sha256_verify_accepts_match_rejects_mismatch() {
        // sha256("") is the well-known empty digest.
        let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert!(verify_sha256(b"", empty).is_ok());
        assert!(verify_sha256(b"x", empty).is_err());
    }

    #[test]
    fn all_assets_covers_both_linux_arches() {
        let arches: Vec<&str> = all_assets().into_iter().map(|(a, _)| a).collect();
        assert!(arches.contains(&"x86_64-linux"));
        assert!(arches.contains(&"aarch64-linux"));
    }
}
