pub mod assessment;
pub mod cmd;
pub mod config;
#[cfg(feature = "services")]
pub mod config_providers;
pub mod config_watch;
#[cfg(feature = "services")]
pub mod connectors;
pub mod crash;
pub mod dashboard;
pub mod daemon;
pub mod events;
#[cfg(feature = "relay")]
pub mod file_tunnels;
pub mod host_keys;
pub mod local_client;
pub mod log_buffer;
pub mod log_layer;
pub mod logs;
#[cfg(feature = "services")]
pub mod managed_service;
pub mod mcp_servers;
pub mod metrics;
pub mod metrics_server;
pub mod nix;
pub mod notify;
pub mod os_mgmt;
#[cfg(feature = "relay")]
pub mod remote_ssh;
pub mod scripts;
#[cfg(feature = "self-update")]
pub mod self_update;
/// Re-export common sentry helpers for backwards compatibility.
pub use mac_mgmt_common::sentry_ext;
pub mod server_push;
pub mod service;
#[cfg(feature = "services")]
pub mod service_mgmt;
#[cfg(feature = "services")]
pub mod services;
#[cfg(feature = "relay")]
pub mod shell_tunnels;
pub mod skills;
pub mod status;
#[cfg(feature = "services")]
pub mod unmanaged;

/// Git commit this binary was built from. Captured at build time by
/// build.rs (GIT_SHA env or `git rev-parse HEAD`); "unknown" when
/// neither is available.
pub const GIT_SHA: &str = env!("GIT_SHA");
