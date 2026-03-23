use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

use crate::services::ollama::OllamaConfig;
use crate::services::openclaw::OpenClawConfig;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub openclaw: OpenClawConfig,
    #[serde(default)]
    pub ollama: OllamaConfig,
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config/mac-mgmt/config.toml")
}

pub fn load() -> Result<Config> {
    let path = config_path();

    if !path.exists() {
        tracing::info!("no config at {}, using defaults", path.display());
        return Ok(Config::default());
    }

    tracing::info!("loading config from {}", path.display());
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let config: Config =
        toml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(config)
}
