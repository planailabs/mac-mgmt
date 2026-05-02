//! Unified config file validation.
//!
//! [`Validator`] provides format-aware syntax checking (JSON / TOML / YAML),
//! JSON Schema validation (from a URL, local file, or command output), and
//! external command validation.  It is the single validation primitive shared
//! by the config merge pipeline ([`Validator::merge_validate_and_write`]) and
//! the relay file-tunnel write path.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ── Public types ───────────────────────────────────────────────────────

/// Config file format — determines how syntax is checked and how values
/// are serialized back after a merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigFormat {
    Json,
    Toml,
    Yaml,
}

/// Where the authoritative schema lives.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SchemaSource {
    /// JSON Schema fetched from a URL (cached globally, periodically refreshed).
    Url { url: String },
    /// JSON Schema loaded from a local file.
    File { path: String },
    /// External command that exits 0 when the file is valid.
    /// `{}` in any arg is replaced with the file's absolute path.
    Command { command: Vec<String> },
    /// External command whose stdout is a JSON Schema (cached globally,
    /// periodically refreshed).  E.g. `["openclaw", "config", "schema"]`.
    Exec { command: Vec<String> },
}

/// Unified config-file validator.
///
/// Each instance describes *which* files it applies to (via [`pattern`]),
/// *how* to parse them ([`format`]), and optionally *what* to validate
/// against ([`schema`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    /// Glob pattern that selects which filenames this validator covers
    /// (e.g. `"*.json"`, `"config.toml"`).
    pub pattern: String,
    /// File format for syntax validation.
    pub format: ConfigFormat,
    /// Optional deeper validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<SchemaSource>,
}

// ── Builders ───────────────────────────────────────────────────────────

impl Validator {
    /// JSON file validator (syntax only).
    pub fn json(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            format: ConfigFormat::Json,
            schema: None,
        }
    }
    /// TOML file validator (syntax only).
    pub fn toml(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            format: ConfigFormat::Toml,
            schema: None,
        }
    }
    /// YAML file validator (syntax only).
    pub fn yaml(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            format: ConfigFormat::Yaml,
            schema: None,
        }
    }

    /// Add a URL-based JSON Schema.
    pub fn with_schema_url(mut self, url: impl Into<String>) -> Self {
        self.schema = Some(SchemaSource::Url { url: url.into() });
        self
    }
    /// Add a file-based JSON Schema.
    pub fn with_schema_file(mut self, path: impl Into<String>) -> Self {
        self.schema = Some(SchemaSource::File { path: path.into() });
        self
    }
    /// Add an external validation command.
    pub fn with_command(mut self, command: Vec<String>) -> Self {
        self.schema = Some(SchemaSource::Command { command });
        self
    }
    /// Add a command whose stdout is a JSON Schema.
    pub fn with_exec(mut self, command: Vec<String>) -> Self {
        self.schema = Some(SchemaSource::Exec { command });
        self
    }
}

// ── Core validation ────────────────────────────────────────────────────

impl Validator {
    /// Parse `content` according to [`format`], returning a JSON Value.
    /// TOML and YAML are converted to the serde_json representation.
    pub fn parse(&self, content: &str) -> Result<serde_json::Value, String> {
        match self.format {
            ConfigFormat::Json => {
                serde_json::from_str(content).map_err(|e| format!("invalid JSON: {e}"))
            }
            ConfigFormat::Toml => {
                let tv: toml::Value =
                    content.parse().map_err(|e| format!("invalid TOML: {e}"))?;
                serde_json::to_value(tv)
                    .map_err(|e| format!("TOML→JSON conversion failed: {e}"))
            }
            ConfigFormat::Yaml => {
                yaml_serde::from_str(content).map_err(|e| format!("invalid YAML: {e}"))
            }
        }
    }

