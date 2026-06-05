//! Bootstrap [nix-portable](https://github.com/DavHau/nix-portable): a static,
//! permissionless nix. On the USB build we download the release asset matching
//! the host architecture into the stick's `HOME`, verify it, and expose a path
//! to invoke `nix` through it. Linux only — macOS uses native nix (see plan).

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// A nix-portable release asset for a given host architecture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    /// Release asset file name, e.g. `nix-portable-x86_64`.
    pub file_name: &'static str,
    /// Rust target triple this asset is for (used for diagnostics).
    pub rust_target: &'static str,
}

/// Resolve the nix-portable asset for a host arch string (as produced by
/// [`host_arch`]). Returns `None` for unsupported arches (e.g. macOS, where
/// nix-portable is not available upstream).
pub fn asset_for_arch(arch: &str) -> Option<Asset> {
    match arch {
        "x86_64-linux" => Some(Asset {
            file_name: "nix-portable-x86_64",
            rust_target: "x86_64-unknown-linux-musl",
        }),
        "aarch64-linux" => Some(Asset {
            file_name: "nix-portable-aarch64",
            rust_target: "aarch64-unknown-linux-musl",
        }),
        _ => None,
    }
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

/// The nix-portable release tag the USB build pins to. Bumped deliberately.
pub const PINNED_TAG: &str = "v012";

/// Path where the nix-portable binary is cached under `home`.
pub fn cached_path(home: &Path) -> PathBuf {
    home.join(".local/bin/nix-portable")
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
    if !allow_network {
        anyhow::bail!(
            "nix-portable missing at {} and running offline — run `usb prefetch` online first",
            dest.display()
        );
    }
    let arch = host_arch();
    let asset = asset_for_arch(&arch)
        .with_context(|| format!("no nix-portable asset for host arch {arch}"))?;
    download_and_verify(&asset, &dest)
        .await
        .with_context(|| format!("failed to bootstrap nix-portable for {arch}"))?;
    Ok(dest)
}

/// Download the asset to `dest`, set it executable. (Checksum verification is
/// added alongside the pinned per-asset digests.)
async fn download_and_verify(_asset: &Asset, _dest: &Path) -> Result<()> {
    anyhow::bail!("nix-portable download not yet implemented")
}
