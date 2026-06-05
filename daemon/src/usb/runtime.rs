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

use std::path::PathBuf;

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
        "macos" => Err(RuntimeError::MacOsFollowOn),
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
    /// macOS support is a follow-on phase.
    MacOsFollowOn,
    UnsupportedOs(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::NeedsPortable => write!(f, "no host nix; portable runtime required"),
            RuntimeError::MacOsFollowOn => {
                write!(f, "macOS sovereign-AI runtime is a follow-on phase")
            }
            RuntimeError::UnsupportedOs(os) => write!(f, "unsupported OS for usb runtime: {os}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

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
    fn macos_without_host_nix_is_follow_on() {
        assert_eq!(select(None, "macos"), Err(RuntimeError::MacOsFollowOn));
    }

    #[test]
    fn unknown_os_is_unsupported() {
        assert_eq!(
            select(None, "plan9"),
            Err(RuntimeError::UnsupportedOs("plan9".to_string()))
        );
    }
}