    /// Serialize a JSON Value back to the configured format.
    fn serialize(&self, value: &serde_json::Value) -> Result<String, String> {
        match self.format {
            ConfigFormat::Json => serde_json::to_string_pretty(value)
                .map_err(|e| format!("JSON serialization failed: {e}")),
            ConfigFormat::Yaml => yaml_serde::to_string(value)
                .map_err(|e| format!("YAML serialization failed: {e}")),
            ConfigFormat::Toml => {
                Err("TOML round-trip serialization from JSON Value is not supported".into())
            }
        }
    }

    /// Validate an in-memory value against the schema source.
    ///
    /// Command-based schemas are silently accepted here because they
    /// require a file on disk — use [`validate_file`] for those.
    pub fn validate_value(&self, value: &serde_json::Value) -> Result<(), String> {
        match &self.schema {
            Some(SchemaSource::Url { url }) => validate_with_cached_schema(url, value),
            Some(SchemaSource::File { path }) => validate_with_file_schema(path, value),
            Some(SchemaSource::Exec { command }) => {
                let key = exec_cache_key(command);
                validate_with_cached_schema(&key, value)
            }
            Some(SchemaSource::Command { .. }) | None => Ok(()),
        }
    }

    /// Validate a file on disk: syntax check → schema → command.
    pub fn validate_file(&self, path: &Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        let value = self.parse(&content)?;
        self.validate_value(&value)?;
        self.validate_command(path)?;
        Ok(())
    }

    /// Run a command-based validator against a file on disk.
    fn validate_command(&self, file_path: &Path) -> Result<(), String> {
        let Some(SchemaSource::Command { command }) = &self.schema else {
            return Ok(());
        };
        if command.is_empty() {
            return Ok(());
        }
        let path_str = file_path.to_string_lossy();
        let args: Vec<String> = command.iter().map(|a| a.replace("{}", &path_str)).collect();

        match crate::cmd::output_with_timeout(
            std::process::Command::new(&args[0]).args(&args[1..]),
            crate::cmd::DEFAULT_TIMEOUT,
        ) {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                Err(format!("{} {}", stdout.trim(), stderr.trim())
                    .trim()
                    .to_string())
            }
            Err(e) => {
                // Binary unavailable — warn but don't block.
                tracing::warn!("validation command {:?} failed to run: {e}", args[0]);
                Ok(())
            }
        }
    }
}

// ── Merge + validate + write ───────────────────────────────────────────

impl Validator {
    /// Read → merge → validate → write a config file.
    ///
    /// 1. Read existing file (or empty doc if missing, creating parent dirs).
    /// 2. Deep-merge `patch` into the existing value.
    /// 3. Validate merged value against URL/file/exec schema (pre-write).
    /// 4. Serialize and write.
    /// 5. Run command validator if configured; rollback on failure.
    pub fn merge_validate_and_write(
        &self,
        config_path: &Path,
        patch: &serde_json::Value,
    ) -> Result<()> {
        let patch_keys: Vec<&str> = patch
            .as_object()
            .map(|o| o.keys().map(|k| k.as_str()).collect())
            .unwrap_or_default();
        tracing::info!(
            "merge_validate_and_write: merging [{}] into {}",
            patch_keys.join(", "),
            config_path.display()
        );
        tracing::debug!("merge_validate_and_write: patch: {patch}");

        let backup = if config_path.exists() {
            std::fs::read_to_string(config_path)
                .with_context(|| format!("failed to read {}", config_path.display()))?
        } else {
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            match self.format {
                ConfigFormat::Json => "{}".into(),
                ConfigFormat::Yaml | ConfigFormat::Toml => String::new(),
            }
        };

        let mut existing = self
            .parse(&backup)
            .map_err(|e| anyhow::anyhow!("failed to parse existing config: {e}"))?;

        merge_json(&mut existing, patch);

        // Pre-write validation (URL / file / exec schemas).
        self.validate_value(&existing)
            .map_err(|e| anyhow::anyhow!("schema validation failed: {e}"))?;

        let merged = self
            .serialize(&existing)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        std::fs::write(config_path, &merged)
            .with_context(|| format!("failed to write {}", config_path.display()))?;

        // Post-write validation (command schemas).
        if let Err(e) = self.validate_command(config_path) {
            tracing::warn!("rolling back {}", config_path.display());
            std::fs::write(config_path, &backup)
                .with_context(|| format!("failed to rollback {}", config_path.display()))?;
            anyhow::bail!("command validation failed, rolled back: {e}");
        }

        Ok(())
    }
}

