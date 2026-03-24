use anyhow::{Context, Result};
use std::process::Command;

const NIX_SOURCE: &str = "https://git.plan.ai/plan-ai/nixpkgs/-/jobs/artifacts/plan-ai/raw/nixpkgs.tar.xz?job=build";

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

/// Check which packages have upgrades available via `nix profile upgrade --option dry-run true`.
/// Returns the names of packages that would be upgraded.
pub fn packages_with_upgrades() -> Result<Vec<String>> {
    let output = Command::new("nix")
        .env("NIXPKGS_ALLOW_UNFREE", "1")
        .env("NIXPKGS_ALLOW_INSECURE", "1")
        .args(["profile", "upgrade", "--option", "dry-run", "true", "--impure", "--all"])
        .output()
        .context("failed to run nix profile upgrade (dry-run)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("nix profile upgrade (dry-run) failed: {}", stderr.trim());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut upgradable = Vec::new();

    // nix profile upgrade --dry-run prints lines like:
    //   upgrading 'flake:nixpkgs#openclaw' from '...' to '...'
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
        anyhow::bail!("nix profile {action} {pkg} failed");
    }

    tracing::info!("nix profile {action} {pkg} succeeded");
    Ok(())
}
