use anyhow::{Context, Result};
use std::path::PathBuf;

use super::Connector;

/// Path to the ollama-env file in the config directory.
fn env_file_path() -> PathBuf {
    crate::config::config_dir().join("ollama-env")
}

/// Reads the current ollama-env file, returns the parsed key=value pairs.
pub fn load_env_file() -> std::collections::HashMap<String, String> {
    let path = env_file_path();
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (k, v) = line.split_once('=')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

/// Sets the OLLAMA_ORIGINS value for the relay tunnel in the ollama-env file.
/// Returns true if the file was changed.
pub struct RelayOllama;

impl Connector for RelayOllama {
    fn name(&self) -> &str {
        "relay→ollama"
    }

    fn depends_on(&self) -> &[&str] {
        &["relay", "ollama"]
    }

    fn connect(
        &self,
        configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let Some(relay_meta) = configs.get("relay") else {
            return Ok(());
        };
        let Some(proxy_hostname) = relay_meta.get("proxy_hostname").and_then(|v| v.as_str()) else {
            return Ok(());
        };
        let Some(instance_prefix) = relay_meta
            .get("instance_id_prefix")
            .and_then(|v| v.as_str())
        else {
            return Ok(());
        };

        let port = configs
            .get("ollama")
            .and_then(|v| v.get("port"))
            .and_then(|v| v.as_u64())
            .unwrap_or(11434);

        let origin_http = format!("http://{instance_prefix}-ollama.{proxy_hostname}");
        let origin_https = format!("https://{instance_prefix}-ollama.{proxy_hostname}");
        let new_origins = format!(
            "{origin_http},{origin_https},http://127.0.0.1:{port},http://[::1]:{port},http://localhost:{port}"
        );

        let path = env_file_path();

        // Read existing env file
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let mut lines: Vec<String> = existing
            .lines()
            .filter(|l| {
                let trimmed = l.trim();
                !trimmed.starts_with("OLLAMA_ORIGINS=")
            })
            .map(String::from)
            .collect();

        lines.push(format!("OLLAMA_ORIGINS={new_origins}"));

        let new_contents = lines.join("\n") + "\n";

        if existing == new_contents {
            tracing::debug!("relay→ollama: ollama-env unchanged");
            return Ok(());
        }

        std::fs::create_dir_all(path.parent().unwrap()).context("failed to create config dir")?;
        std::fs::write(&path, &new_contents)
            .with_context(|| format!("failed to write {}", path.display()))?;

        tracing::info!("relay→ollama: updated OLLAMA_ORIGINS in {}", path.display());
        Ok(())
    }
}
