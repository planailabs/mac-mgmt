use anyhow::{Context, Result};
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::Duration;

use crate::managed_service::ManagedService;

pub struct Ollama {
    pub host: String,
    pub port: u16,
}

impl Default for Ollama {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 11434,
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
        crate::nix::profile_install("nixpkgs#ollama", false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn spawn(&self) -> Result<std::process::Child> {
        let mut cmd = Command::new("ollama");
        cmd.arg("serve");

        if self.host != "127.0.0.1" || self.port != 11434 {
            cmd.env("OLLAMA_HOST", format!("{}:{}", self.host, self.port));
        }

        let child = cmd.spawn().context("failed to start ollama serve")?;
        tracing::info!("ollama serve started (pid: {})", child.id());
        Ok(child)
    }

    fn check_health(&self) -> Result<bool> {
        let addr: SocketAddr = format!("{}:{}", self.host, self.port)
            .parse()
            .context("invalid ollama address")?;

        match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
            Ok(_) => {
                tracing::debug!("ollama is reachable at {addr}");
                Ok(true)
            }
            Err(e) => {
                tracing::warn!("ollama is not reachable at {addr}: {e}");
                Ok(false)
            }
        }
    }

    fn repair(&self) -> Result<()> {
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        let upgradable = crate::nix::packages_with_upgrades()?;

        if !upgradable.iter().any(|name| name == "ollama") {
            return Ok(false);
        }

        tracing::info!("upgrading ollama via nix");
        crate::nix::profile_install("nixpkgs#ollama", true)?;
        tracing::info!("ollama upgraded, restart pending");
        Ok(true)
    }
}
