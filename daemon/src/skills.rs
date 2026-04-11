use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

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

    // Ensure skills directory exists
    std::fs::create_dir_all(skills_dir)
        .with_context(|| format!("failed to create {}", skills_dir.display()))?;

    // Realise each skill with --add-root to create a GC root link in skills_dir
    let mut up_to_date = 0u32;
    let mut realised = 0u32;
    let mut failed = 0u32;
    for (slug, store_path) in &skills {
        let link = skills_dir.join(slug);

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
        let output = tokio::process::Command::new("nix-store")
            .args(["--add-root", &link_str, "--realise", store_path])
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
                let _ = std::fs::remove_file(entry.path());
                removed += 1;
            }
        }
    }

    tracing::info!(
        "skills sync complete: {realised} realised, {up_to_date} up-to-date, {failed} failed, {removed} removed"
    );

    Ok(())
}
