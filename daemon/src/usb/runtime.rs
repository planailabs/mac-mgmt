//! Nix runtime selection for the USB build.
//!
//! At startup we pick how nix is provided, preferring an existing host nix:
//!
//!   - [`Runtime::HostNix`]: a usable `nix` is already on `PATH`. We import the
//!     stick's `.nar` closures into the existing `/nix/store` and run directly.
//!   - [`Runtime::Portable`] (Linux, no host nix): bootstrap nix-portable and a
//!     private ext4 store image mounted at `/nix` inside a mount namespace.
//!   - [`Runtime::MacNative`] (macOS, no host nix): native nix against a
//!     case-sensitive APFS image at `/nix` (follow-on phase).
//!
//! All three keep the standard `/nix/store` prefix, so the `.nar` cache and
//! xzar substitute identically regardless of runtime.

use super::{nix_darwin, nix_portable, store_image};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Runtime {
    /// Use the host's existing nix at this path.
    HostNix { nix_bin: PathBuf },
    /// Linux: nix-portable + an ext4 store image mounted at `/nix`.
    Portable { nix_portable: PathBuf },
    /// macOS: native nix against an APFS store image (follow-on).
    MacNative { nix_bin: PathBuf },
}

/// Probe whether a usable host `nix` is available. A runtime is "usable" if the
/// `nix` binary is on `PATH` and a trivial store operation succeeds.
pub fn detect_host_nix() -> Option<PathBuf> {
    let nix = which::which("nix").ok()?;
    // `nix store ping` is a cheap liveness probe against the local store.
    let ok = std::process::Command::new(&nix)
        .args(["store", "ping"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok.then_some(nix)
}

/// Decide which runtime to use given whether a host nix was detected and the
/// target OS. Pure, so it is unit-testable.
pub fn select(host_nix: Option<PathBuf>, os: &str) -> Result<Runtime, RuntimeError> {
    if let Some(nix_bin) = host_nix {
        return Ok(Runtime::HostNix { nix_bin });
    }
    match os {
        "macos" => Err(RuntimeError::NeedsMacImage),
        "linux" => Err(RuntimeError::NeedsPortable),
        other => Err(RuntimeError::UnsupportedOs(other.to_string())),
    }
}

/// Outcome of [`select`] when no host nix is available — the caller bootstraps
/// the appropriate portable runtime, or errors on unsupported platforms.
#[derive(Debug, PartialEq, Eq)]
pub enum RuntimeError {
    /// Linux without host nix → bootstrap nix-portable + ext4 image.
    NeedsPortable,
    /// macOS without host nix → bootstrap native nix + a case-sensitive APFS
    /// store image mounted at `/nix`.
    NeedsMacImage,
    UnsupportedOs(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::NeedsPortable => write!(f, "no host nix; portable runtime required"),
            RuntimeError::NeedsMacImage => {
                write!(
                    f,
                    "no host nix; macOS APFS store image + native nix required"
                )
            }
            RuntimeError::UnsupportedOs(os) => write!(f, "unsupported OS for usb runtime: {os}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

/// Prepare the nix runtime for the stack. **Sync and must run single-threaded**
/// (before the tokio runtime) because the Portable path enters a mount
/// namespace, which only moves the calling thread.
///
/// On success the process is ready to run nix (host nix on PATH, or the static
/// nix from nix-portable against an image-backed `/nix`), and the relevant
/// environment (`PATH`, `MAC_MGMT_NIXPKGS_TARBALL` consumers) is configured.
///
/// `offline` forbids any network (no nix-portable download); `home` is the
/// stick directory.
pub fn prepare(home: &Path, offline: bool) -> Result<Runtime> {
    if let Some(nix_bin) = detect_host_nix() {
        tracing::info!(nix = %nix_bin.display(), "using existing host nix");
        return Ok(Runtime::HostNix { nix_bin });
    }
    match std::env::consts::OS {
        "linux" => prepare_portable(home, offline),
        "macos" => prepare_macos(home, offline),
        other => Err(RuntimeError::UnsupportedOs(other.to_string()).into()),
    }
}

/// Linux, no host nix: bootstrap nix-portable + an ext4 store image mounted at
/// `/nix` in a private namespace, then expose nix-portable's static `nix` on
/// PATH so it drives the real `/nix` directly.
fn prepare_portable(home: &Path, offline: bool) -> Result<Runtime> {
    let np = nix_portable::ensure_blocking(home, !offline)
        .context("failed to bootstrap nix-portable")?;
    store_image::ensure_image(home, store_image::DEFAULT_IMAGE_SIZE)
        .context("failed to create store image")?;
    store_image::ensure_nar_cache(home).context("failed to init on-stick nar cache")?;
    // Unpack the static nix *before* entering the namespace (it writes under
    // HOME, which we want visible to the host's filesystem, not the image).
    let static_nix =
        nix_portable::extract_static_nix(home, &np).context("failed to extract static nix")?;

    store_image::enter_namespace_and_mount(home).context("failed to mount store image at /nix")?;

    // Put the static nix's directory first on PATH so `nix_command("nix")`
    // (daemon/src/nix.rs) resolves to it, operating on the real mounted /nix.
    if let Some(bin_dir) = static_nix.parent() {
        prepend_path(bin_dir);
    }

    Ok(Runtime::Portable { nix_portable: np })
}

/// macOS, no host nix: create + attach a case-sensitive APFS store image at
/// `/nix` (the data lives on the stick, like the official installer but
/// relocated), then bootstrap a native `nix` into it. Keeps the standard
/// `/nix/store` prefix → full cache.nixos.org + xzar reuse, no rebuild.
///
/// Requires root the first time (to create the `/nix` synthetic mountpoint via
/// `/etc/synthetic.conf`); `hdiutil` + `apfs.util` are stock macOS tools.
fn prepare_macos(home: &Path, offline: bool) -> Result<Runtime> {
    store_image::ensure_image_macos(home, store_image::DEFAULT_IMAGE_SIZE)
        .context("failed to create APFS store image")?;
    store_image::ensure_nar_cache(home).context("failed to init on-stick nar cache")?;
    store_image::mount_macos(home).context("failed to mount APFS store image at /nix")?;

    // Bring up a native nix that operates on the mounted /nix store.
    let nix_bin = nix_darwin::ensure_blocking(home, !offline)
        .context("failed to bootstrap native nix for macOS")?;
    if let Some(bin_dir) = nix_bin.parent() {
        prepend_path(bin_dir);
    }
    Ok(Runtime::MacNative { nix_bin })
}

/// Prepend `dir` to the process `PATH`.
fn prepend_path(dir: &Path) {
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<PathBuf> = vec![dir.to_path_buf()];
    paths.extend(std::env::split_paths(&existing));
    if let Ok(joined) = std::env::join_paths(paths) {
        // SAFETY: called single-threaded during pre-runtime setup.
        unsafe {
            std::env::set_var("PATH", joined);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_nix_present_wins() {
        let r = select(Some(PathBuf::from("/usr/bin/nix")), "linux").unwrap();
        assert_eq!(
            r,
            Runtime::HostNix {
                nix_bin: PathBuf::from("/usr/bin/nix")
            }
        );
    }

    #[test]
    fn linux_without_host_nix_needs_portable() {
        assert_eq!(select(None, "linux"), Err(RuntimeError::NeedsPortable));
    }

    #[test]
    fn macos_without_host_nix_needs_image() {
        assert_eq!(select(None, "macos"), Err(RuntimeError::NeedsMacImage));
    }

    #[test]
    fn unknown_os_is_unsupported() {
        assert_eq!(
            select(None, "plan9"),
            Err(RuntimeError::UnsupportedOs("plan9".to_string()))
        );
    }
}
