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

// ── Config merge + validate ────────────────────────────────────────────

/// Options for [`merge_json_config`].
#[derive(Default)]
pub struct MergeValidateOpts<'a> {
    /// External command to run after writing (e.g. `["openclaw", "config", "validate"]`).
    pub validate_cmd: Option<&'a [&'a str]>,
    /// URL of a JSON Schema to validate the merged config against.
    /// Uses [`crate::schema_cache`] for caching.
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
        if let Err(summary) = crate::schema_cache::validate(url, &existing) {
            tracing::warn!(
                "JSON schema validation failed for {}: {summary}",
                config_path.display()
            );
            anyhow::bail!("JSON schema validation failed: {summary}");
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
