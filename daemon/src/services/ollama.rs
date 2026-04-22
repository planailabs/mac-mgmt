use anyhow::{Context, Result};

use crate::managed_service::{FileTunnelDef, ManagedService, TunnelDef};
use crate::sentry_ext;
pub use mac_mgmt_common::OllamaConfig;

const ALL_FLAVOURS: &[&str] = &["cpu", "rocm", "cuda", "vulkan"];

/// Returns the nix package name for a given flavour.
/// "cpu" maps to "ollama", others map to "ollama-{flavour}".
fn pkg_for_flavour(flavour: &str) -> String {
    if flavour == "cpu" {
        "ollama".to_string()
    } else {
        format!("ollama-{flavour}")
    }
}

/// Returns all ollama package names for flavours other than the given one.
fn other_flavour_pkgs(flavour: &str) -> Vec<String> {
    ALL_FLAVOURS
        .iter()
        .filter(|&&f| f != flavour)
        .map(|f| pkg_for_flavour(f))
        .collect()
}

pub struct Ollama {
    config: OllamaConfig,
    loaded_models: prometheus::IntGauge,
    /// Hash of the ollama-env file at last spawn, to detect changes.
    last_env_hash: std::sync::atomic::AtomicU64,
}

impl Ollama {
    pub fn new(config: OllamaConfig) -> Self {
        let loaded_models = prometheus::IntGauge::new(
            "mac_mgmt_ollama_loaded_models",
            "Number of models currently loaded in ollama",
        )
        .unwrap();
        Self {
            config,
            loaded_models,
            last_env_hash: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn env_file_hash() -> u64 {
        use std::hash::{Hash, Hasher};
        let content = std::fs::read_to_string(crate::config::config_dir().join("ollama-env"))
            .unwrap_or_default();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    fn effective_host(&self) -> &str {
        if self.config.host.is_empty() {
            "127.0.0.1"
        } else {
            &self.config.host
        }
    }

    fn effective_port(&self) -> u16 {
        if self.config.port == 0 {
            11434
        } else {
            self.config.port
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}:{}", self.effective_host(), self.effective_port())
    }

    async fn http_get(&self, path: &str) -> Result<String> {
        super::http_get(self.effective_host(), self.effective_port(), path).await
    }

    async fn check_health_impl(&self) -> Result<bool> {
        match self.http_get("/").await {
            Ok(body) if body.contains("Ollama is running") => {
                tracing::debug!("ollama is healthy at {}", self.base_url());
                Ok(true)
            }
            Ok(body) => {
                tracing::warn!("ollama unexpected response: {body}");
                Ok(false)
            }
            Err(e) => {
                tracing::warn!("ollama health check failed at {}: {e}", self.base_url());
                Ok(false)
            }
        }
    }

    fn loaded_model_count(&self) -> usize {
        // Blocking call for metrics collection (called from sync context).
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.http_get("/api/ps"))
        })
        .ok()
        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
        .and_then(|json| json.get("models")?.as_array().map(|a| a.len()))
        .unwrap_or(0)
    }
}

impl ManagedService for Ollama {
    fn name(&self) -> &str {
        "ollama"
    }