// ── Prefetch & refresh ─────────────────────────────────────────────────

impl Validator {
    /// If this validator uses a URL or Exec schema, fetch and cache it.
    pub async fn prefetch(&self) {
        match &self.schema {
            Some(SchemaSource::Url { url }) => cache_store_url(url).await,
            Some(SchemaSource::Exec { command }) => cache_store_exec(command),
            _ => {}
        }
    }
}

/// Pre-fetch schemas for a set of validators.
pub async fn prefetch_all(validators: &[Validator]) {
    for v in validators {
        v.prefetch().await;
    }
}

/// Re-fetch all cached URL and Exec schemas.  Call periodically (e.g.
/// on the update tick) so schema changes are picked up without a
/// daemon restart.
pub async fn refresh_all(validators: &[Validator]) {
    for v in validators {
        match &v.schema {
            Some(SchemaSource::Url { url }) => cache_store_url(url).await,
            Some(SchemaSource::Exec { command }) => cache_store_exec(command),
            _ => {}
        }
    }
}

// ── Pattern matching (used by file tunnels) ────────────────────────────

impl Validator {
    /// Whether this validator's pattern matches `path`.
    ///
    /// Uses `require_literal_separator` so `*` does not cross `/`
    /// boundaries — pattern `openclaw.json` matches `openclaw.json`
    /// but not `skills/openclaw.json`.  Pass a relative path (within
    /// the tunnel folder) for scoped matching.
    pub fn matches(&self, path: &Path) -> bool {
        let s = path.to_string_lossy();
        glob::Pattern::new(&self.pattern).is_ok_and(|p| {
            p.matches_with(
                &s,
                glob::MatchOptions {
                    require_literal_separator: true,
                    ..Default::default()
                },
            )
        })
    }
}

/// Find the first validator whose pattern matches `path`.
pub fn find_matching<'a>(validators: &'a [Validator], path: &Path) -> Option<&'a Validator> {
    validators.iter().find(|v| v.matches(path))
}

// ── Deep JSON merge ────────────────────────────────────────────────────

/// Recursively merge `source` into `target`.  For objects, keys from source
/// are merged into target.  For all other types, source overwrites target.
pub fn merge_json(target: &mut serde_json::Value, source: &serde_json::Value) {
    match (target, source) {
        (serde_json::Value::Object(target), serde_json::Value::Object(source)) => {
            for (key, value) in source {
                merge_json(
                    target.entry(key.clone()).or_insert(serde_json::Value::Null),
                    value,
                );
            }
        }
        (target, source) => {
            *target = source.clone();
        }
    }
}

// ── Schema cache (private) ─────────────────────────────────────────────

/// Cache key for Exec schemas: "exec:<program> <args...>"
fn exec_cache_key(command: &[String]) -> String {
    format!("exec:{}", command.join(" "))
}

static SCHEMA_CACHE: OnceLock<Mutex<HashMap<String, serde_json::Value>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, serde_json::Value>> {
    SCHEMA_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Fetch a URL schema and store it in the cache.
async fn cache_store_url(url: &str) {
    tracing::info!("fetching JSON schema from {url}");
    let result = async {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()?;
        let resp = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("fetch {url}"))?;
        resp.json::<serde_json::Value>()
            .await
            .with_context(|| format!("parse {url}"))
    }
    .await;
    match result {
        Ok(schema) => {
            cache().lock().unwrap().insert(url.to_string(), schema);
            tracing::info!("cached JSON schema from {url}");
        }
        Err(e) => {
            tracing::warn!("failed to fetch JSON schema from {url}: {e}");
        }
    }
}

