use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock, RwLock};

use crate::sentry_ext;

/// Serialises all nix profile mutations so concurrent callers (e.g.
/// multiple managed-service upgrade checks firing in parallel) can't
/// corrupt the profile by racing add/remove/upgrade/replace. Held
/// for the duration of every public entry point that touches the
/// profile (`profile_install`, `profile_remove`, `upgrade_nix`,
/// `packages_with_upgrades`).
static PROFILE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn profile_lock() -> &'static Mutex<()> {
    PROFILE_LOCK.get_or_init(|| Mutex::new(()))
}

const NIX_SOURCE_BASE: &str = "https://git.plan.ai/plan-ai/nixpkgs/-/jobs/artifacts/plan-ai/raw/nixpkgs.tar.xz?job=build";

/// In-process pin: when set, all nix profile operations target this commit's
/// GitLab archive tarball instead of the rolling CI-artifact source.
static NIXPKGS_PIN: OnceLock<RwLock<Option<String>>> = OnceLock::new();

fn pin_cell() -> &'static RwLock<Option<String>> {
    NIXPKGS_PIN.get_or_init(|| RwLock::new(None))
}

pub fn set_nixpkgs_commit(commit: Option<String>) {
    *pin_cell().write().unwrap() = commit;
}

pub fn current_nixpkgs_commit() -> Option<String> {
    pin_cell().read().unwrap().clone()
}

/// Standard GitLab repo archive tarball for a commit. System-agnostic;
/// served by git.plan.ai without auth.
fn nixpkgs_tarball_url(commit: &str) -> String {
    format!(
        "https://git.plan.ai/plan-ai/nixpkgs/-/archive/{commit}/nixpkgs-{commit}.tar.bz2"
    )
}

/// Base flake URL (no `#attr`) for the desired state. Honours the per-cluster
/// pin if set, otherwise falls back to the legacy CI-artifact URL (which still
/// carries the `_<system>` suffix).
fn desired_flake_base() -> Result<String> {
    if let Some(sha) = current_nixpkgs_commit() {
        return Ok(nixpkgs_tarball_url(&sha));
    }
    let system = nix_current_system()?;
    Ok(format!("{NIX_SOURCE_BASE}_{system}"))
}

pub fn desired_flake_ref(pkg: &str) -> Result<String> {
    Ok(format!("{}#{pkg}", desired_flake_base()?))
}

/// Flake ref for unmanaged installs: uses the standard nixpkgs channel
/// instead of the git.plan.ai custom tarball. This makes unmanaged
/// services follow the host's nixpkgs pin rather than the cluster's.
pub fn nixpkgs_flake_ref(pkg: &str) -> String {
    format!("nixpkgs#{pkg}")
}

/// Check whether a package is installed from the git.plan.ai custom
/// flake and reinstall it from standard nixpkgs if so. Returns true
/// if a migration was performed.
pub fn migrate_to_nixpkgs(pkg: &str) -> Result<bool> {
    let urls = profile_original_urls()?;
    let Some(current_url) = urls.get(pkg) else {
        return Ok(false);
    };
    if !current_url.contains("git.plan.ai") && !current_url.contains("nixpkgs.tar") {
        return Ok(false);
    }
    tracing::info!(
        "migrating {pkg} from custom flake ({current_url}) to nixpkgs#"
    );
    let desired = nixpkgs_flake_ref(pkg);
    if has_replace_support() {
        run_profile_cmd("nix", "replace", pkg, &["replace", pkg, &desired])?;
    } else {
        pre_build_package("nix", pkg, &desired)?;
        run_profile_cmd("nix", "remove", pkg, &["remove", pkg])?;
        run_profile_cmd("nix", "add", pkg, &["add", &desired])?;
    }
    Ok(true)
}

pub fn current_system() -> Result<&'static str> {
    nix_current_system()
}

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

/// Cached result of whether `nix profile upgrade --dry-run` is supported.
/// `RwLock<Option<bool>>` (rather than `OnceLock<bool>`) so it can be
/// invalidated after `upgrade_nix` swaps the binary under us.
static DRY_RUN_SUPPORTED: RwLock<Option<bool>> = RwLock::new(None);

/// Cached result of whether `nix profile replace` is supported (only present
/// in this project's nix fork). Falls back to remove+add when absent.
static REPLACE_SUPPORTED: RwLock<Option<bool>> = RwLock::new(None);

