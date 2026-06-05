//! Self-contained binary updater for the USB build.
//!
//! Replaces the server-driven update path (`/api/update` + nix-store realise)
//! with a direct download of a GitLab release tarball, checksum-verified, that
//! self-replaces the running binary and re-execs. Dependency closures are
//! unaffected — they keep coming from xzar / the on-stick `.nar` cache.

use anyhow::{Context, Result};

/// GitLab host serving releases for the USB build.
pub const GITLAB_HOST: &str = "git.plan.ai";
/// URL-encoded GitLab project path.
pub const GITLAB_PROJECT: &str = "plan-ai%2Fmac-mgmt";

/// Release-asset file name for a target arch (matches the CI release job).
pub fn asset_name(arch: &str) -> String {
    format!("mac-mgmt-usb-{arch}.tar.gz")
}

/// GitLab releases API URL (newest first).
pub fn releases_url() -> String {
    format!("https://{GITLAB_HOST}/api/v4/projects/{GITLAB_PROJECT}/releases?per_page=20")
}

/// A release asset for one target, parsed from the GitLab releases API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub version: String,
    pub url: String,
    /// URL of a sibling `<asset>.sha256` link, if the release published one.
    pub sha256_url: Option<String>,
}

/// Compare two semver-ish version strings; `true` if `candidate` is newer than
/// `current`. Falls back to 0 for non-numeric components.
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

/// Pick the newest release (strictly newer than `current`) that publishes an
/// asset for `arch`, from a GitLab releases API JSON array.
pub fn parse_best_release(
    releases: &serde_json::Value,
    current: &str,
    arch: &str,
) -> Option<ReleaseAsset> {
    let want = asset_name(arch);
    let sha_want = format!("{want}.sha256");
    let mut best: Option<ReleaseAsset> = None;
    for rel in releases.as_array()? {
        let tag = rel.get("tag_name").and_then(|t| t.as_str())?;
        if !is_newer(current, tag) {
            continue;
        }
        let links = rel
            .get("assets")
            .and_then(|a| a.get("links"))
            .and_then(|l| l.as_array());
        let Some(links) = links else { continue };

        let mut url = None;
        let mut sha_url = None;
        for link in links {
            let name = link.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let durl = link
                .get("direct_asset_url")
                .or_else(|| link.get("url"))
                .and_then(|u| u.as_str());
            if name == want {
                url = durl.map(str::to_string);
            } else if name == sha_want {
                sha_url = durl.map(str::to_string);
            }
        }
        let Some(url) = url else { continue };
        let candidate = ReleaseAsset {
            version: tag.trim_start_matches('v').to_string(),
            url,
            sha256_url: sha_url,
        };
        match &best {
            Some(b) if !is_newer(&b.version, &candidate.version) => {}
            _ => best = Some(candidate),
        }
    }
    best
}

/// Check for and apply a newer release. Caller gates this off when offline.
/// Returns `Ok(true)` if an update was applied — the caller should then re-exec
/// (see [`reexec_self`]).
pub async fn check_and_apply() -> Result<bool> {
    let current = env!("CARGO_PKG_VERSION");
    let arch = crate::usb::nix_portable::host_arch();
    let url = releases_url();
    let client = reqwest::Client::new();

    let releases: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("failed to GET {url}"))?
        .error_for_status()
        .with_context(|| format!("bad status from {url}"))?
        .json()
        .await
        .context("failed to parse releases JSON")?;

    let Some(asset) = parse_best_release(&releases, current, &arch) else {
        tracing::info!("usb updater: already on the newest release ({current})");
        return Ok(false);
    };
    tracing::info!(
        "usb updater: {} -> {} ({})",
        current,
        asset.version,
        asset.url
    );

    apply(&client, &asset).await?;
    Ok(true)
}

/// Download, verify, extract, and self-replace from `asset`.
async fn apply(client: &reqwest::Client, asset: &ReleaseAsset) -> Result<()> {
    let bytes = client
        .get(&asset.url)
        .send()
        .await
        .with_context(|| format!("failed to download {}", asset.url))?
        .error_for_status()?
        .bytes()
        .await
        .context("failed to read release tarball")?;

    if let Some(sha_url) = &asset.sha256_url {
        let expected = client
            .get(sha_url)
            .send()
            .await
            .with_context(|| format!("failed to download {sha_url}"))?
            .error_for_status()?
            .text()
            .await
            .context("failed to read sha256")?;
        verify_sha256(&bytes, expected.split_whitespace().next().unwrap_or(""))
            .context("release tarball checksum mismatch")?;
    } else {
        tracing::warn!("usb updater: release published no .sha256 — skipping checksum");
    }

    let new_bin = extract_binary(&bytes).context("failed to extract bin/mac-mgmt from release")?;
    self_replace::self_replace(&new_bin)
        .context("failed to self-replace the running binary")?;
    let _ = std::fs::remove_file(&new_bin);
    tracing::info!("usb updater: binary replaced — re-exec to apply");
    Ok(())
}

