use anyhow::{Context, Result};
use rust_embed::Embed;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[derive(Embed)]
#[folder = "scripts/"]
pub struct Scripts;

/// Run an embedded script by name (e.g. "setup.sh").
pub fn run(name: &str) -> Result<()> {
    let file = Scripts::get(name).with_context(|| format!("embedded script not found: {name}"))?;

    let mut tmp = tempfile::NamedTempFile::new().context("failed to create tempfile")?;
    tmp.write_all(&file.data)
        .context("failed to write script to tempfile")?;
    tmp.flush()?;

    // Make executable
    let perms = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(tmp.path(), perms)?;

    tracing::info!(
        "running embedded script '{}' via {}",
        name,
        tmp.path().display()
    );

    let status = Command::new("bash")
        .arg(tmp.path())
        .status()
        .with_context(|| format!("failed to execute {name}"))?;

    if !status.success() {
        anyhow::bail!("{name} exited with status {status}");
    }

    Ok(())
}

/// List all embedded script names.
pub fn list() -> Vec<String> {
    Scripts::iter().map(|f| f.to_string()).collect()
}