/// Clear the cached `nix profile` capability probes. Call after upgrading nix
/// itself, since the new binary may support more (or fewer) verbs/flags.
pub fn invalidate_capability_cache() {
    *DRY_RUN_SUPPORTED.write().unwrap() = None;
    *REPLACE_SUPPORTED.write().unwrap() = None;
    tracing::info!("nix capability cache invalidated");
}

/// Probe whether `nix` recognises a particular subcommand/flag combination.
/// Runs the given args, checks stderr for any of the `unsupported_hints`
/// strings, caches the result in `cache`.
fn probe_nix_capability(
    cache: &RwLock<Option<bool>>,
    args: &[&str],
    unsupported_hints: &[&str],
    feature_name: &str,
) -> bool {
    if let Some(v) = *cache.read().unwrap() {
        return v;
    }
    let output = Command::new("nix").args(args).output();
    let supported = match output {
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let unsupported = unsupported_hints.iter().any(|h| stderr.contains(h));
            if unsupported {
                tracing::info!("{feature_name} is NOT supported");
            } else {
                tracing::info!("{feature_name} is supported");
            }
            !unsupported
        }
        Err(_) => {
            tracing::warn!("failed to probe for {feature_name} support, assuming unsupported");
            false
        }
    };
    *cache.write().unwrap() = Some(supported);
    supported
}

fn has_replace_support() -> bool {
    probe_nix_capability(
        &REPLACE_SUPPORTED,
        &["profile", "replace", "__nonexistent_probe__", "__nonexistent_probe__"],
        &["unknown subcommand", "unrecognised subcommand", "unrecognized subcommand", "unknown command", "is not a recognised command", "is not a recognized command"],
        "nix profile replace",
    )
}

/// Run `nix profile list --json` and return the parsed JSON.
/// Optionally targets a specific profile path.
fn profile_list_json(profile: Option<&str>) -> Result<serde_json::Value> {
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
    serde_json::from_slice(&output.stdout).context("failed to parse nix profile list json")
}

