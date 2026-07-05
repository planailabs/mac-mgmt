use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

const BUILTIN_PREFIX: &str = "builtin://";

/// Toggle write permission on the skills dir (and real subdirs holding
/// builtin skills) so agents don't write to it by accident. Symlinks to
/// /nix/store are left alone — the store is already read-only.
#[cfg(unix)]
fn set_skills_writable(dir: &Path, writable: bool) {
    use std::os::unix::fs::PermissionsExt;
    let (dir_mode, file_mode) = if writable {
        (0o755, 0o644)
    } else {
        (0o555, 0o444)
    };
    fn chmod(path: &Path, mode: u32) {
        if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)) {
            tracing::warn!("chmod {:o} {} failed: {e}", mode, path.display());
        }
    }
    fn walk(path: &Path, dir_mode: u32, file_mode: u32) {
        chmod(path, dir_mode);
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_symlink() {
                continue;
            }
            if p.is_dir() {
                walk(&p, dir_mode, file_mode);
            } else {
                chmod(&p, file_mode);
            }
        }
    }
    walk(dir, dir_mode, file_mode);
}

// ponytail: no-op on non-unix — readonly dir attributes don't block writes on Windows
#[cfg(not(unix))]
fn set_skills_writable(_dir: &Path, _writable: bool) {}

/// Relocks the skills dir read-only when dropped, so early returns and
/// errors during sync can't leave it writable.
struct WritableGuard<'a>(&'a Path);

impl Drop for WritableGuard<'_> {
    fn drop(&mut self) {
        set_skills_writable(self.0, false);
    }
}

#[derive(rust_embed::Embed)]
#[folder = "../skills/"]
#[include = "*/SKILL.md"]
struct BuiltinSkills;

/// Write embedded skill content to disk.
/// Returns `Ok(true)` if already up-to-date, `Ok(false)` if written fresh.
fn materialize_builtin(slug: &str, dest: &Path) -> Result<bool> {
    let asset_path = format!("{slug}/SKILL.md");
    let file = BuiltinSkills::get(&asset_path)
        .with_context(|| format!("no embedded content for builtin skill {slug}"))?;
    let content = std::str::from_utf8(&file.data)
        .with_context(|| format!("invalid UTF-8 in embedded skill {slug}"))?;

    let skill_md = dest.join("SKILL.md");

    // Idempotency: skip if content matches.
    if skill_md.exists() {
        if let Ok(existing) = std::fs::read_to_string(&skill_md) {
            if existing == content {
                return Ok(true);
            }
        }
    }

    std::fs::create_dir_all(dest)
        .with_context(|| format!("failed to create {}", dest.display()))?;
    std::fs::write(&skill_md, content)
        .with_context(|| format!("failed to write {}", skill_md.display()))?;

    tracing::info!("{slug}: materialized builtin skill");
    Ok(false)
}

async fn nix_system() -> Result<String> {
    let output = tokio::process::Command::new("nix-instantiate")
        .args(["--eval", "--expr", "builtins.currentSystem"])
        .output()
        .await
        .context("failed to run nix-instantiate --eval")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("nix-instantiate --eval failed: {stderr}");
    }

    let raw = String::from_utf8(output.stdout).context("invalid UTF-8 from nix-instantiate")?;
    // nix-instantiate returns a quoted string, e.g. "x86_64-linux"
    Ok(raw.trim().trim_matches('"').to_string())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn readonly_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("skills");
        let sub = dir.join("builtin-skill");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("SKILL.md"), "x").unwrap();

        set_skills_writable(&dir, false);
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir), 0o555);
        assert_eq!(mode(&sub), 0o555);
        assert_eq!(mode(&sub.join("SKILL.md")), 0o444);
        if unsafe { libc::geteuid() } != 0 {
            assert!(std::fs::write(sub.join("SKILL.md"), "y").is_err());
        }

        // unlock so writes work again and tempdir cleanup can delete it
        set_skills_writable(&dir, true);
        assert_eq!(mode(&dir), 0o755);
        std::fs::write(sub.join("SKILL.md"), "y").unwrap();
    }
}

pub async fn sync_skills(server_url: &str, token: &str, skills_dir: &Path) -> Result<()> {
    let arch = nix_system().await?;
    tracing::info!("syncing skills for architecture: {arch}");

    // Fetch skill→store_path mappings from server
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{server_url}/api/skills"))
        .query(&[("arch", &arch)])
        .bearer_auth(token)
        .send()
        .await
        .context("failed to reach skills API")?;

    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("skills API returned {status}");
    }

    let skills: HashMap<String, String> = resp
        .json()
        .await
        .context("failed to parse skills response")?;

    tracing::info!("server returned {} skill(s)", skills.len());
    if skills.is_empty() {
        tracing::debug!("no skills assigned to this cluster");
    }

    // Ensure skills directory exists, unlock it for the duration of the
    // sync; relocked read-only on drop (including error paths).
    std::fs::create_dir_all(skills_dir)
        .with_context(|| format!("failed to create {}", skills_dir.display()))?;
    set_skills_writable(skills_dir, true);
    let _relock = WritableGuard(skills_dir);

    // Realise each skill with --add-root to create a GC root link in skills_dir
    let mut up_to_date = 0u32;
    let mut realised = 0u32;
    let mut failed = 0u32;
    for (slug, store_path) in &skills {
        let link = skills_dir.join(slug);

        // Handle built-in skills: write embedded content to disk.
        if store_path.starts_with(BUILTIN_PREFIX) {
            match materialize_builtin(slug, &link) {
                Ok(true) => {
                    up_to_date += 1;
                }
                Ok(false) => {
                    realised += 1;
                }
                Err(e) => {
                    tracing::warn!("{slug}: failed to materialize builtin skill: {e}");
                    failed += 1;
                }
            }
            continue;
        }

        // Skip if already pointing to the right store path
        if let Ok(target) = std::fs::read_link(&link) {
            if target.to_string_lossy() == *store_path {
                tracing::debug!("{slug}: already at {store_path}");
                up_to_date += 1;
                continue;
            }
            tracing::info!("{slug}: updating from {} to {store_path}", target.display());
            // Remove stale link so --add-root can recreate it
            let _ = std::fs::remove_file(&link);
        } else {
            tracing::info!("{slug}: new skill, realising {store_path}");
        }

        let link_str = link.to_string_lossy().to_string();
        tracing::debug!("{slug}: nix-store --add-root {link_str} --realise {store_path}");
        let cache_args = crate::nix::extra_substituter_args();
        let output = tokio::process::Command::new("nix-store")
            .args(["--add-root", &link_str, "--realise", store_path])
            .args(&cache_args)
            .output()
            .await
            .with_context(|| format!("failed to run nix-store --realise for {slug}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("{slug}: nix-store --realise failed: {stderr}");
            failed += 1;
            continue;
        }

        tracing::info!("{slug}: linked -> {store_path}");
        realised += 1;
    }

    // Clean up skills not in the response
    let mut removed = 0u32;
    if let Ok(entries) = std::fs::read_dir(skills_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !skills.contains_key(name_str.as_ref()) {
                tracing::info!("removing old skill: {name_str}");
                let path = entry.path();
                if path.is_dir() {
                    let _ = std::fs::remove_dir_all(&path);
                } else {
                    let _ = std::fs::remove_file(&path);
                }
                removed += 1;
            }
        }
    }

    tracing::info!(
        "skills sync complete: {realised} realised, {up_to_date} up-to-date, {failed} failed, {removed} removed"
    );

    Ok(())
}
