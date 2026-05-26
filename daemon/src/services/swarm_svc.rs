use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;

use crate::managed_service::{ManagedService, ServiceMode, SpawnSpec};

/// Integrated service that reports the libp2p swarm health.
///
/// Healthy when at least one QUIC listener is active.
pub struct SwarmService {
    listening: Arc<AtomicBool>,
}

impl SwarmService {
    pub fn new(listening: Arc<AtomicBool>) -> Self {
        Self { listening }
    }
}

impl ManagedService for SwarmService {
    fn name(&self) -> &str {
        "swarm"
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
        unreachable!("swarm is an integrated service")
    }

    fn check_health(&self) -> Result<bool> {
        Ok(self.listening.load(Ordering::Relaxed))
    }

    fn repair(&self) -> Result<()> {
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        Ok(false)
    }
}