/// Check if a package is installed via `nix profile list --json`.
pub fn is_installed(pkg: &str) -> Result<bool> {
    let json = profile_list_json(None)?;

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

/// See: https://github.com/NixOS/nix/pull/15545
fn has_dry_run_support() -> bool {
    probe_nix_capability(
        &DRY_RUN_SUPPORTED,
        &["profile", "upgrade", "--dry-run", "__nonexistent_probe__"],
        &["unrecognised flag", "unrecognized flag", "unknown flag"],
        "nix profile upgrade --dry-run",
    )
}

/// Check which packages have upgrades available via `nix profile upgrade --dry-run`.
/// Requires nix with https://github.com/NixOS/nix/pull/15545
fn packages_with_upgrades_dry_run(packages: &[&str]) -> Result<Vec<String>> {
    let mut cmd = Command::new("nix");
    cmd.env("NIXPKGS_ALLOW_UNFREE", "1")
        .env("NIXPKGS_ALLOW_INSECURE", "1")
        .args(["profile", "upgrade", "--dry-run", "--impure"]);
    for pkg in packages {
        cmd.arg(*pkg);
    }
    let output = cmd
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
fn profile_store_paths(profile: Option<&str>) -> Result<HashMap<String, Vec<String>>> {
    let json = profile_list_json(profile)?;

    let mut result = HashMap::new();
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
fn packages_with_upgrades_temp_profile(packages: &[&str]) -> Result<Vec<String>> {
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
            "profile", "upgrade", "--impure",
            "--profile", tmp_profile.to_str().unwrap(),
        ])
        .args(packages)
        .output()
        .context("failed to run nix profile upgrade on temp profile")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        sentry_ext::capture_cmd_failure(
            "nix profile upgrade (temp profile)",
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

/// Check which of the given packages have upgrades available.
/// Uses --dry-run if supported, otherwise falls back to temp profile comparison.
/// Also unions in any installed packages whose flake URL has drifted from the
/// desired one (e.g. the cluster's nixpkgs pin moved) — those would not be
/// caught by `nix profile upgrade --dry-run` since the flake ref itself changed.
///
/// If the upgrade dry-run itself fails (e.g. because a drifted flake ref is no
/// longer resolvable), the error is logged and drift detection still runs so that
/// the package can be reinstalled from the correct flake URL.
pub fn packages_with_upgrades(packages: &[&str]) -> Result<Vec<String>> {
    let _guard = profile_lock().lock().unwrap_or_else(|e| e.into_inner());
    let upgrade_result = if has_dry_run_support() {
        tracing::debug!("using --dry-run for upgrade check");
        packages_with_upgrades_dry_run(packages)
    } else {
        tracing::debug!("using temp profile for upgrade check");
        packages_with_upgrades_temp_profile(packages)
    };

    let mut result: Vec<String> = match upgrade_result {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("upgrade dry-run failed, continuing with drift detection: {e}");
            Vec::new()
        }
    };

    // Add packages whose installed flake URL no longer matches the desired one.
    if let Ok(installed_urls) = profile_original_urls() {
        if let Ok(desired_base) = desired_flake_base() {
            for (name, url) in installed_urls {
                if packages.contains(&name.as_str())
                    && url != desired_base
                    && !result.contains(&name)
                {
                    tracing::info!("flake URL drift detected for {name}: {url} -> {desired_base}");
                    result.push(name);
                }
            }
        }
    }

    Ok(result)
}

/// Remove a package from the nix profile by element name.
pub fn profile_remove(pkg: &str) -> Result<()> {
    let _guard = profile_lock().lock().unwrap_or_else(|e| e.into_inner());
    run_profile_cmd("nix", "remove", pkg, &["remove", pkg])
}

/// List installed element names from `nix profile list --json`.
pub fn installed_elements() -> Result<Vec<String>> {
    let json = profile_list_json(None)?;
    let mut names = Vec::new();
    if let Some(elements) = json.get("elements").and_then(|e| e.as_object()) {
        for (name, _) in elements {
            names.push(name.clone());
        }
    }
    Ok(names)
}

/// Resolve a binary name to its nix store path by following symlinks.
/// Returns the store path prefix (e.g., `/nix/store/abc123-ollama-0.1/`),
/// or None if the binary isn't in the nix store.
pub fn binary_store_path(binary_name: &str) -> Option<String> {
    let output = Command::new("which").arg(binary_name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    store_path_prefix(&path)
}

/// Extract the `/nix/store/<hash>-<name>-<version>` prefix from an absolute
/// path (canonicalising symlinks). Returns `None` when the path doesn't live
/// in the nix store.
pub fn store_path_prefix(path: &str) -> Option<String> {
    let resolved = std::fs::canonicalize(path).ok()?;
    let resolved_str = resolved.to_string_lossy();
    if !resolved_str.starts_with("/nix/store/") {
        return None;
    }
    let rest = &resolved_str["/nix/store/".len()..];
    match rest.find('/') {
        Some(slash) => Some(format!("/nix/store/{}", &rest[..slash])),
        None => Some(resolved_str.into_owned()),
    }
}

/// Install or upgrade a package via `nix profile`.
pub fn profile_install(pkg: &str, upgrade: bool) -> Result<()> {
    let _guard = profile_lock().lock().unwrap_or_else(|e| e.into_inner());
    profile_install_with_nix("nix", pkg, upgrade)
}

/// Read `originalUrl` per element from `nix profile list --json`. Used to
/// detect drift between the installed flake URL and the desired one (which
/// may have moved if the cluster's nixpkgs pin changed).
fn profile_original_urls() -> Result<HashMap<String, String>> {
    let json = profile_list_json(None)?;
    let mut result = HashMap::new();
    if let Some(elements) = json.get("elements").and_then(|e| e.as_object()) {
        for (name, element) in elements {
            if let Some(url) = element.get("originalUrl").and_then(|v| v.as_str()) {
                result.insert(name.clone(), url.to_string());
            }
        }
    }
    Ok(result)
}

/// Pre-build a nix derivation so it lands in the store before any profile
/// mutation. Bails on build failure so the profile stays untouched.
/// Build the flake ref into the store without mutating any profile, so a
/// subsequent non-atomic profile operation (remove+add) doesn't get stuck
/// half-done if the build fails.
fn pre_build_package(nix_bin: &str, pkg: &str, flake_ref: &str) -> Result<()> {
    tracing::info!("pre-building {pkg} from {flake_ref}");
    let build = Command::new(nix_bin)
        .env("NIXPKGS_ALLOW_UNFREE", "1")
        .env("NIXPKGS_ALLOW_INSECURE", "1")
        .args(["build", "--no-link", "--impure", flake_ref])
        .output()
        .context("failed to run nix build for pre-build")?;
    if !build.status.success() {
        let stderr = String::from_utf8_lossy(&build.stderr);
        sentry_ext::capture_cmd_failure(
            &format!("nix build (pre-upgrade {pkg})"),
            build.status.code(),
            stderr.trim(),
        );
        anyhow::bail!("nix build for {pkg} upgrade failed: {}", stderr.trim());
    }
    tracing::info!("pre-build of {pkg} succeeded");
    Ok(())
}

/// Run a single `nix profile <args...>` invocation with the standard env +
/// `--impure` flag. Logs/breadcrumbs and bails on non-zero exit.
fn run_profile_cmd(nix_bin: &str, action: &str, pkg: &str, args: &[&str]) -> Result<()> {
    tracing::info!("running nix profile {action} {pkg}");
    let output = Command::new(nix_bin)
        .env("NIXPKGS_ALLOW_UNFREE", "1")
        .env("NIXPKGS_ALLOW_INSECURE", "1")
        .arg("profile")
        .args(args)
        .arg("--impure")
        .output()
        .with_context(|| format!("failed to run nix profile {action}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        tracing::error!(
            "nix profile {action} {pkg} failed (exit {:?})\nstdout: {}\nstderr: {}",
            output.status.code(),
            stdout.trim(),
            stderr.trim(),
        );
        sentry_ext::capture_cmd_failure(
            &format!("nix profile {action} {pkg}"),
            output.status.code(),
            stderr.trim(),
        );
        anyhow::bail!("nix profile {action} {pkg} failed: {}", stderr.trim());
    }

    tracing::info!("nix profile {action} {pkg} succeeded");
    sentry_ext::breadcrumb("nix", &format!("nix profile {action} {pkg} succeeded"), &[("package", pkg)]);
    Ok(())
}

/// Install or upgrade a package via `nix profile`, using the given nix binary path.
/// When `upgrade` is set and the installed flake URL differs from the desired
/// one (i.e. the cluster's nixpkgs pin moved), use `nix profile replace` if the
/// fork's verb is available, otherwise fall back to `remove` + `add`.
fn profile_install_with_nix(nix_bin: &str, pkg: &str, upgrade: bool) -> Result<()> {
    let desired = desired_flake_ref(pkg)?;
    let desired_base = desired_flake_base()?;

    if !upgrade {
        return run_profile_cmd(nix_bin, "install", pkg, &["add", &desired]);
    }

    let installed_url = profile_original_urls()
        .ok()
        .and_then(|m| m.get(pkg).cloned());

    if installed_url.as_deref() == Some(desired_base.as_str()) {
        // Same URL → in-place upgrade.
        return run_profile_cmd(nix_bin, "upgrade", pkg, &["upgrade", pkg]);
    }

    // Pin moved (or installed under a different URL) → swap the package over.
    if has_replace_support() {
        // `replace` is atomic: if the build fails the old generation stays
        // active, so pre-building would only duplicate work.
        run_profile_cmd(nix_bin, "replace", pkg, &["replace", pkg, &desired])
    } else {
        // Fallback is non-atomic (remove then add). Pre-build first so a
        // build failure doesn't leave the profile with the package missing.
        tracing::info!("nix profile replace unsupported, pre-building {pkg} before remove+add");
        pre_build_package(nix_bin, pkg, &desired)?;
        run_profile_cmd(nix_bin, "remove", pkg, &["remove", pkg])?;
        run_profile_cmd(nix_bin, "add", pkg, &["add", &desired])
    }
}

/// Resolve the absolute path to the nix binary.
/// Must be called before any operation that might remove nix from the profile.
/// Falls back to scanning `/nix/store/*/bin/nix` when nix isn't on PATH
/// (e.g. after a botched profile remove left the profile link broken).
fn resolve_nix_binary() -> Result<PathBuf> {
    let output = Command::new("which")
        .arg("nix")
        .output()
        .context("failed to run which nix")?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let canonical = std::fs::canonicalize(&path)
            .with_context(|| format!("failed to canonicalize nix path: {path}"))?;
        tracing::info!("resolved nix binary: {}", canonical.display());
        return Ok(canonical);
    }

    tracing::warn!("nix not found in PATH, scanning /nix/store for a usable binary");
    find_nix_in_store()
}

/// Walk `/nix/store/*/bin/nix` and return the first executable that
/// responds to `--version`. This is a last-resort recovery path —
/// the returned binary may be any version, but it's enough to
/// bootstrap a fresh `nix profile install`.
fn find_nix_in_store() -> Result<PathBuf> {
    let store = std::path::Path::new("/nix/store");
    if !store.is_dir() {
        anyhow::bail!("/nix/store does not exist");
    }
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(store)
        .context("reading /nix/store")?
        .filter_map(|e| e.ok())
        .map(|e| e.path().join("bin/nix"))
        .filter(|p| p.is_file())
        .collect();
    // Sort descending by mtime so we prefer the newest store path.
    candidates.sort_by(|a, b| {
        let ma = a.metadata().and_then(|m| m.modified()).ok();
        let mb = b.metadata().and_then(|m| m.modified()).ok();
        mb.cmp(&ma)
    });
    for candidate in &candidates {
        let ok = Command::new(candidate)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            tracing::info!(
                "found working nix binary in store: {}",
                candidate.display()
            );
            return Ok(candidate.clone());
        }
    }
    anyhow::bail!(
        "no working nix binary found in /nix/store (scanned {} candidates)",
        candidates.len()
    )
}

/// Call at daemon startup: if `nix` isn't on PATH, find any working
/// binary in `/nix/store` and use it to reinstall nix into the profile
/// so subsequent operations work normally.
pub fn ensure_nix_on_path() {
    let has_nix = Command::new("which")
        .arg("nix")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if has_nix {
        return;
    }
    tracing::warn!("nix not on PATH at startup — attempting store-based recovery");
    let nix_bin = match find_nix_in_store() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("cannot recover nix: {e}");
            return;
        }
    };
    let nix_str = match nix_bin.to_str() {
        Some(s) => s,
        None => {
            tracing::error!("nix store path is not valid UTF-8");
            return;
        }
    };
    let desired = match desired_flake_ref("nix") {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("cannot resolve desired nix flake ref: {e}");
            return;
        }
    };
    tracing::info!("reinstalling nix into profile using {}", nix_bin.display());
    if let Err(e) = run_profile_cmd(nix_str, "add", "nix", &["add", &desired]) {
        tracing::error!("failed to reinstall nix: {e}");
    }
}

