use anyhow::Result;
use std::collections::HashMap;

use super::{Connector, ConnectorPhase};

/// Writes `~/.config/mac-mgmt/restic-includes.txt` with all backup-worthy
/// paths from managed services + any extra paths from the backup config.
/// The daemon's backup tick reads this file when invoking `restic backup`.
pub struct BackupConnector;

impl Connector for BackupConnector {
    fn name(&self) -> &str {
        "backup"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["backup", "backup_paths"]
    }

    fn connect(
        &self,
        configs: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let paths = configs
            .get("backup_paths")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut lines: Vec<String> = paths
            .iter()
            .filter_map(|entry| entry.get("path").and_then(|p| p.as_str()).map(String::from))
            .collect();

        // Add extra_paths from the backup config.
        if let Some(backup_val) = configs.get("backup") {
            if let Some(extra) = backup_val.get("extra_paths").and_then(|v| v.as_array()) {
                for p in extra {
                    if let Some(s) = p.as_str() {
                        lines.push(s.to_string());
                    }
                }
            }
        }

        // Deduplicate and filter to existing paths.
        lines.sort();
        lines.dedup();
        let existing: Vec<&str> = lines
            .iter()
            .filter(|p| std::path::Path::new(p.as_str()).exists())
            .map(|s| s.as_str())
            .collect();

        let includes_path = crate::config::config_dir().join("restic-includes.txt");
        std::fs::write(&includes_path, existing.join("\n"))
            .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", includes_path.display()))?;

        tracing::info!(
            "backup connector: wrote {} paths to {}",
            existing.len(),
            includes_path.display()
        );

        Ok(())
    }
}
