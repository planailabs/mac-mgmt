pub mod installer;
pub mod manifest;
pub mod unit_generator;

use std::path::PathBuf;

use crate::managed_service::ManagedService;

/// How the long-running process should be daemonized after install+configure.
#[derive(Debug, Clone)]
pub enum ServiceStrategy {
    /// The software ships its own install/uninstall subcommand that creates
    /// a platform-native system service (e.g. `openclaw install`).
    BuiltInDaemon {
        install_cmd: Vec<String>,
        uninstall_cmd: Vec<String>,
    },
    /// We generate a systemd/launchd unit from the service's `spawn_spec()`.
    GeneratedUnit,
    /// Install-only — no long-running process (tools like mcporter, apprise).
    InstallOnly,
}

/// A named path the service owns — config files, data dirs, model caches.
#[derive(Debug, Clone)]
pub struct ServicePath {
    pub name: &'static str,
    pub path: PathBuf,
}

/// A service managed outside the daemon's supervisor — installed via CLI,
/// daemonized via systemd/launchd (or the software's own installer), and
/// tracked in an on-disk manifest.
pub struct UnmanagedService {
    pub svc: Box<dyn ManagedService>,
    pub strategy: ServiceStrategy,
    pub paths: Vec<ServicePath>,
}

impl UnmanagedService {
    pub fn name(&self) -> &str {
        self.svc.name()
    }
}

/// Build the set of unmanaged services from the daemon config. Calls the
/// existing `build_services()` to get the same set the daemon would
/// manage, then wraps each with strategy + detection paths.
pub fn build_unmanaged(cfg: &mut mac_mgmt_common::DaemonConfig) -> Vec<UnmanagedService> {
    use crate::connectors::build_services;

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
    let services = build_services(
        &cfg.global,
        std::mem::take(&mut cfg.openclaw),
        std::mem::take(&mut cfg.ollama),
        std::mem::take(&mut cfg.lms),
    );

    services
        .into_iter()
        .map(|svc| {
            let (strategy, paths) = match svc.name() {
                "openclaw" => (
                    ServiceStrategy::BuiltInDaemon {
                        install_cmd: vec!["openclaw".into(), "install".into()],
                        uninstall_cmd: vec!["openclaw".into(), "uninstall".into()],
                    },
                    vec![
                        ServicePath {
                            name: "config",
                            path: home.join(".openclaw/openclaw.json"),
                        },
                        ServicePath {
                            name: "data",
                            path: home.join(".openclaw"),
                        },
                    ],
                ),
                "ollama" => (
                    ServiceStrategy::GeneratedUnit,
                    vec![
                        ServicePath {
                            name: "data",
                            path: home.join(".ollama"),
                        },
                        ServicePath {
                            name: "models",
                            path: home.join(".ollama/models"),
                        },
                    ],
                ),
                "lms" => (
                    ServiceStrategy::GeneratedUnit,
                    vec![ServicePath {
                        name: "cache",
                        path: home.join(".cache/lm-studio"),
                    }],
                ),
                _ => (ServiceStrategy::InstallOnly, vec![]),
            };
            UnmanagedService {
                svc,
                strategy,
                paths,
            }
        })
        .collect()
}
