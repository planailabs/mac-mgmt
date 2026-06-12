use anyhow::Result;
use std::future::Future;
use std::pin::Pin;

use crate::managed_service::{ManagedService, ServiceMode, SpawnSpec, TunnelDef};

pub struct AiProxyService {
    host: String,
    port: u16,
}

impl AiProxyService {
    pub fn new(cfg: &mac_mgmt_common::AiProxyConfig) -> Self {
        Self {
            host: cfg.host.clone(),
            port: cfg.port,
        }
    }
}

impl ManagedService for AiProxyService {
    fn name(&self) -> &str {
        "ai-proxy"
    }

    fn service_mode(&self) -> ServiceMode {
        ServiceMode::Integrated
    }

    fn ensure_installed(&self) -> Result<()> {
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn spawn_spec(&self) -> SpawnSpec {
        unreachable!("ai-proxy is an integrated service")
    }

    fn check_health(&self) -> Result<bool> {
        // Unused — async version is the real implementation.
        Ok(false)
    }

    fn check_health_async(&self) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>> {
        let url = format!("http://{}:{}/health", self.host, self.port);
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
        vec![TunnelDef {
            name: "ai-proxy".into(),
            host: self.host.clone(),
            tcp_port: self.port,
        }]
    }
}
