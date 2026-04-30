pub mod ai_proxy_svc;
pub mod apprise;
pub mod gpu_tool_common;
pub mod litellm;
pub mod lms;
pub mod mcporter;
pub mod nvidia_smi;
pub mod ollama;
pub mod openclaw;
pub mod opencode;
pub mod restic;
pub mod rocm_smi;
pub mod unsloth;

use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;

/// Non-blocking HTTP GET using reqwest. Must be called from an async context.
pub async fn http_get(host: &str, port: u16, path: &str) -> Result<String> {
    let url = format!("http://{host}:{port}{path}");
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build()?;
    let resp = client.get(&url).send().await?;
    Ok(resp.text().await?)
}

// ── JSON Schema cache ──────────────────────────────────────────────────

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Global schema cache. Populated at daemon startup, consulted by
/// `merge_json_config` when a `schema_url` is provided.
static SCHEMA_CACHE: OnceLock<Mutex<HashMap<String, serde_json::Value>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, serde_json::Value>> {
    SCHEMA_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Pre-fetch a JSON Schema from `url` and store it in the global cache.
/// Called at daemon startup so validation doesn't block on network later.
/// Logs a warning and continues if the fetch fails.
pub async fn prefetch_schema(url: &str) {
    tracing::info!("prefetching JSON schema from {url}");
    match fetch_schema(url).await {
        Ok(schema) => {
            cache().lock().unwrap().insert(url.to_string(), schema);
            tracing::info!("cached JSON schema from {url}");
        }
        Err(e) => {
            tracing::warn!("failed to prefetch JSON schema from {url}: {e}");
        }
    }
}

async fn fetch_schema(url: &str) -> Result<serde_json::Value> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()?;
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to fetch schema from {url}"))?;
    let schema: serde_json::Value = resp
        .json()
        .await
        .with_context(|| format!("failed to parse schema from {url}"))?;
    Ok(schema)
}

/// Try to get a cached schema. If not cached, attempt a blocking fetch
/// and cache for next time. Returns `None` if fetch fails.
fn get_or_fetch_schema(url: &str) -> Option<serde_json::Value> {
    {
        let c = cache().lock().unwrap();
        if let Some(schema) = c.get(url) {
            return Some(schema.clone());
        }
    }
    // Not cached — try a blocking fetch at runtime.
    tracing::info!("schema not cached, fetching {url} at runtime");
    match reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .and_then(|c| c.get(url).send())
        .and_then(|r| r.json::<serde_json::Value>())
    {
        Ok(schema) => {
            cache()
                .lock()
                .unwrap()
                .insert(url.to_string(), schema.clone());
            tracing::info!("fetched and cached JSON schema from {url}");
            Some(schema)
        }
        Err(e) => {
            tracing::warn!("failed to fetch JSON schema from {url}: {e}");
            None
        }
    }
}

/// Validate `instance` against a JSON schema from the given URL.
/// Returns a list of validation errors, or an empty vec if valid.
/// Returns `None` if the schema couldn't be obtained (skip validation).
fn validate_against_schema(url: &str, instance: &serde_json::Value) -> Option<Vec<String>> {
    let schema = get_or_fetch_schema(url)?;
    let validator = match jsonschema::validator_for(&schema) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("failed to compile JSON schema from {url}: {e}");
            return None;
        }
    };
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("{} (at {})", e, e.instance_path()))
        .collect();
    Some(errors)
}

// ── Config merge + validate ────────────────────────────────────────────

/// Options for [`merge_json_config`].
#[derive(Default)]
pub struct MergeValidateOpts<'a> {
    /// External command to run after writing (e.g. `["openclaw", "config", "validate"]`).
    pub validate_cmd: Option<&'a [&'a str]>,
    /// URL of a JSON Schema to validate the merged config against.
    /// Schemas are pre-fetched at startup; if not cached, a runtime
    /// fetch is attempted. Validation is skipped if the schema can't
    /// be obtained.
    pub schema_url: Option<&'a str>,
}

/// Atomically merge a JSON patch into a config file with optional validation
/// and rollback.
///
/// 1. Read current config (or `{}` if missing, creating parent dirs).
/// 2. Deep-merge `patch` into the existing JSON.
/// 3. Validate against JSON Schema (if `schema_url` set).
/// 4. Write the merged result.
/// 5. If `validate_cmd` is `Some`, run the command and rollback on failure.
///
/// Used by service setup, connectors, and config hot-reload paths.
pub fn merge_json_config(
    config_path: &Path,
    patch: &serde_json::Value,
    opts: MergeValidateOpts<'_>,
) -> Result<()> {
    let patch_keys: Vec<&str> = patch
        .as_object()
        .map(|o| o.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();
    tracing::info!(
        "merge_json_config: merging patch with keys [{}] into {}",
        patch_keys.join(", "),
        config_path.display()
    );
    tracing::debug!("merge_json_config: patch content: {patch}");

    let backup = if config_path.exists() {
        std::fs::read_to_string(config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?
    } else {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        "{}".to_string()
    };

    let mut existing: serde_json::Value =
        serde_json::from_str(&backup).context("failed to parse existing config")?;

    crate::connectors::merge_json(&mut existing, patch);

    // Validate against JSON Schema before writing, if configured.
    if let Some(url) = opts.schema_url {
        if let Some(errors) = validate_against_schema(url, &existing) {
            if !errors.is_empty() {
                let summary = errors.join("; ");
                tracing::warn!("JSON schema validation failed for {}: {summary}", config_path.display());
                anyhow::bail!("JSON schema validation failed: {summary}");
            }
        }
    }

    let merged =
        serde_json::to_string_pretty(&existing).context("failed to serialize merged config")?;

    tracing::debug!(
        "merge_json_config: writing merged config ({} bytes)",
        merged.len()
    );
    std::fs::write(config_path, &merged)
        .with_context(|| format!("failed to write {}", config_path.display()))?;

    // Run external validator if provided.
    if let Some(cmd) = opts.validate_cmd {
        let (program, args) = cmd.split_first().context("validate_cmd is empty")?;
        let valid = match crate::cmd::output_with_timeout(
            std::process::Command::new(program).args(args.iter()),
            crate::cmd::DEFAULT_TIMEOUT,
        ) {
            Ok(output) if output.status.success() => true,
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                tracing::warn!(
                    "config validation failed: {} {}",
                    stdout.trim(),
                    stderr.trim()
                );
                false
            }
            Err(e) => {
                tracing::warn!("failed to run validator: {e}");
                true // Can't validate — don't block.
            }
        };

        if !valid {
            tracing::warn!("rolling back {}", config_path.display());
            std::fs::write(config_path, &backup)
                .with_context(|| format!("failed to rollback {}", config_path.display()))?;
            anyhow::bail!("config validation failed, rolled back");
        }
    }

    Ok(())
}
