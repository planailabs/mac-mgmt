use anyhow::{Context, Result};
use std::path::PathBuf;

pub use mac_mgmt_common::DaemonConfig as Config;

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config/mac-mgmt/config.toml")
}

fn merge_toml(base: &mut toml::Value, overlay: &toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(b), toml::Value::Table(o)) => {
            for (k, v) in o {
                merge_toml(
                    b.entry(k).or_insert(toml::Value::Boolean(false)),
                    v,
                );
            }
        }
        (b, o) => {
            *b = o.clone();
        }
    }
}

async fn fetch_remote_config(url: &str, token: &str) -> Result<Option<toml::Value>> {
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

    let body = resp.text().await.context("failed to read response body")?;
    let value: toml::Value = toml::from_str(&body).context("failed to parse remote config")?;
    Ok(Some(value))
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

        if let Some(mut remote) = remote {
            merge_toml(&mut remote, &local_value);
            let config: Config = remote
                .try_into()
                .context("failed to deserialize merged config")?;
            return Ok(config);
        }
    }

    let config: Config =
        toml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(config)
}
