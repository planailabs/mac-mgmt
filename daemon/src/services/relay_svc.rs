use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;

use crate::managed_service::{ManagedService, ServiceMode, SpawnSpec};

/// Integrated service that reports relay connection health.
///
/// Healthy when the daemon has a fully registered RPC connection to the relay.
/// Only instantiated when a relay address is configured.
pub struct RelayService {
    registered: Arc<AtomicBool>,
}

impl RelayService {
    pub fn new(registered: Arc<AtomicBool>) -> Self {
        Self { registered }
    }
}

impl ManagedService for RelayService {
    fn name(&self) -> &str {
        "relay"
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
        unreachable!("relay is an integrated service")
    }

    fn check_health(&self) -> Result<bool> {
        Ok(self.registered.load(Ordering::Relaxed))
    }

    fn repair(&self) -> Result<()> {
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        Ok(false)
    }
}
