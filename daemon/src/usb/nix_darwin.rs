//! Native nix bootstrap for macOS (USB build).
//!
//! macOS can't use nix-portable — its proot/bwrap store virtualization needs
//! Linux namespaces/ptrace. So we run **native** nix against the case-sensitive
//! APFS store image mounted at `/nix` ([`super::store_image::mount_macos`]).
//! This module downloads the official nix install tarball for the host darwin
//! arch and installs its closure into the mounted `/nix`, then registers it in
//! the nix database — mirroring what the official installer does, but the store
//! data lives on the stick. Keeps the standard `/nix/store` prefix → full
//! cache.nixos.org + xzar reuse, no rebuild.
//!
//! Most of this compiles on every platform (it shells out to `tar`/`cp`/`nix`);
//! it is only reached at runtime on macOS via [`super::runtime::prepare`].

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pinned nix version for the macOS bootstrap. Update [`PINNED_SHA256`] in
/// lockstep when bumping.
pub const PINNED_NIX_VERSION: &str = "2.24.10";

/// Per-`system` pinned lowercase-hex SHA-256 of the install tarball. Empty means
/// "not pinned" — the download proceeds but is logged as unverified. Fill these
/// from `releases.nixos.org/nix/nix-<ver>/` to harden the bootstrap.
pub fn pinned_sha256(system: &str) -> &'static str {
    match system {
        "x86_64-darwin" => "",
        "aarch64-darwin" => "",
        _ => "",
    }
}

/// nix release `system` token for the host darwin arch, or `None` on
/// unsupported arches.
pub fn darwin_system() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Some("x86_64-darwin"),
        "aarch64" => Some("aarch64-darwin"),
        _ => None,
    }
}

/// Download URL of the nix install tarball for `system` at `version`.
pub fn tarball_url(version: &str, system: &str) -> String {
    format!("https://releases.nixos.org/nix/nix-{version}/nix-{version}-{system}.tar.xz")
}

/// Cached tarball path under `home`.
fn tarball_path(home: &Path, system: &str) -> PathBuf {
    home.join(".cache/usb")
        .join(format!("nix-{PINNED_NIX_VERSION}-{system}.tar.xz"))
}

/// Ensure a native `nix` is available against the mounted `/nix`, installing it
/// from the official tarball when absent. Returns the `nix` binary path.
/// Blocking — called before the async runtime, like the Linux path.
pub fn ensure_blocking(home: &Path, allow_network: bool) -> Result<PathBuf> {
    if let Some(nix) = find_installed_nix() {
        tracing::info!(nix = %nix.display(), "native nix already present in /nix");
        return Ok(nix);
    }
    let system = darwin_system().context("unsupported macOS arch for nix bootstrap")?;

    let tarball = tarball_path(home, system);
    if !tarball.exists() {
        if !allow_network {
            anyhow::bail!(
                "native nix not installed in /nix and running offline — run \
                 `usb prefetch` online first"
            );
        }
        download_tarball(&tarball, system)?;
    }
    install_from_tarball(&tarball)?;
    find_installed_nix().context("nix binary not found after install")
}

/// Download the install tarball to `dest`, verifying its SHA-256 when pinned.
/// Blocking reqwest (no lasting threads before the runtime is built).
pub fn download_tarball(dest: &Path, system: &str) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let url = tarball_url(PINNED_NIX_VERSION, system);
    tracing::info!("downloading native nix from {url}");
    let bytes = reqwest::blocking::Client::new()
        .get(&url)
        .send()
        .with_context(|| format!("failed to GET {url}"))?
        .error_for_status()
        .with_context(|| format!("bad status from {url}"))?
        .bytes()
        .context("failed to read nix tarball body")?;

    let pin = pinned_sha256(system);
    if pin.is_empty() {
        tracing::warn!(
            "nix tarball checksum not pinned for {system}; proceeding unverified \
             (set nix_darwin::pinned_sha256 to harden)"
        );
    } else {
        let got = sha256_hex(&bytes);
        if got != pin {
            anyhow::bail!("nix tarball checksum mismatch: expected {pin}, got {got}");
        }
    }

    let tmp = dest.with_extension("xz.part");
    std::fs::write(&tmp, &bytes).context("failed to write nix tarball")?;
    std::fs::rename(&tmp, dest).context("failed to finalize nix tarball")?;
    tracing::info!("nix tarball ready at {}", dest.display());
    Ok(())
}

/// Unpack the tarball and install its store closure into the mounted `/nix`,
/// then register it with `nix-store --load-db` (mirrors the official installer).
fn install_from_tarball(tarball: &Path) -> Result<()> {
    let work = tarball
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("nix-extract");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).context("failed to create extract dir")?;

    // macOS bsdtar auto-detects xz.
    let st = Command::new("tar")
        .arg("xf")
        .arg(tarball)
        .arg("-C")
        .arg(&work)
        .status()
        .context("failed to extract nix tarball (tar)")?;
    if !st.success() {
        anyhow::bail!("tar extract of {} failed", tarball.display());
    }

    // The tarball has a single top dir: nix-<ver>-<system>/.
    let root = std::fs::read_dir(&work)?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir())
        .context("unexpected nix tarball layout (no top dir)")?;

    // Required store dirs.
    std::fs::create_dir_all("/nix/store").ok();
    std::fs::create_dir_all("/nix/var/nix").ok();

    // Copy the closure into the mounted store, preserving perms/timestamps.
    let st = Command::new("cp")
        .arg("-a")
        .arg(format!("{}/.", root.join("store").display()))
        .arg("/nix/store/")
        .status()
        .context("failed to copy nix store closure")?;
    if !st.success() {
        anyhow::bail!("copying nix closure into /nix/store failed");
    }

    let nix = find_installed_nix().context("nix binary missing after copy")?;
    let bin_dir = nix.parent().context("nix has no parent dir")?;

    // Register the copied closure in the nix database.
    let reginfo = root.join(".reginfo");
    let regfile =
        std::fs::File::open(&reginfo).with_context(|| format!("missing {}", reginfo.display()))?;
    let st = Command::new(bin_dir.join("nix-store"))
        .arg("--load-db")
        .env("NIX_STORE_DIR", "/nix/store")
        .env("NIX_STATE_DIR", "/nix/var/nix")
        .stdin(regfile)
        .status()
        .context("failed to run nix-store --load-db")?;
    if !st.success() {
        anyhow::bail!("nix-store --load-db failed");
    }
    tracing::info!("native nix installed + registered at {}", nix.display());
    Ok(())
}

/// Find an installed `nix` binary in the mounted store (or the default profile).
fn find_installed_nix() -> Option<PathBuf> {
    let profile = PathBuf::from("/nix/var/nix/profiles/default/bin/nix");
    if profile.exists() {
        return Some(profile);
    }
    let store = Path::new("/nix/store");
    for e in std::fs::read_dir(store).ok()?.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.contains("-nix-") && !name.ends_with(".drv") {
            let nix = e.path().join("bin/nix");
            if nix.exists() {
                return Some(nix);
            }
        }
    }
    None
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tarball_url_is_official() {
        assert_eq!(
            tarball_url("2.24.10", "aarch64-darwin"),
            "https://releases.nixos.org/nix/nix-2.24.10/nix-2.24.10-aarch64-darwin.tar.xz"
        );
    }

    #[test]
    fn darwin_system_maps_arch() {
        // The host arch under test is whatever CI runs on; just assert the
        // mapping shape is one of the known systems or None.
        let s = darwin_system();
        assert!(matches!(
            s,
            Some("x86_64-darwin") | Some("aarch64-darwin") | None
        ));
    }
}
