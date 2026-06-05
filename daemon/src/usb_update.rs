//! Self-contained binary updater for the USB build.
//!
//! Replaces the server-driven update path (`/api/update` + nix-store realise)
//! with a direct download of a GitLab release tarball, checksum-verified, that
//! self-replaces the running binary and re-execs. Dependency closures are
//! unaffected — they keep coming from xzar / the on-stick `.nar` cache.

use anyhow::Result;

/// GitLab host serving releases for the USB build.
pub const GITLAB_HOST: &str = "git.plan.ai";
/// URL-encoded GitLab project path.
pub const GITLAB_PROJECT: &str = "plan-ai%2Fmac-mgmt";

/// A release asset for one target, as parsed from the GitLab releases API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
}

/// Compare two semver-ish version strings; `true` if `candidate` is newer than
/// `current`. Falls back to string inequality for non-numeric components.
pub fn is_newer(current: &str, candidate: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.trim_start_matches('v')
            .split(|c: char| c == '.' || c == '-')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    }
    let (c, n) = (parts(current), parts(candidate));
    for i in 0..c.len().max(n.len()) {
        let a = c.get(i).copied().unwrap_or(0);
        let b = n.get(i).copied().unwrap_or(0);
        if b != a {
            return b > a;
        }
    }
    false
}

/// Check for and apply a newer release. No-op when offline (caller gates this).
/// Returns `Ok(true)` if an update was applied (and a re-exec should follow).
pub async fn check_and_apply() -> Result<bool> {
    anyhow::bail!("usb binary updater not yet implemented")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions_detected() {
        assert!(is_newer("0.1.5", "0.1.6"));
        assert!(is_newer("0.1.5", "0.2.0"));
        assert!(is_newer("v0.1.5", "v0.1.6"));
        assert!(!is_newer("0.1.6", "0.1.6"));
        assert!(!is_newer("0.1.6", "0.1.5"));
        assert!(!is_newer("0.2.0", "0.1.9"));
    }
}
