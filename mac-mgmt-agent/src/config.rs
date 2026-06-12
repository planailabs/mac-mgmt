use anyhow::{Context, Result};
use std::path::PathBuf;

pub use mac_mgmt_common::DaemonConfig as Config;

/// Default metrics port when not configured.
const DEFAULT_METRICS_PORT: u16 = 9396;

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// Returns ~/.config/mac-mgmt/, falling back to /root/.config/mac-mgmt/.
/// MAC_MGMT_CONFIG_DIR overrides the whole path — the plan-ai-usb daemon pins
/// its state to the stick this way (`HOME` pinning alone is unix-only:
/// dirs::home_dir() ignores the env var on windows).
pub fn config_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("MAC_MGMT_CONFIG_DIR") {
        return PathBuf::from(d);
    }
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

pub fn merge_json(base: &mut serde_json::Value, overlay: &serde_json::Value) {
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

/// Load environment variables from `~/.config/mac-mgmt/.env` (dotenv format).
/// Returns an empty map if the file doesn't exist.
pub fn load_env_file() -> std::collections::HashMap<String, String> {
    let path = config_dir().join(".env");
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Default::default(),
    };
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

fn remote_config_cache_path() -> PathBuf {
    config_dir().join(".remote-config.json")
}

fn cache_remote_config(json: &serde_json::Value) {
    let path = remote_config_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string(json) {
        Ok(s) => {
            if let Err(e) = std::fs::write(&path, s) {
                tracing::warn!("failed to cache remote config to {}: {e}", path.display());
            }
        }
        Err(e) => tracing::warn!("failed to serialize remote config for cache: {e}"),
    }
}

fn load_cached_remote_config() -> Option<serde_json::Value> {
    let path = remote_config_cache_path();
    let contents = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str(&contents) {
        Ok(v) => {
            tracing::info!("using cached remote config from {}", path.display());
            Some(v)
        }
        Err(e) => {
            tracing::warn!("cached remote config is invalid: {e}");
            None
        }
    }
}

pub async fn fetch_remote_config(
    url: &str,
    token: &str,
) -> Result<Option<serde_json::Value>> {
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
    cache_remote_config(&json);
    Ok(Some(json))
}

/// Fetch all secrets for this cluster from the server vault.
/// Returns an empty map on failure (secrets are optional).
pub async fn fetch_secrets(
    url: &str,
    token: &str,
) -> std::collections::HashMap<String, String> {
    let client = reqwest::Client::new();
    let resp = match client
        .get(format!("{url}/api/secrets"))
        .bearer_auth(token)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("failed to fetch secrets: {e}");
            return crate::secrets_cache::load_cached_secrets().unwrap_or_default();
        }
    };

    if !resp.status().is_success() {
        tracing::debug!("secrets endpoint returned {}", resp.status());
        return crate::secrets_cache::load_cached_secrets().unwrap_or_default();
    }

    match resp
        .json::<std::collections::HashMap<String, String>>()
        .await
    {
        Ok(secrets) => {
            if let Err(e) = crate::secrets_cache::cache_secrets(&secrets) {
                tracing::warn!("failed to cache secrets: {e}");
            }
            secrets
        }
        Err(e) => {
            tracing::warn!("failed to parse secrets response: {e}");
            crate::secrets_cache::load_cached_secrets().unwrap_or_default()
        }
    }
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

    // Load local env vars for secret resolution.
    let env_vars = load_env_file();

    // Check for server config to fetch remote base config.
    // Resolve env: references on the token since it's needed before full resolution.
    let server_url = local_value
        .get("server")
        .and_then(|s| s.get("url"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let server_token = local_value
        .get("server")
        .and_then(|s| s.get("token"))
        .and_then(|v| v.as_str())
        .map(|s| {
            if let Some(var) = s.strip_prefix("env:") {
                env_vars.get(var).cloned().unwrap_or_else(|| {
                    tracing::warn!("env var '{var}' not found in .env for server.token");
                    s.to_string()
                })
            } else {
                s.to_string()
            }
        });

    // Fetch secrets from the vault (if server is configured).
    let vault = if let (Some(url), Some(token)) = (&server_url, &server_token) {
        fetch_secrets(url, token).await
    } else {
        crate::secrets_cache::load_cached_secrets().unwrap_or_default()
    };

    if let (Some(url), Some(token)) = (server_url, server_token) {
        tracing::info!("fetching remote config from {url}");
        let remote = match fetch_remote_config(&url, &token).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("failed to fetch remote config: {e}; trying cached version");
                load_cached_remote_config()
            }
        };

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
                Ok(mut config) => {
                    resolve_config_secrets(&mut config, &env_vars, &vault);
                    return Ok(config);
                }
                Err(e) => {
                    tracing::warn!(
                        "failed to deserialize remote config after migration: {e}; \
                         falling back to local config"
                    );
                }
            }
        }
    }

    let mut config: Config =
        toml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))?;
    resolve_config_secrets(&mut config, &env_vars, &vault);
    Ok(config)
}

/// Resolve `env:` and `secret:` references in all Secret fields of the config.
/// Logs warnings on resolution failures but does not fail the load.
fn resolve_config_secrets(
    config: &mut Config,
    env_vars: &std::collections::HashMap<String, String>,
    vault: &std::collections::HashMap<String, String>,
) {
    if let Err(errors) = config.resolve_secrets(env_vars, vault) {
        for e in &errors {
            tracing::warn!("secret resolution failed: {e}");
        }
    }
}
