use anyhow::{Context, Result};
use std::process::Command;

use crate::managed_service::ManagedService;
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
}

impl Ollama {
    pub fn new(config: OllamaConfig) -> Self {
        Self { config }
    }

    fn base_url(&self) -> String {
        format!("http://{}:{}", self.config.host, self.config.port)
    }

    fn http_get(&self, path: &str) -> Result<String> {
        super::http_get(&self.config.host, self.config.port, path)
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

    fn spawn(&self) -> Result<std::process::Child> {
        let mut cmd = Command::new("ollama");
        cmd.arg("serve");

        if self.config.host != "127.0.0.1" || self.config.port != 11434 {
            cmd.env(
                "OLLAMA_HOST",
                format!("{}:{}", self.config.host, self.config.port),
            );
        }

        let child = cmd
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
        match self.http_get("/") {
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
        let body = self.http_get("/api/ps")?;
        let json: serde_json::Value =
            serde_json::from_str(&body).context("failed to parse ollama /api/ps")?;

        let busy = json
            .get("models")
            .and_then(|m| m.as_array())
            .is_some_and(|models| !models.is_empty());

        if busy {
            tracing::info!("ollama has models loaded");
        } else {
            tracing::debug!("ollama is idle");
        }

        Ok(busy)
    }
}
