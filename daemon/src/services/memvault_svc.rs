//! Memvault integrated service.
//!
//! Exposes memvault as a managed service so it participates in health checks,
//! inventory, sample collection, and tunnel exposure alongside other services.

use anyhow::Result;
use std::future::Future;
use std::pin::Pin;

use mac_mgmt_common::{InventoryEntry, InventoryValueType, MemvaultConfig};

use crate::managed_service::{ManagedService, ServiceMode, SpawnSpec, TunnelDef};

pub struct MemvaultService {
    config: MemvaultConfig,
}

impl MemvaultService {
    pub fn new(cfg: &MemvaultConfig) -> Self {
        Self {
            config: cfg.clone(),
        }
    }
}

impl ManagedService for MemvaultService {
    fn name(&self) -> &str {
        "memvault"
    }

    fn service_mode(&self) -> ServiceMode {
        ServiceMode::Integrated
    }

    // No-op — memvault runs in-process, initialized by MemvaultHandle.
    fn ensure_installed(&self) -> Result<()> {
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn spawn_spec(&self) -> SpawnSpec {
        unreachable!("memvault is an integrated service")
    }

    fn check_health(&self) -> Result<bool> {
        // Sync fallback — check that the store file exists.
        let data_dir = if self.config.data_dir.is_empty() {
            dirs::data_local_dir()
                .unwrap_or_default()
                .join("memvault")
        } else {
            std::path::PathBuf::from(&self.config.data_dir)
        };
        Ok(data_dir.join("blocks.redb").exists())
    }

    fn check_health_async(&self) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>> {
        if self.config.port == 0 {
            // No API server — just check store exists.
            let healthy = self.check_health().unwrap_or(false);
            return Box::pin(async move { Ok(healthy) });
        }

        let url = format!("http://127.0.0.1:{}/api/v1/health", self.config.port);
        Box::pin(async move {
            let resp = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(2))
                .timeout(std::time::Duration::from_secs(5))
                .build()?
                .get(&url)
                .send()
                .await?;
            Ok(resp.status().is_success())
        })
    }

    fn repair(&self) -> Result<()> {
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        Ok(false)
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        if self.config.port > 0 {
            vec![TunnelDef {
                name: "memvault".into(),
                host: "127.0.0.1".into(),
                tcp_port: self.config.port,
            }]
        } else {
            vec![]
        }
    }

    /// Static inventory: cluster ID, data dir, configuration.
    fn service_inventory(
        &self,
    ) -> Pin<Box<dyn Future<Output = Vec<InventoryEntry>> + Send + '_>> {
        Box::pin(async move {
            let mut entries = vec![
                InventoryEntry {
                    id: "cluster_id".into(),
                    name: "Cluster ID".into(),
                    value: serde_json::Value::String(self.config.cluster_id.clone()),
                    value_type: InventoryValueType::String,
                },
                InventoryEntry {
                    id: "data_dir".into(),
                    name: "Data Directory".into(),
                    value: serde_json::Value::String(self.config.data_dir.clone()),
                    value_type: InventoryValueType::String,
                },
                InventoryEntry {
                    id: "port".into(),
                    name: "API Port".into(),
                    value: serde_json::json!(self.config.port),
                    value_type: InventoryValueType::Number,
                },
                InventoryEntry {
                    id: "bootstrap_peers".into(),
                    name: "Bootstrap Peers".into(),
                    value: serde_json::json!(self.config.bootstrap_peers),
                    value_type: InventoryValueType::Json,
                },
            ];

            // Try to get store stats.
            let data_dir = if self.config.data_dir.is_empty() {
                dirs::data_local_dir().unwrap_or_default().join("memvault")
            } else {
                std::path::PathBuf::from(&self.config.data_dir)
            };
            let db_path = data_dir.join("blocks.redb");
            if let Ok(store) = memvault_store::MemvaultStore::open(&db_path) {
                // Count blocks by querying a broad time range.
                if let Ok(blocks) = store.query_by_time(0, u64::MAX, usize::MAX) {
                    entries.push(InventoryEntry {
                        id: "block_count".into(),
                        name: "Total Blocks".into(),
                        value: serde_json::json!(blocks.len()),
                        value_type: InventoryValueType::Number,
                    });
                }
            }

            // Report store file size.
            if let Ok(meta) = std::fs::metadata(&db_path) {
                entries.push(InventoryEntry {
                    id: "store_bytes".into(),
                    name: "Store Size".into(),
                    value: serde_json::json!(meta.len()),
                    value_type: InventoryValueType::Number,
                });
            }

            entries
        })
    }

    /// Dynamic sample: current store size, block count (lightweight).
    fn service_sample(
        &self,
    ) -> Pin<Box<dyn Future<Output = Vec<InventoryEntry>> + Send + '_>> {
        Box::pin(async move {
            let mut entries = Vec::new();

            let data_dir = if self.config.data_dir.is_empty() {
                dirs::data_local_dir().unwrap_or_default().join("memvault")
            } else {
                std::path::PathBuf::from(&self.config.data_dir)
            };
            let db_path = data_dir.join("blocks.redb");

            // Store file size (fast stat call).
            if let Ok(meta) = std::fs::metadata(&db_path) {
                entries.push(InventoryEntry {
                    id: "store_bytes".into(),
                    name: "Store Size".into(),
                    value: serde_json::json!(meta.len()),
                    value_type: InventoryValueType::Number,
                });
            }

            entries
        })
    }
}
