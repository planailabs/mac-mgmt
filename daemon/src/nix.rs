use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use crate::sentry_ext;

const NIX_SOURCE_BASE: &str = "https://git.plan.ai/plan-ai/nixpkgs/-/jobs/artifacts/plan-ai/raw/nixpkgs.tar.xz?job=build";

fn nix_current_system() -> Result<&'static str> {
    static CACHED: OnceLock<Result<String, String>> = OnceLock::new();
    let result = CACHED.get_or_init(|| {
        let output = Command::new("nix-instantiate")
            .args(["--eval", "--expr", "builtins.currentSystem"])
            .output()
            .map_err(|e| format!("failed to run nix-instantiate: {e}"))?;
        if !output.status.success() {
            return Err("nix-instantiate failed".into());
        }
        String::from_utf8(output.stdout)
            .map(|s| s.trim().trim_matches('"').to_string())
            .map_err(|e| format!("non-utf8 nix system: {e}"))
    });
    match result {
        Ok(s) => Ok(s.as_str()),
        Err(e) => anyhow::bail!("{e}"),
    }
}

fn nix_source() -> Result<String> {
    let system = nix_current_system()?;
    Ok(format!("{NIX_SOURCE_BASE}_{system}"))
}

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
                // After '#' we get e.g. "legacyPackages.x86_64-linux.nix" — strip the first two dot-separated segments
                let after_hash = flake_ref.rsplit_once('#').map_or(flake_ref, |(_, n)| n);
                let name = after_hash.splitn(3, '.').last().unwrap_or(after_hash);
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
    let current_profile = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/root"))
        .join(".nix-profile");
    let current_profile = current_profile.to_string_lossy();

    // Copy current profile to temp
    let status = Command::new("nix")
        .args(["profile", "list", "--profile", &current_profile])
        .status()
        .context("failed to verify current profile")?;
    if !status.success() {
        anyhow::bail!("current profile not accessible");
    }

    // Copy profile by creating a symlink to the same generation
    let real_profile = std::fs::read_link(&*current_profile)
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
    profile_install_with_nix("nix", pkg, upgrade)
}