/// Run a command, parse its stdout as a JSON Schema, and cache it.
fn cache_store_exec(command: &[String]) {
    if command.is_empty() {
        return;
    }
    let key = exec_cache_key(command);
    tracing::info!("fetching JSON schema via command: {}", command.join(" "));
    match crate::cmd::output_with_timeout(
        std::process::Command::new(&command[0]).args(&command[1..]),
        crate::cmd::DEFAULT_TIMEOUT,
    ) {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(mut schema) => {
                    // HACK: openclaw emits schemas with broken $defs pointers;
                    // strip them so jsonschema can compile the schema at all.
                    strip_defs_refs(&mut schema);
                    cache().lock().unwrap().insert(key, schema);
                    tracing::info!("cached JSON schema from command: {}", command.join(" "));
                }
                Err(e) => {
                    tracing::warn!(
                        "command {} produced invalid JSON schema: {e}",
                        command.join(" ")
                    );
                }
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!(
                "schema command {} failed: {}",
                command.join(" "),
                stderr.trim()
            );
        }
        Err(e) => {
            tracing::warn!(
                "schema command {} failed to run: {e}",
                command.join(" ")
            );
        }
    }
}

/// Look up a cached schema by key (URL or exec key).  If not cached,
/// attempt a blocking URL fetch (for URL keys only).
fn get_cached_schema(key: &str) -> Option<serde_json::Value> {
    {
        let c = cache().lock().unwrap();
        if let Some(schema) = c.get(key) {
            return Some(schema.clone());
        }
    }
    // For URL keys, try a blocking runtime fetch as fallback.
    if key.starts_with("exec:") {
        tracing::warn!("exec schema not cached for {key}, skipping validation");
        return None;
    }
    tracing::info!("schema not cached, fetching {key} at runtime");
    match reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .and_then(|c| c.get(key).send())
        .and_then(|r| r.json::<serde_json::Value>())
    {
        Ok(schema) => {
            cache()
                .lock()
                .unwrap()
                .insert(key.to_string(), schema.clone());
            tracing::info!("fetched and cached JSON schema from {key}");
            Some(schema)
        }
        Err(e) => {
            tracing::warn!("failed to fetch JSON schema from {key}: {e}");
            None
        }
    }
}

fn validate_with_cached_schema(key: &str, value: &serde_json::Value) -> Result<(), String> {
    let Some(schema) = get_cached_schema(key) else {
        return Ok(());
    };
    run_jsonschema(&schema, value)
}

fn validate_with_file_schema(path: &str, value: &serde_json::Value) -> Result<(), String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("failed to read schema {path}: {e}"))?;
    let schema: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("failed to parse schema {path}: {e}"))?;
    run_jsonschema(&schema, value)
}

/// HACK: Sanitize exec-sourced schemas (e.g. `openclaw config schema`) so they
/// work with our partial-patch validation approach:
/// - Replace `{"$ref": "#/$defs/..."}` nodes with `{}` (accept any value),
///   because openclaw emits dangling `$ref` pointers.
/// - Strip all `"required"` arrays, because we validate the full merged config
///   but only patch a subset of keys — missing required fields in parts we
///   didn't touch would fail validation.
/// Remove once openclaw ships schemas with valid `$defs` and we move to
/// patch-scoped validation.
fn strip_defs_refs(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            // If this object is a `$ref` pointing into `$defs`, replace with `{}`
            if let Some(r) = map.get("$ref").and_then(|v| v.as_str()) {
                if r.starts_with("#/$defs/") {
                    map.clear();
                    return;
                }
            }
            map.remove("$defs");
            map.remove("required");
            for v in map.values_mut() {
                strip_defs_refs(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                strip_defs_refs(v);
            }
        }
        _ => {}
    }
}

fn run_jsonschema(schema: &serde_json::Value, value: &serde_json::Value) -> Result<(), String> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|e| format!("failed to compile JSON schema: {e}"))?;
    let errors: Vec<String> = validator
        .iter_errors(value)
        .map(|e| format!("{} (at {})", e, e.instance_path()))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