/// Upgrade nix itself using a three-stage fallback:
/// 1. `nix upgrade-nix` (preferred, works for non-profile installs)
/// 2. `nix profile upgrade nix` (for profile-managed installs)
/// 3. Remove + reinstall from profile (if nix wasn't added from a flake)
///
/// On success, invalidates the cached `nix profile` capability probes since
/// the new binary may support more (or fewer) verbs/flags than the old one.
pub fn upgrade_nix() -> Result<()> {
    let _guard = profile_lock().lock().unwrap_or_else(|e| e.into_inner());
    let result = upgrade_nix_inner();
    if result.is_ok() {
        invalidate_capability_cache();
    }
    result
}

fn upgrade_nix_inner() -> Result<()> {
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

    // `nix profile upgrade` is non-atomic: it removes nix and then re-adds
    // it. Pre-build the new derivation first so a build failure doesn't
    // leave nix missing from the profile.
    match profile_original_urls() {
        Ok(urls) => match urls.get("nix") {
            Some(url) => pre_build_package(nix_bin_str, "nix", &format!("{url}#nix"))?,
            None => tracing::warn!("no originalUrl for nix element, skipping pre-build"),
        },
        Err(e) => tracing::warn!("could not list profile to pre-build nix: {e}"),
    }

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

        // Stage 3: remove nix (and nix-manual if present) from profile, then reinstall.
        // Pre-build the target derivation first so a build failure can't
        // leave the profile with nix removed and nothing to replace it.
        let desired = desired_flake_ref("nix")?;
        pre_build_package(nix_bin_str, "nix", &desired)?;

        let installed = installed_elements()?;
        if installed.iter().any(|name| name == "nix-manual") {
            run_profile_cmd(nix_bin_str, "remove", "nix-manual", &["remove", "nix-manual"])?;
        }

        run_profile_cmd(nix_bin_str, "remove", "nix", &["remove", "nix"])?;
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