    fn ensure_installed(&self) -> Result<()> {
        let pkg = pkg_for_flavour(&self.config.flavour);

        // Remove other ollama flavours if installed
        let installed = crate::nix::installed_elements()?;
        for wrong_pkg in other_flavour_pkgs(&self.config.flavour) {
            if installed.iter().any(|name| name == &wrong_pkg) {
                tracing::info!("removing wrong ollama flavour: {wrong_pkg}");
                sentry_ext::breadcrumb(
                    "install",
                    &format!("removing wrong flavour {wrong_pkg}"),
                    &[("service", "ollama"), ("package", &wrong_pkg)],
                );
                crate::nix::profile_remove(&wrong_pkg)?;
            }
        }

        if crate::nix::is_installed(&pkg)? {
            tracing::info!("{pkg} is already installed");
            return Ok(());
        }

        tracing::info!("{pkg} not found, installing via nix");
        sentry_ext::breadcrumb(
            "install",
            &format!("installing {pkg} via nix"),
            &[("service", "ollama"), ("package", &pkg)],
        );
        crate::nix::profile_install(&pkg, false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        let mut env = std::collections::HashMap::new();
        if self.config.host != "127.0.0.1" || self.config.port != 11434 {
            env.insert(
                "OLLAMA_HOST".into(),
                format!("{}:{}", self.config.host, self.config.port),
            );
        }
        // Load extra env vars from ollama-env file (e.g. OLLAMA_ORIGINS from relay connector).
        let extra = crate::connectors::relay_ollama::load_env_file();
        for (k, v) in extra {
            env.entry(k).or_insert(v);
        }
        // Record the env file hash so we can detect changes.
        self.last_env_hash
            .store(Self::env_file_hash(), std::sync::atomic::Ordering::Relaxed);
        crate::managed_service::SpawnSpec {
            program: "ollama".into(),
            args: vec!["serve".into()],
            env,
        }
    }

    fn check_health(&self) -> Result<bool> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.check_health_impl())
        })
    }

    fn check_health_async(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool>> + Send + '_>> {
        Box::pin(self.check_health_impl())
    }

    fn repair(&self) -> Result<()> {
        Ok(())
    }

    fn post_start(&self) -> Result<()> {
        pull_configured_models(&self.config)
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        let pkg = pkg_for_flavour(&self.config.flavour);
        let upgradable = crate::nix::packages_with_upgrades(&[&pkg])?;

        if !upgradable.iter().any(|name| name == &pkg) {
            return Ok(false);
        }

        tracing::info!("upgrading {pkg} via nix");
        sentry_ext::breadcrumb(
            "upgrade",
            &format!("upgrading {pkg} via nix"),
            &[("service", "ollama"), ("package", &pkg)],
        );
        crate::nix::profile_install(&pkg, true)?;
        tracing::info!("{pkg} upgraded, restart pending");
        Ok(true)
    }

    fn is_busy(&self) -> Result<bool> {
        let count = self.loaded_model_count();
        if count > 0 {
            tracing::info!("ollama has {count} model(s) loaded");
        } else {
            tracing::debug!("ollama is idle");
        }
        Ok(count > 0)
    }

    fn needs_restart(&self) -> bool {
        let current = Self::env_file_hash();
        let last = self
            .last_env_hash
            .load(std::sync::atomic::Ordering::Relaxed);
        // Only trigger if we've spawned at least once (last != 0) and hash changed.
        last != 0 && current != last
    }

    fn metric_collectors(&self) -> Vec<Box<dyn prometheus::core::Collector>> {
        vec![Box::new(self.loaded_models.clone())]
    }

    fn collect_metrics(&self) {
        self.loaded_models.set(self.loaded_model_count() as i64);
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        vec![TunnelDef {
            name: "ollama".into(),
            host: self.effective_host().to_string(),
            tcp_port: self.effective_port(),
        }]
    }

    fn expose_shell_commands(&self) -> Vec<crate::managed_service::ShellCommandDef> {
        use crate::managed_service::{ShellArgTemplate, ShellCommandDef};
        let model_arg = || {
            Some(ShellArgTemplate {
                label: "Model name".into(),
                placeholder: "llama3.2".into(),
                validation: Some(r"^[a-zA-Z0-9._:/-]+$".into()),
            })
        };
        vec![
            ShellCommandDef {
                name: "ollama-list".into(),
                command: "ollama".into(),
                args: vec!["list".into()],
                description: "List installed models".into(),
                arg_template: None,
                timeout_secs: None,
            },
            ShellCommandDef {
                name: "ollama-ps".into(),
                command: "ollama".into(),
                args: vec!["ps".into()],
                description: "Show running models".into(),
                arg_template: None,
                timeout_secs: None,
            },
            ShellCommandDef {
                name: "ollama-pull".into(),
                command: "ollama".into(),
                args: vec!["pull".into()],
                description: "Pull a model (up to 1h timeout)".into(),
                arg_template: model_arg(),
                timeout_secs: Some(3600),
            },
            ShellCommandDef {
                name: "ollama-show".into(),
                command: "ollama".into(),
                args: vec!["show".into()],
                description: "Show model details".into(),
                arg_template: model_arg(),
                timeout_secs: None,
            },
            ShellCommandDef {
                name: "ollama-rm".into(),
                command: "ollama".into(),
                args: vec!["rm".into()],
                description: "Remove a model".into(),
                arg_template: model_arg(),
                timeout_secs: None,
            },
        ]
    }

    fn expose_files(&self) -> Vec<FileTunnelDef> {
        let env_path = crate::config::config_dir().join("ollama-env");
        vec![FileTunnelDef::File {
            name: "ollama-env".into(),
            path: env_path.to_string_lossy().into(),
            writable: true,
            description: "Ollama environment variables (key=value)".into(),
        }]
    }

    fn service_inventory(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>> {
        use mac_mgmt_common::{InventoryEntry, InventoryValueType};
        Box::pin(async move {
            let mut entries = Vec::new();

            // Version
            if let Ok(out) = crate::cmd::output_with_timeout(
                std::process::Command::new("ollama").arg("--version"),
                crate::cmd::DEFAULT_TIMEOUT,
            ) {
                if out.status.success() {
                    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !v.is_empty() {
                        entries.push(InventoryEntry {
                            id: "version".into(),
                            name: "Version".into(),
                            value: serde_json::Value::String(v),
                            value_type: InventoryValueType::String,
                        });
                    }
                }
            }

            // Installed models via /api/tags
            if let Ok(body) = self.http_get("/api/tags").await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
                        let names: Vec<serde_json::Value> = models.iter()
                            .filter_map(|m| m.get("name").cloned())
                            .collect();
                        entries.push(InventoryEntry {
                            id: "installed_models".into(),
                            name: "Installed Models".into(),
                            value: serde_json::Value::Array(names),
                            value_type: InventoryValueType::Json,
                        });
                    }
                }
            }

            entries
        })
    }

    fn service_sample(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>> {
        use mac_mgmt_common::{InventoryEntry, InventoryValueType};
        Box::pin(async move {
            let mut entries = Vec::new();

            if let Ok(body) = self.http_get("/api/ps").await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
                        let names: Vec<serde_json::Value> = models.iter()
                            .filter_map(|m| m.get("name").cloned())
                            .collect();
                        entries.push(InventoryEntry {
                            id: "loaded_models".into(),
                            name: "Loaded Models".into(),
                            value: serde_json::Value::Array(names),
                            value_type: InventoryValueType::Json,
                        });
                    }
                }
            }

            entries
        })
    }

    fn service_security(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::SecurityFinding>> + Send + '_>> {
        use mac_mgmt_common::{FindingSeverity, SecurityFinding};
        Box::pin(async move {
            let mut findings = Vec::new();

            // Check if OLLAMA_ORIGINS is set to wildcard in the env file
            let env_path = crate::config::config_dir().join("ollama-env");
            if let Ok(content) = std::fs::read_to_string(&env_path) {
                let has_wildcard = content.lines().any(|line| {
                    let line = line.trim();
                    if let Some(val) = line.strip_prefix("OLLAMA_ORIGINS=") {
                        val.trim() == "*"
                    } else {
                        false
                    }
                });
                findings.push(SecurityFinding {
                    id: "ollama_wildcard_origins".into(),
                    severity: FindingSeverity::High,
                    message: if has_wildcard {
                        "OLLAMA_ORIGINS set to wildcard (*) — any origin can access the API".into()
                    } else {
                        "OLLAMA_ORIGINS is not set to wildcard".into()
                    },
                    pass: !has_wildcard,
                });
            }

            findings
        })
    }
}

/// Pull every model listed in `config.models` via `ollama pull`.
/// Idempotent — already-pulled models are a fast no-op. Shared by
/// the managed post_start path and the unmanaged installer.
pub fn pull_configured_models(config: &mac_mgmt_common::OllamaConfig) -> anyhow::Result<()> {
    use std::process::Command;
    for model in &config.models {
        tracing::info!("pulling ollama model: {model}");
        crate::sentry_ext::breadcrumb(
            "post_start",
            &format!("pulling model {model}"),
            &[("service", "ollama"), ("model", model)],
        );
        let output = Command::new("ollama")
            .args(["pull", model])
            .output()
            .with_context(|| format!("failed to run ollama pull {model}"))?;
        if output.status.success() {
            tracing::info!("ollama model {model} pulled successfully");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("ollama pull {model} exited with {}", output.status);
            crate::sentry_ext::capture_cmd_failure(
                &format!("ollama pull {model}"),
                output.status.code(),
                stderr.trim(),
            );
        }
    }
    Ok(())
}
