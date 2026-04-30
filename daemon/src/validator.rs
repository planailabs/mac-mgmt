//! Unified config file validation.
//!
//! [`Validator`] provides format-aware syntax checking (JSON / TOML / YAML),
//! JSON Schema validation (from a URL or local file), and external command
//! validation.  It is the single validation primitive shared by the config
//! merge pipeline ([`Validator::merge_validate_and_write`]) and the relay
//! file-tunnel write path.

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
    /// JSON Schema fetched from a URL (cached globally at startup).
    Url { url: String },
    /// JSON Schema loaded from a local file.
    File { path: String },
    /// External command that exits 0 when the file is valid.
    /// `{}` in any arg is replaced with the file's absolute path.
    Command { command: Vec<String> },
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
            Some(SchemaSource::Url { url }) => validate_with_url_schema(url, value),
            Some(SchemaSource::File { path }) => validate_with_file_schema(path, value),
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
    /// 3. Validate merged value against URL/file schema (pre-write).
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

        // Pre-write validation (URL / file schemas).
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

// ── Prefetch ───────────────────────────────────────────────────────────

impl Validator {
    /// If this validator uses a URL schema, pre-fetch and cache it.
    pub async fn prefetch(&self) {
        if let Some(SchemaSource::Url { url }) = &self.schema {
            prefetch_schema(url).await;
        }
    }
}

/// Pre-fetch schemas for a set of validators (e.g. collected from all services).
pub async fn prefetch_all(validators: &[Validator]) {
    for v in validators {
        v.prefetch().await;
    }
}

// ── Pattern matching (used by file tunnels) ────────────────────────────

/// Find the first validator whose pattern matches `filename`.
pub fn find_matching<'a>(validators: &'a [Validator], path: &Path) -> Option<&'a Validator> {
    let filename = path.file_name().and_then(|n| n.to_str())?;
    validators
        .iter()
        .find(|v| glob::Pattern::new(&v.pattern).is_ok_and(|p| p.matches(filename)))
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

static SCHEMA_CACHE: OnceLock<Mutex<HashMap<String, serde_json::Value>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, serde_json::Value>> {
    SCHEMA_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

async fn prefetch_schema(url: &str) {
    tracing::info!("prefetching JSON schema from {url}");
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build();
    let result = async {
        let resp = client?
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
            tracing::warn!("failed to prefetch JSON schema from {url}: {e}");
        }
    }
}

fn get_or_fetch_schema(url: &str) -> Option<serde_json::Value> {
    {
        let c = cache().lock().unwrap();
        if let Some(schema) = c.get(url) {
            return Some(schema.clone());
        }
    }
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

fn validate_with_url_schema(url: &str, value: &serde_json::Value) -> Result<(), String> {
    let Some(schema) = get_or_fetch_schema(url) else {
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
