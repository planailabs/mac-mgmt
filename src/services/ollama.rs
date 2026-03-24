use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::Read;
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::Duration;

use crate::managed_service::ManagedService;

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    11434
}

fn default_models() -> Vec<String> {
    vec![
        "qwen3.5".to_string(),
        "qwen3-coder-next".to_string(),
        "glm-5".to_string(),
        "kimi-k2.5".to_string(),
        "minimax-m2.7".to_string(),
    ]
}

fn default_model() -> String {
    "qwen3.5".to_string()
}

#[derive(Debug, Deserialize)]
pub struct OllamaConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_models")]
    pub models: Vec<String>,
    #[serde(default = "default_model")]
    pub default_model: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            models: default_models(),
            default_model: default_model(),
        }
    }
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

    /// Send a simple HTTP GET request and return the response body.
    /// Uses raw TCP to avoid adding an HTTP client dependency.
    fn http_get(&self, path: &str) -> Result<String> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .context("invalid ollama address")?;

        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
            .context("failed to connect to ollama")?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;

        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
            self.config.host, self.config.port
        );
        stream.write_all(request.as_bytes())?;

        let mut response = String::new();
        stream.read_to_string(&mut response)?;

        // Split headers from body
        if let Some(body) = response.split_once("\r\n\r\n").map(|(_, b)| b) {
            Ok(body.to_string())
        } else {
            Ok(response)
        }
    }
}

impl ManagedService for Ollama {
    fn name(&self) -> &str {
        "ollama"
    }

    fn ensure_installed(&self) -> Result<()> {
        if crate::nix::is_installed("ollama")? {
            tracing::info!("ollama is already installed");
            return Ok(());
        }

        tracing::info!("ollama not found, installing via nix");
        crate::nix::profile_install("ollama", false)?;
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

        let child = cmd.spawn().context("failed to start ollama serve")?;
        tracing::info!("ollama serve started (pid: {})", child.id());
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
            let status = Command::new("ollama")
                .args(["pull", model])
                .status()
                .with_context(|| format!("failed to run ollama pull {model}"))?;

            if status.success() {
                tracing::info!("ollama model {model} pulled successfully");
            } else {
                tracing::warn!("ollama pull {model} exited with {status}");
            }
        }

        let model = &self.config.default_model;
        tracing::info!("configuring ollama launch with model {model}");
        let status = Command::new("ollama")
            .args(["launch", "--yes", "--config", "--model", model, "openclaw"])
            .status()
            .with_context(|| format!("failed to run ollama launch --model {model}"))?;

        if status.success() {
            tracing::info!("ollama launch config completed successfully");
        } else {
            tracing::warn!("ollama launch exited with {status}");
        }

        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        let upgradable = crate::nix::packages_with_upgrades()?;

        if !upgradable.iter().any(|name| name == "ollama") {
            return Ok(false);
        }

        tracing::info!("upgrading ollama via nix");
        crate::nix::profile_install("ollama", true)?;
        tracing::info!("ollama upgraded, restart pending");
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
