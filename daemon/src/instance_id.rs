use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

fn instance_id_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config/mac-mgmt/instance-id")
}

pub fn get_or_create() -> Result<String> {
    let path = instance_id_path();
    if path.exists() {
        let id = fs::read_to_string(&path)
            .with_context(|| format!("failed to read instance ID from {}", path.display()))?
            .trim()
            .to_string();
        if !id.is_empty() {
            return Ok(id);
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let id = uuid::Uuid::new_v4().to_string();
    fs::write(&path, &id)
        .with_context(|| format!("failed to write instance ID to {}", path.display()))?;
    tracing::info!("generated new instance ID: {id}");
    Ok(id)
}
