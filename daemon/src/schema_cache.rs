//! Global JSON Schema cache with async pre-fetch and sync fallback.
//!
//! Schemas are fetched at daemon startup via [`prefetch`] and stored
//! in-process.  At validation time, [`validate`] checks the cache first;
//! if the schema is missing it attempts a blocking runtime fetch. If
//! that also fails, validation is skipped with a warning.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use anyhow::{Context, Result};

static CACHE: OnceLock<Mutex<HashMap<String, serde_json::Value>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, serde_json::Value>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Pre-fetch a JSON Schema from `url` and store it in the global cache.
/// Called at daemon startup so validation doesn't block on network later.
/// Logs a warning and continues if the fetch fails.
pub async fn prefetch(url: &str) {
    tracing::info!("prefetching JSON schema from {url}");
    match fetch_async(url).await {
        Ok(schema) => {
            cache().lock().unwrap().insert(url.to_string(), schema);
            tracing::info!("cached JSON schema from {url}");
        }
        Err(e) => {
            tracing::warn!("failed to prefetch JSON schema from {url}: {e}");
        }
    }
}

async fn fetch_async(url: &str) -> Result<serde_json::Value> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()?;
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to fetch schema from {url}"))?;
    resp.json()
        .await
        .with_context(|| format!("failed to parse schema from {url}"))
}

/// Try to get a cached schema. If not cached, attempt a blocking fetch
/// and cache for next time. Returns `None` if the fetch fails.
fn get_or_fetch(url: &str) -> Option<serde_json::Value> {
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

/// Validate `instance` against a JSON Schema at the given URL.
///
/// Returns `Ok(())` if valid or if the schema can't be obtained (skip
/// validation). Returns `Err` with a summary of validation errors.
pub fn validate(url: &str, instance: &serde_json::Value) -> Result<(), String> {
    let Some(schema) = get_or_fetch(url) else {
        return Ok(());
    };
    let validator = match jsonschema::validator_for(&schema) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("failed to compile JSON schema from {url}: {e}");
            return Ok(());
        }
    };
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("{} (at {})", e, e.instance_path()))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// Validate a file's contents against a JSON Schema at the given URL.
///
/// Reads the file, parses as JSON, and validates against the schema.
/// Returns `Ok(())` if valid or if schema/file can't be processed.
/// Returns `Err(message)` on validation failure.
pub fn validate_file(url: &str, path: &std::path::Path) -> Result<(), String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("failed to read file: {e}"))?;
    let instance: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("invalid JSON: {e}"))?;
    validate(url, &instance)
}