/// Install or upgrade a package via `nix profile`, using the given nix binary path.
fn profile_install_with_nix(nix_bin: &str, pkg: &str, upgrade: bool) -> Result<()> {
    let source = nix_source()?;
    let flake_ref = format!("{source}#{pkg}");
    let action = if upgrade { "upgrade" } else { "install" };
    tracing::info!("running nix profile {action} {pkg}");

    let mut cmd = Command::new(nix_bin);
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

/// Remove packages installed from the old NIX_SOURCE (without system suffix in the job name).
/// The old source ended with `?job=build#pkg` while the new one ends with `?job=build_<system>#pkg`.
pub fn remove_old_source_packages() -> Result<()> {
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

    let Some(elements) = json.get("elements").and_then(|e| e.as_object()) else {
        return Ok(());
    };

    for (name, element) in elements {
        let url = element
            .get("originalUrl")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        // Old source has originalUrl exactly equal to NIX_SOURCE_BASE (ending in ?job=build).
        // New source will have ?job=build_<system> so won't match.
        if url == NIX_SOURCE_BASE {
            tracing::info!("removing package {name} from old nix source: {url}");
            if let Err(e) = profile_remove(name) {
                tracing::warn!("failed to remove old-source package {name}: {e}");
            }
        }
    }

    Ok(())
}

/// Resolve the absolute path to the nix binary.
/// Must be called before any operation that might remove nix from the profile.
fn resolve_nix_binary() -> Result<PathBuf> {
    let output = Command::new("which")
        .arg("nix")
        .output()
        .context("failed to run which nix")?;

    if !output.status.success() {
        anyhow::bail!("nix binary not found in PATH");
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let resolved = PathBuf::from(&path);

    // Resolve symlinks to get the actual store path binary
    let canonical = std::fs::canonicalize(&resolved)
        .with_context(|| format!("failed to canonicalize nix path: {path}"))?;

    tracing::info!("resolved nix binary: {}", canonical.display());
    Ok(canonical)
}

/// Upgrade nix itself using a three-stage fallback:
/// 1. `nix upgrade-nix` (preferred, works for non-profile installs)
/// 2. `nix profile upgrade nix` (for profile-managed installs)
/// 3. Remove + reinstall from profile (if nix wasn't added from a flake)
pub fn upgrade_nix() -> Result<()> {
    // Resolve nix binary path upfront, before any removal
    let nix_bin = resolve_nix_binary()?;
    let nix_bin_str = nix_bin.to_str().context("nix binary path is not valid UTF-8")?;

    sentry_ext::breadcrumb("nix", "attempting nix self-upgrade", &[
        ("nix_bin", nix_bin_str),
    ]);

    // Stage 1: try nix upgrade-nix
    tracing::info!("trying nix upgrade-nix");
    let output = Command::new(nix_bin_str)
        .args(["upgrade-nix"])
        .output()
        .context("failed to run nix upgrade-nix")?;

    if output.status.success() {
        tracing::info!("nix upgrade-nix succeeded");
        sentry_ext::breadcrumb("nix", "nix upgrade-nix succeeded", &[]);
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    tracing::info!("nix upgrade-nix failed: {}", stderr.trim());

    // Check if the error is about profile-managed nix
    if !stderr.contains("managed by") {
        sentry_ext::capture_cmd_failure("nix upgrade-nix", output.status.code(), stderr.trim());
        anyhow::bail!("nix upgrade-nix failed: {}", stderr.trim());
    }

    // Stage 2: try nix profile upgrade nix
    tracing::info!("nix is profile-managed, trying nix profile upgrade nix");
    sentry_ext::breadcrumb("nix", "nix is profile-managed, trying profile upgrade", &[]);

    let output = Command::new(nix_bin_str)
        .env("NIXPKGS_ALLOW_UNFREE", "1")
        .env("NIXPKGS_ALLOW_INSECURE", "1")
        .args(["profile", "upgrade", "nix", "--impure"])
        .output()
        .context("failed to run nix profile upgrade nix")?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check if nix wasn't added from a flake (can't be upgraded in place)
    if stderr.contains("not added from a flake") {
        tracing::info!("nix was not added from a flake, will remove and reinstall");
        sentry_ext::breadcrumb("nix", "nix not from flake, removing and reinstalling", &[]);

        // Stage 3: remove nix (and nix-manual if present) from profile, then reinstall
        // Use the resolved absolute path for all subsequent nix commands

        // Remove nix-manual first if installed, as it clashes with the nix flake package
        let installed = installed_elements()?;
        if installed.iter().any(|name| name == "nix-manual") {
            tracing::info!("removing nix-manual before reinstalling nix");
            let status = Command::new(nix_bin_str)
                .args(["profile", "remove", "nix-manual"])
                .status()
                .context("failed to run nix profile remove nix-manual")?;

            if !status.success() {
                sentry_ext::capture_cmd_failure("nix profile remove nix-manual", status.code(), "");
                anyhow::bail!("nix profile remove nix-manual failed");
            }
            tracing::info!("nix-manual removed from profile");
        }

        let status = Command::new(nix_bin_str)
            .args(["profile", "remove", "nix"])
            .status()
            .context("failed to run nix profile remove nix")?;

        if !status.success() {
            sentry_ext::capture_cmd_failure("nix profile remove nix", status.code(), "");
            anyhow::bail!("nix profile remove nix failed");
        }

        tracing::info!("nix removed from profile, reinstalling via absolute path");
        profile_install_with_nix(nix_bin_str, "nix", false)?;

        tracing::info!("nix reinstalled successfully");
        sentry_ext::breadcrumb("nix", "nix reinstalled from flake", &[]);
        return Ok(());
    }

    if !output.status.success() {
        sentry_ext::capture_cmd_failure("nix profile upgrade nix", output.status.code(), stderr.trim());
        anyhow::bail!("nix profile upgrade nix failed: {}", stderr.trim());
    }

    tracing::info!("nix profile upgrade nix succeeded");
    sentry_ext::breadcrumb("nix", "nix profile upgrade succeeded", &[]);
    Ok(())
}
