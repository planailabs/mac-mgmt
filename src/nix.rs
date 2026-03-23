use anyhow::{Context, Result};
use std::process::Command;

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

/// Check which packages have upgrades available by dry-running `nix profile upgrade`.
/// Upgrades all installed packages in a single dry-run and returns the names of those
/// that would be upgraded.
pub fn packages_with_upgrades() -> Result<Vec<String>> {
    let output = Command::new("nix")
        .env("NIXPKGS_ALLOW_UNFREE", "1")
        .env("NIXPKGS_ALLOW_INSECURE", "1")
        .args(["profile", "upgrade", "--dry-run", "--impure", "--all"])
        .output()
        .context("failed to run nix profile upgrade --dry-run")?;

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
    let action = if upgrade { "upgrade" } else { "install" };
    tracing::info!("running nix profile {action} {pkg}");

    let mut cmd = Command::new("nix");
    cmd.env("NIXPKGS_ALLOW_UNFREE", "1")
        .env("NIXPKGS_ALLOW_INSECURE", "1")
        .arg("profile");

    if upgrade {
        // nix profile upgrade uses the installed element name (part after #)
        let name = pkg.rsplit_once('#').map_or(pkg, |(_, name)| name);
        cmd.args(["upgrade", name]);
    } else {
        cmd.args(["install", pkg]);
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