/// Verify `bytes` hash to the expected lowercase-hex SHA-256.
fn verify_sha256(bytes: &[u8], expected: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let got = hex::encode(hasher.finalize());
    if !got.eq_ignore_ascii_case(expected) {
        anyhow::bail!("sha256 mismatch: expected {expected}, got {got}");
    }
    Ok(())
}

/// Extract `bin/mac-mgmt` from a `.tar.gz` into a temp file (mode 0755) and
/// return its path.
fn extract_binary(tar_gz: &[u8]) -> Result<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let gz = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(gz);
    let tmp = std::env::temp_dir().join(format!("mac-mgmt-update-{}", std::process::id()));
    for entry in archive.entries().context("failed to read tar entries")? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if path.file_name().and_then(|n| n.to_str()) == Some("mac-mgmt")
            && path.to_string_lossy().contains("bin/")
        {
            entry
                .unpack(&tmp)
                .with_context(|| format!("failed to unpack to {}", tmp.display()))?;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
            return Ok(tmp);
        }
    }
    anyhow::bail!("no bin/mac-mgmt entry in release tarball")
}

/// Re-exec the (now self-replaced) running binary with the original argv.
pub fn reexec_self() -> Result<()> {
    use std::os::unix::process::CommandExt;
    let exe = std::env::current_exe().context("failed to resolve current exe")?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let err = std::process::Command::new(exe).args(args).exec();
    Err(err).context("failed to re-exec updated binary")
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

    #[test]
    fn asset_name_matches_ci_convention() {
        assert_eq!(asset_name("x86_64-linux"), "mac-mgmt-usb-x86_64-linux.tar.gz");
    }

    #[test]
    fn parse_best_release_picks_newest_with_asset() {
        let json = serde_json::json!([
            {
                "tag_name": "v0.1.6",
                "assets": { "links": [
                    { "name": "mac-mgmt-usb-x86_64-linux.tar.gz", "direct_asset_url": "https://x/0.1.6/bin.tar.gz" },
                    { "name": "mac-mgmt-usb-x86_64-linux.tar.gz.sha256", "url": "https://x/0.1.6/bin.sha256" }
                ]}
            },
            {
                "tag_name": "v0.1.7",
                "assets": { "links": [
                    { "name": "mac-mgmt-usb-x86_64-linux.tar.gz", "direct_asset_url": "https://x/0.1.7/bin.tar.gz" }
                ]}
            },
            {
                "tag_name": "v0.1.8",
                "assets": { "links": [
                    { "name": "mac-mgmt-usb-aarch64-linux.tar.gz", "url": "https://x/0.1.8/arm.tar.gz" }
                ]}
            }
        ]);
        let got = parse_best_release(&json, "0.1.5", "x86_64-linux").unwrap();
        // 0.1.7 is the newest with an x86_64 asset (0.1.8 only has aarch64).
        assert_eq!(got.version, "0.1.7");
        assert_eq!(got.url, "https://x/0.1.7/bin.tar.gz");
        assert!(got.sha256_url.is_none());
    }

    #[test]
    fn parse_best_release_none_when_not_newer() {
        let json = serde_json::json!([
            { "tag_name": "v0.1.4", "assets": { "links": [
                { "name": "mac-mgmt-usb-x86_64-linux.tar.gz", "url": "u" }
            ]}}
        ]);
        assert!(parse_best_release(&json, "0.1.5", "x86_64-linux").is_none());
    }

    #[test]
    fn parse_best_release_prefers_sha_link() {
        let json = serde_json::json!([
            { "tag_name": "v1.0.0", "assets": { "links": [
                { "name": "mac-mgmt-usb-x86_64-linux.tar.gz", "url": "u" },
                { "name": "mac-mgmt-usb-x86_64-linux.tar.gz.sha256", "url": "s" }
            ]}}
        ]);
        let got = parse_best_release(&json, "0.1.0", "x86_64-linux").unwrap();
        assert_eq!(got.sha256_url.as_deref(), Some("s"));
    }
}
