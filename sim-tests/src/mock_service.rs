//! Mock ManagedService implementations for simulation testing.
//!
//! These stubs implement the ManagedService trait without calling real nix
//! commands or spawning real processes. They expose configurable file tunnels
//! and shell commands for relay integration testing.

use anyhow::Result;
use mac_mgmt_daemon::managed_service::*;
use mac_mgmt_services::SpawnSpec;
use std::future::Future;
use std::pin::Pin;

/// A mock service that does nothing but expose tunnels and report healthy.
pub struct MockManagedService {
    svc_name: String,
    tunnels: Vec<TunnelDef>,
    file_tunnels: Vec<FileTunnelDef>,
    shell_commands: Vec<ShellCommandDef>,
    healthy: std::sync::atomic::AtomicBool,
}

impl MockManagedService {
    pub fn new(name: &str) -> Self {
        Self {
            svc_name: name.to_string(),
            tunnels: Vec::new(),
            file_tunnels: Vec::new(),
            shell_commands: Vec::new(),
            healthy: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// Add a TCP tunnel definition.
    pub fn with_tunnel(mut self, name: &str, host: &str, port: u16) -> Self {
        self.tunnels.push(TunnelDef {
            name: name.to_string(),
            host: host.to_string(),
            tcp_port: port,
        });
        self
    }

    /// Add a file tunnel definition.
    pub fn with_file_tunnel(mut self, name: &str, path: &str, writable: bool) -> Self {
        self.file_tunnels.push(FileTunnelDef::File {
            name: name.to_string(),
            path: path.to_string(),
            writable,
            description: format!("Mock file tunnel: {name}"),
        });
        self
    }

    /// Add a shell command definition.
    pub fn with_shell_command(mut self, name: &str, command: &str, args: &[&str]) -> Self {
        self.shell_commands.push(ShellCommandDef {
            name: name.to_string(),
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            description: format!("Mock command: {name}"),
            arg_template: None,
        });
        self
    }

    /// Add a shell command with an argument template.
    pub fn with_shell_command_with_arg(
        mut self,
        name: &str,
        command: &str,
        args: &[&str],
        label: &str,
        placeholder: &str,
    ) -> Self {
        self.shell_commands.push(ShellCommandDef {
            name: name.to_string(),
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            description: format!("Mock command: {name}"),
            arg_template: Some(ShellArgTemplate {
                label: label.to_string(),
                placeholder: placeholder.to_string(),
                validation: None,
            }),
        });
        self
    }

    /// Set healthy/unhealthy state.
    pub fn set_healthy(&self, healthy: bool) {
        self.healthy
            .store(healthy, std::sync::atomic::Ordering::Relaxed);
    }
}

impl ManagedService for MockManagedService {
    fn name(&self) -> &str {
        &self.svc_name
    }

    fn ensure_installed(&self) -> Result<()> {
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn spawn_spec(&self) -> SpawnSpec {
        // Spawn `sleep infinity` — a harmless long-running process
        SpawnSpec {
            program: "sleep".into(),
            args: vec!["infinity".into()],
            env: std::collections::HashMap::new(),
        }
    }

    fn check_health(&self) -> Result<bool> {
        Ok(self.healthy.load(std::sync::atomic::Ordering::Relaxed))
    }

    fn check_health_async(&self) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>> {
        Box::pin(std::future::ready(self.check_health()))
    }

    fn repair(&self) -> Result<()> {
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        Ok(false)
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        self.tunnels.clone()
    }

    fn expose_files(&self) -> Vec<FileTunnelDef> {
        self.file_tunnels.clone()
    }

    fn expose_shell_commands(&self) -> Vec<ShellCommandDef> {
        self.shell_commands.clone()
    }
}
