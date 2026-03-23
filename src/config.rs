use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

fn default_ollama_host() -> String {
    "127.0.0.1".to_string()
}

fn default_ollama_port() -> u16 {
    11434
}

fn default_ollama_models() -> Vec<String> {
    vec![
        "qwen3-coder-next".to_string(),
        "glm-5".to_string(),
        "kimi-k2.5".to_string(),
        "minimax-m2.7".to_string(),
    ]
}

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub openclaw: OpenClawConfig,
    #[serde(default)]
    pub ollama: OllamaConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct OpenClawConfig {
    #[serde(default)]
    pub extra_config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_host")]
    pub host: String,
    #[serde(default = "default_ollama_port")]
    pub port: u16,
    #[serde(default = "default_ollama_models")]
    pub models: Vec<String>,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: default_ollama_host(),
            port: default_ollama_port(),
            models: default_ollama_models(),
        }
    }
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
