use anyhow::{Context, Result};
use std::process::Command;
use std::sync::OnceLock;

use crate::sentry_ext;

const NIX_SOURCE: &str = "https://git.plan.ai/plan-ai/nixpkgs/-/jobs/artifacts/plan-ai/raw/nixpkgs.tar.xz?job=build";

/// Cached result of whether `nix profile upgrade --dry-run` is supported.
static DRY_RUN_SUPPORTED: OnceLock<bool> = OnceLock::new();

/// Check if a package is installed via `nix profile list --json`.
pub fn is_installed(pkg: &str) -> Result<bool> {
    let output = Command::new("nix")
        .args(["profile", "list", "--json"])
        .output()
        .context("failed to run nix profile list")?;

    if !output.status.success() {
        anyhow::bail!(
            "nix profile list failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("failed to parse nix profile list json")?;

    // The JSON has an "elements" object; each element has a "storePaths" array
    // containing store paths that include the package name.
    if let Some(elements) = json.get("elements").and_then(|e| e.as_object()) {
        for (_key, element) in elements {
            if let Some(paths) = element.get("storePaths").and_then(|p| p.as_array()) {
                for path in paths {
                    if let Some(s) = path.as_str() {
                        if s.contains(pkg) {
                            return Ok(true);
                        }
                    }
                }
            }
        }
    }

    Ok(false)
}

/// Detect if `nix profile upgrade --dry-run` is supported by running it with
/// a non-existent element. If the error is about the unknown flag, dry-run is
/// not supported. Any other error (e.g. element not found) means the flag was
/// accepted.
/// See: https://github.com/NixOS/nix/pull/15545
fn has_dry_run_support() -> bool {
    *DRY_RUN_SUPPORTED.get_or_init(|| {
        let output = Command::new("nix")
            .args(["profile", "upgrade", "--dry-run", "__nonexistent_probe__"])
            .output();

        match output {
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                // If nix doesn't recognize --dry-run, the error message will mention
                // the unrecognized flag. Otherwise the error will be about the element.
                let unsupported = stderr.contains("unrecognised flag")
                    || stderr.contains("unrecognized flag")
                    || stderr.contains("unknown flag");
                if unsupported {
                    tracing::info!("nix profile upgrade --dry-run is NOT supported");
                } else {
                    tracing::info!("nix profile upgrade --dry-run is supported");
                }
                !unsupported
            }
            Err(_) => {
                tracing::warn!("failed to probe for --dry-run support, assuming unsupported");
                false
            }
        }
    })
}

/// Check which packages have upgrades available via `nix profile upgrade --dry-run`.
/// Requires nix with https://github.com/NixOS/nix/pull/15545
fn packages_with_upgrades_dry_run() -> Result<Vec<String>> {
    let output = Command::new("nix")
        .env("NIXPKGS_ALLOW_UNFREE", "1")
        .env("NIXPKGS_ALLOW_INSECURE", "1")
        .args(["profile", "upgrade", "--dry-run", "--impure", "--all"])
        .output()
        .context("failed to run nix profile upgrade --dry-run")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("nix profile upgrade --dry-run failed: {}", stderr.trim());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut upgradable = Vec::new();

    for line in stderr.lines() {
        if let Some(rest) = line.strip_prefix("upgrading '") {
            if let Some(flake_ref) = rest.split('\'').next() {
                let name = flake_ref.rsplit_once('#').map_or(flake_ref, |(_, n)| n);
                tracing::info!("upgrade available for {name}");
                upgradable.push(name.to_string());
            }
        }
    }

    if upgradable.is_empty() {
        tracing::info!("no upgrades available");
    }

    Ok(upgradable)
}

/// Get store paths for each element in a profile.
fn profile_store_paths(profile: Option<&str>) -> Result<std::collections::HashMap<String, Vec<String>>> {
    let mut cmd = Command::new("nix");
    cmd.args(["profile", "list", "--json"]);
    if let Some(p) = profile {
        cmd.args(["--profile", p]);
    }
    let output = cmd.output().context("failed to run nix profile list")?;
    if !output.status.success() {
        anyhow::bail!(
            "nix profile list failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("failed to parse nix profile list json")?;

    let mut result = std::collections::HashMap::new();
    if let Some(elements) = json.get("elements").and_then(|e| e.as_object()) {
        for (name, element) in elements {
            let paths: Vec<String> = element
                .get("storePaths")
                .and_then(|p| p.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            result.insert(name.clone(), paths);
        }
    }

    Ok(result)
}

/// Check which packages have upgrades available by upgrading a temporary
/// profile copy and comparing store paths against the current profile.
/// Fallback for nix versions without --dry-run support.
fn packages_with_upgrades_temp_profile() -> Result<Vec<String>> {
    let tmp_dir = tempfile::tempdir().context("failed to create temp dir")?;
    let tmp_profile = tmp_dir.path().join("profile");

    // Get current profile path
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let current_profile = format!("{home}/.nix-profile");

    // Copy current profile to temp
    let status = Command::new("nix")
        .args(["profile", "list", "--profile", &current_profile])
        .status()
        .context("failed to verify current profile")?;
    if !status.success() {
        anyhow::bail!("current profile not accessible");
    }

    // Copy profile by creating a symlink to the same generation
    let real_profile = std::fs::read_link(&current_profile)
        .with_context(|| format!("failed to read profile link {current_profile}"))?;
    std::os::unix::fs::symlink(&real_profile, &tmp_profile)
        .context("failed to symlink temp profile")?;

    let before = profile_store_paths(Some(tmp_profile.to_str().unwrap()))?;

    // Upgrade the temp profile
    let output = Command::new("nix")
        .env("NIXPKGS_ALLOW_UNFREE", "1")
        .env("NIXPKGS_ALLOW_INSECURE", "1")
        .args([
            "profile", "upgrade", "--all", "--impure",
            "--profile", tmp_profile.to_str().unwrap(),
        ])
        .output()
        .context("failed to run nix profile upgrade on temp profile")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        sentry_ext::capture_cmd_failure(
            "nix profile upgrade --all (temp profile)",
            output.status.code(),
            stderr.trim(),
        );
        anyhow::bail!("nix profile upgrade (temp) failed: {}", stderr.trim());
    }

    let after = profile_store_paths(Some(tmp_profile.to_str().unwrap()))?;

    // Compare store paths to find what changed
    let mut upgradable = Vec::new();
    for (name, after_paths) in &after {
        if let Some(before_paths) = before.get(name) {
            if before_paths != after_paths {
                tracing::info!("upgrade available for {name}");
                upgradable.push(name.clone());
            }
        }
    }

    if upgradable.is_empty() {
        tracing::info!("no upgrades available");
    }

    // tmp_dir cleanup is automatic via Drop
    Ok(upgradable)
}

/// Check which packages have upgrades available.
/// Uses --dry-run if supported, otherwise falls back to temp profile comparison.
pub fn packages_with_upgrades() -> Result<Vec<String>> {
    if has_dry_run_support() {
        tracing::debug!("using --dry-run for upgrade check");
        packages_with_upgrades_dry_run()
    } else {
        tracing::debug!("using temp profile for upgrade check");
        packages_with_upgrades_temp_profile()
    }
}

/// Remove a package from the nix profile by element name.
pub fn profile_remove(pkg: &str) -> Result<()> {
    tracing::info!("removing nix profile element {pkg}");

    let status = Command::new("nix")
        .args(["profile", "remove", pkg])
        .status()
        .with_context(|| format!("failed to run nix profile remove {pkg}"))?;

    if !status.success() {
        sentry_ext::capture_cmd_failure(&format!("nix profile remove {pkg}"), status.code(), "");
        anyhow::bail!("nix profile remove {pkg} failed");
    }

    tracing::info!("nix profile remove {pkg} succeeded");
    sentry_ext::breadcrumb("nix", &format!("nix profile remove {pkg} succeeded"), &[("package", pkg)]);
    Ok(())
}

/// List installed element names from `nix profile list --json`.
pub fn installed_elements() -> Result<Vec<String>> {
    let output = Command::new("nix")
        .args(["profile", "list", "--json"])
        .output()
        .context("failed to run nix profile list")?;

    if !output.status.success() {
        anyhow::bail!(
            "nix profile list failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("failed to parse nix profile list json")?;

    let mut names = Vec::new();
    if let Some(elements) = json.get("elements").and_then(|e| e.as_object()) {
        for (name, _) in elements {
            names.push(name.clone());
        }
    }

    Ok(names)
}

/// Install or upgrade a package via `nix profile`.
pub fn profile_install(pkg: &str, upgrade: bool) -> Result<()> {
    let flake_ref = format!("{NIX_SOURCE}#{pkg}");
    let action = if upgrade { "upgrade" } else { "install" };
    tracing::info!("running nix profile {action} {pkg}");

    let mut cmd = Command::new("nix");
    cmd.env("NIXPKGS_ALLOW_UNFREE", "1")
        .env("NIXPKGS_ALLOW_INSECURE", "1")
        .arg("profile");

    if upgrade {
        // nix profile upgrade uses the installed element name (part after #)
        cmd.args(["upgrade", pkg]);
    } else {
        cmd.args(["add", &flake_ref]);
    }

    // --impure is needed when NIXPKGS_ALLOW_UNFREE is set
    cmd.arg("--impure");

    let status = cmd.status().with_context(|| format!("failed to run nix profile {action}"))?;

    if !status.success() {
        sentry_ext::capture_cmd_failure(
            &format!("nix profile {action} {pkg}"),
            status.code(),
            "",
        );
        anyhow::bail!("nix profile {action} {pkg} failed");
    }

    tracing::info!("nix profile {action} {pkg} succeeded");
    sentry_ext::breadcrumb("nix", &format!("nix profile {action} {pkg} succeeded"), &[("package", pkg)]);
    Ok(())
}
