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

/// Check if an upgrade is available for a package by dry-running `nix profile upgrade`.
pub fn has_upgrade(pkg: &str) -> Result<bool> {
    let name = pkg.rsplit_once('#').map_or(pkg, |(_, name)| name);

    let output = Command::new("nix")
        .env("NIXPKGS_ALLOW_UNFREE", "1")
        .env("NIXPKGS_ALLOW_INSECURE", "1")
        .args(["profile", "upgrade", "--dry-run", "--impure", name])
        .output()
        .context("failed to run nix profile upgrade --dry-run")?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    // If the dry-run mentions a new version or any store path changes, an upgrade is available.
    // A no-op dry-run produces no output.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let has_changes = !stdout.trim().is_empty() || stderr.contains("upgrading");

    if has_changes {
        tracing::info!("upgrade available for {name}");
    } else {
        tracing::info!("no upgrade available for {name}");
    }

    Ok(has_changes)
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
