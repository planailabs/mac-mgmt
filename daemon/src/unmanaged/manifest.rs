use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// On-disk record of what the unmanaged installer has touched.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstallManifest {
    pub services: HashMap<String, ServiceState>,
    #[serde(default)]
    pub connectors_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    pub package_installed: bool,
    pub configured: bool,
    pub models_pulled: bool,
    pub service_active: bool,
    /// SHA-256 of the last-written unit file contents. Used to detect
    /// when spawn_spec() output changed so the unit can be rewritten
    /// and the service restarted.
    #[serde(default)]
    pub unit_hash: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    pub installed_at: DateTime<Utc>,
}

impl ServiceState {
    pub fn new() -> Self {
        Self {
            package_installed: false,
            configured: false,
            models_pulled: false,
            service_active: false,
            unit_hash: None,
            last_error: None,
            installed_at: Utc::now(),
        }
    }
}

impl InstallManifest {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("/root/.config"))
            .join("mac-mgmt/install-state.json")
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&s).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let s = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, &s)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn get_or_create(&mut self, name: &str) -> &mut ServiceState {
        self.services
            .entry(name.to_string())
            .or_insert_with(ServiceState::new)
    }
}
