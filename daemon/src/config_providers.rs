//! Config provider system: a reactive store of named config values.
//!
//! Config providers are named sources of configuration (e.g. "relay", "ollama").
//! Connectors declare which providers they depend on. When any dependency
//! changes, the connector is re-triggered.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A single config provider entry with change tracking.
#[derive(Clone, Serialize, Deserialize)]
struct ProviderEntry {
    value: serde_json::Value,
    /// Monotonic version counter; incremented on each update.
    #[serde(skip)]
    version: u64,
}

/// Tracks which provider versions a connector last ran with.
#[derive(Default)]
pub struct ConnectorSnapshot {
    versions: HashMap<String, u64>,
}

/// The config store that holds all providers and tracks changes.
pub struct ConfigStore {
    providers: HashMap<String, ProviderEntry>,
    /// Path to persist provider state on disk (for relay config, etc.).
    cache_path: Option<PathBuf>,
}

/// On-disk cache format.
#[derive(Serialize, Deserialize, Default)]
struct CacheFile {
    providers: HashMap<String, serde_json::Value>,
}

impl ConfigStore {
    pub fn new(cache_path: Option<PathBuf>) -> Self {
        let mut store = Self {
            providers: HashMap::new(),
            cache_path,
        };
        store.load_cache();
        store
    }

    /// Set a config provider value. Returns true if the value actually changed.
    pub fn set(&mut self, name: &str, value: serde_json::Value) -> bool {
        if let Some(entry) = self.providers.get(name) {
            if entry.value == value {
                return false;
            }
        }
        let version = self
            .providers
            .get(name)
            .map(|e| e.version + 1)
            .unwrap_or(1);
        self.providers.insert(
            name.to_string(),
            ProviderEntry { value, version },
        );
        self.save_cache();
        true
    }

    /// Get a provider value by name.
    pub fn get(&self, name: &str) -> Option<&serde_json::Value> {
        self.providers.get(name).map(|e| &e.value)
    }

    /// Check if all named providers are available.
    pub fn all_available(&self, names: &[&str]) -> bool {
        names.iter().all(|n| self.providers.contains_key(*n))
    }

    /// Check if any provider in `names` has changed since `snapshot`.
    /// Also returns true if the snapshot is empty (first run).
    pub fn any_changed(&self, names: &[&str], snapshot: &ConnectorSnapshot) -> bool {
        for name in names {
            let current = self.providers.get(*name).map(|e| e.version).unwrap_or(0);
            let seen = snapshot.versions.get(*name).copied().unwrap_or(0);
            if current != seen {
                return true;
            }
        }
        false
    }

    /// Record the current versions for the given provider names.
    pub fn snapshot(&self, names: &[&str]) -> ConnectorSnapshot {
        let mut versions = HashMap::new();
        for name in names {
            if let Some(entry) = self.providers.get(*name) {
                versions.insert(name.to_string(), entry.version);
            }
        }
        ConnectorSnapshot { versions }
    }

    /// Build a map of provider name → value for the given names.
    pub fn values_for(&self, names: &[&str]) -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();
        for name in names {
            if let Some(entry) = self.providers.get(*name) {
                map.insert(name.to_string(), entry.value.clone());
            }
        }
        map
    }

    /// Load cached providers from disk.
    fn load_cache(&mut self) {
        let Some(ref path) = self.cache_path else { return };
        let Ok(contents) = std::fs::read_to_string(path) else { return };
        let Ok(cache) = serde_json::from_str::<CacheFile>(&contents) else { return };
        for (name, value) in cache.providers {
            // Don't overwrite providers already set (runtime values take priority).
            if !self.providers.contains_key(&name) {
                self.providers.insert(
                    name.clone(),
                    ProviderEntry { value, version: 1 },
                );
                tracing::info!("restored config provider '{name}' from cache");
            }
        }
    }

    /// Save all providers to disk cache.
    fn save_cache(&self) {
        let Some(ref path) = self.cache_path else { return };
        let cache = CacheFile {
            providers: self
                .providers
                .iter()
                .map(|(k, v)| (k.clone(), v.value.clone()))
                .collect(),
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(&cache) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    tracing::warn!("failed to save config cache to {}: {e}", path.display());
                }
            }
            Err(e) => tracing::warn!("failed to serialize config cache: {e}"),
        }
    }
}
