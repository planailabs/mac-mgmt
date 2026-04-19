use anyhow::{Context, Result};
use std::path::PathBuf;

pub use mac_mgmt_common::DaemonConfig as Config;

/// Default metrics port when not configured.
const DEFAULT_METRICS_PORT: u16 = 9396;

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// Returns ~/.config/mac-mgmt/, falling back to /root/.config/mac-mgmt/.
pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/root"))
        .join(".config/mac-mgmt")
}

/// Read the metrics port from the config file without fully loading/merging.
/// Used by CLI commands (status, logs) that need the port before the daemon starts.
pub fn read_metrics_port() -> u16 {
    let path = config_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str::<toml::Value>(&s).ok())
        .and_then(|v| v.get("metrics")?.get("port")?.as_integer())
        .map(|p| p as u16)
        .unwrap_or(DEFAULT_METRICS_PORT)
}

fn merge_json(base: &mut serde_json::Value, overlay: &serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            for (k, v) in o {
                merge_json(b.entry(k).or_insert(serde_json::Value::Null), v);
            }
        }
        (b, o) => {
            *b = o.clone();
        }
    }
}

async fn fetch_remote_config(url: &str, token: &str) -> Result<Option<serde_json::Value>> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{url}/api/config"))
        .bearer_auth(token)
        .send()
        .await
        .context("failed to reach config server")?;

    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        tracing::info!("remote config not found (404), skipping");
        return Ok(None);
    }
    if !status.is_success() {
        anyhow::bail!("config server returned {status}");
    }

    let json: serde_json::Value = resp.json().await.context("failed to parse remote config")?;
    Ok(Some(json))
}

pub async fn reload() -> Result<Config> {
    load().await
}

/// Load only the local TOML config without fetching remote config.
/// Used by CLI commands that only need [server] url/token.
pub fn load_local() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let cfg: Config =
        toml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(cfg)
}

pub async fn load() -> Result<Config> {
    let path = config_path();

    if !path.exists() {
        tracing::info!("no config at {}, using defaults", path.display());
        return Ok(Config::default());
    }

    tracing::info!("loading config from {}", path.display());
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let local_value: toml::Value =
        toml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))?;

    // Check for server config to fetch remote base config
    let server_url = local_value
        .get("server")
        .and_then(|s| s.get("url"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let server_token = local_value
        .get("server")
        .and_then(|s| s.get("token"))
        .and_then(|v| v.as_str())
        .map(String::from);

    if let (Some(url), Some(token)) = (server_url, server_token) {
        tracing::info!("fetching remote config from {url}");
        let remote = fetch_remote_config(&url, &token)
            .await
            .context("failed to fetch remote config")?;

        if let Some(mut remote_json) = remote {
            // Apply config migrations to the remote JSON before merging
            mac_mgmt_common::config_migrate::migrate(&mut remote_json);

            // Convert local TOML to JSON for merging
            let local_json: serde_json::Value =
                serde_json::to_value(toml::from_str::<toml::Value>(&contents)?)
                    .context("failed to convert local config to JSON")?;
            merge_json(&mut remote_json, &local_json);

            // Apply migrations again after merge in case local overlay
            // reintroduced old-format fields
            mac_mgmt_common::config_migrate::migrate(&mut remote_json);

            match serde_json::from_value::<Config>(remote_json.clone()) {
                Ok(config) => return Ok(config),
                Err(e) => {
                    tracing::warn!(
                        "failed to deserialize remote config after migration: {e}; \
                         falling back to local config"
                    );
                }
            }
        }
    }

    let config: Config =
        toml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(config)
}
