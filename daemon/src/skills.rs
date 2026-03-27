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

    // Ensure skills directory exists
    std::fs::create_dir_all(skills_dir)
        .with_context(|| format!("failed to create {}", skills_dir.display()))?;

    // Realise each skill with --add-root to create a GC root link in skills_dir
    for (slug, store_path) in &skills {
        let link = skills_dir.join(slug);

        // Skip if already pointing to the right store path
        if let Ok(target) = std::fs::read_link(&link) {
            if target.to_string_lossy() == *store_path {
                continue;
            }
            // Remove stale link so --add-root can recreate it
            let _ = std::fs::remove_file(&link);
        }

        let link_str = link.to_string_lossy().to_string();
        tracing::info!("realising skill {slug}: {store_path}");
        let output = tokio::process::Command::new("nix-store")
            .args(["--add-root", &link_str, "--realise", store_path])
            .output()
            .await
            .with_context(|| format!("failed to run nix-store --realise for {slug}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("nix-store --realise failed for {slug}: {stderr}");
            continue;
        }

        tracing::info!("linked {slug} -> {store_path}");
    }

    // Clean up skills not in the response
    if let Ok(entries) = std::fs::read_dir(skills_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !skills.contains_key(name_str.as_ref()) {
                tracing::info!("removing old skill: {name_str}");
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    Ok(())
}
