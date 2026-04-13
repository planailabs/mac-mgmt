use anyhow::{Context, Result};
use std::process::Command;

use crate::managed_service::{ManagedService, TunnelDef};
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
    last_env_hash: std::cell::Cell<u64>,
}

impl Ollama {
    pub fn new(config: OllamaConfig) -> Self {
        let loaded_models = prometheus::IntGauge::new(
            "mac_mgmt_ollama_loaded_models",
            "Number of models currently loaded in ollama",
        )
        .unwrap();
        Self { config, loaded_models, last_env_hash: std::cell::Cell::new(0) }
    }

    fn env_file_hash() -> u64 {
        use std::hash::{Hash, Hasher};
        let content = std::fs::read_to_string(
            crate::config::config_dir().join("ollama-env"),
        ).unwrap_or_default();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    fn effective_host(&self) -> &str {
        if self.config.host.is_empty() { "127.0.0.1" } else { &self.config.host }
    }

    fn effective_port(&self) -> u16 {
        if self.config.port == 0 { 11434 } else { self.config.port }
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
                sentry_ext::breadcrumb("install", &format!("removing wrong flavour {wrong_pkg}"), &[
                    ("service", "ollama"),
                    ("package", &wrong_pkg),
                ]);
                crate::nix::profile_remove(&wrong_pkg)?;
            }
        }

        if crate::nix::is_installed(&pkg)? {
            tracing::info!("{pkg} is already installed");
            return Ok(());
        }

        tracing::info!("{pkg} not found, installing via nix");
        sentry_ext::breadcrumb("install", &format!("installing {pkg} via nix"), &[
            ("service", "ollama"),
            ("package", &pkg),
        ]);
        crate::nix::profile_install(&pkg, false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn spawn_spec(&self) -> crate::service_ipc::protocol::SpawnSpec {
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
        self.last_env_hash.set(Self::env_file_hash());
        crate::service_ipc::protocol::SpawnSpec {
            program: "ollama".into(),
            args: vec!["serve".into()],
            env,
        }
    }

    fn spawn(&self) -> Result<std::process::Child> {
        let spec = self.spawn_spec();
        let child = Command::new(&spec.program)
            .args(&spec.args)
            .envs(&spec.env)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("failed to start ollama serve")?;
        tracing::info!("ollama serve started (pid: {})", child.id());
        sentry_ext::breadcrumb("spawn", "ollama serve started", &[
            ("service", "ollama"),
            ("pid", &child.id().to_string()),
        ]);
        Ok(child)
    }

    fn check_health(&self) -> Result<bool> {
        // Sync fallback (used by external-process wrapper).
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.check_health_impl())
        })
    }

    fn check_health_async(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool>> + '_>> {
        Box::pin(self.check_health_impl())
    }

    fn repair(&self) -> Result<()> {
        Ok(())
    }

    fn post_start(&self) -> Result<()> {
        for model in &self.config.models {
            tracing::info!("pulling ollama model: {model}");
            sentry_ext::breadcrumb("post_start", &format!("pulling model {model}"), &[
                ("service", "ollama"),
                ("model", model),
            ]);
            let output = Command::new("ollama")
                .args(["pull", model])
                .output()
                .with_context(|| format!("failed to run ollama pull {model}"))?;

            if output.status.success() {
                tracing::info!("ollama model {model} pulled successfully");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("ollama pull {model} exited with {}", output.status);
                sentry_ext::capture_cmd_failure(
                    &format!("ollama pull {model}"),
                    output.status.code(),
                    stderr.trim(),
                );
            }
        }

        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        let pkg = pkg_for_flavour(&self.config.flavour);
        let upgradable = crate::nix::packages_with_upgrades(&[&pkg])?;

        if !upgradable.iter().any(|name| name == &pkg) {
            return Ok(false);
        }

        tracing::info!("upgrading {pkg} via nix");
        sentry_ext::breadcrumb("upgrade", &format!("upgrading {pkg} via nix"), &[
            ("service", "ollama"),
            ("package", &pkg),
        ]);
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
        let last = self.last_env_hash.get();
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
}
