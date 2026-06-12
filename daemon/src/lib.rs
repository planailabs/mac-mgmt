// The lib target is the daemon — it is not compiled for WASM.
// When dx builds with --features web, only main.rs (the WASM client entry) is needed.
#![cfg(not(target_arch = "wasm32"))]

// The agent control plane (assessment/heartbeat, supervisor + catalog, relay,
// embedded memvault, config fetch) lives in the mac-mgmt-agent crate; the
// daemon re-exports it under the old `crate::` paths so existing code (and
// the dashboard/CLI surface that stays here) compiles unchanged.
#[cfg(feature = "services")]
pub mod ai_proxy;
pub use mac_mgmt_agent::assessment;
pub use mac_mgmt_agent::canary;
pub use mac_mgmt_agent::cmd;
pub use mac_mgmt_agent::config;
#[cfg(feature = "services")]
pub use mac_mgmt_agent::config_providers;
pub mod config_watch;
#[cfg(feature = "services")]
pub use mac_mgmt_agent::connectors;
pub mod crash;
pub mod daemon;
pub mod dashboard;
pub use mac_mgmt_agent::embed_write;
pub use mac_mgmt_agent::events;
#[cfg(feature = "relay")]
pub use mac_mgmt_agent::file_tunnels;
#[cfg(feature = "healer")]
pub mod healer_bridge;
pub use mac_mgmt_agent::heartbeat;
pub use mac_mgmt_agent::host_keys;
pub use mac_mgmt_agent::local_client;
pub use mac_mgmt_agent::log_buffer;
pub use mac_mgmt_agent::log_layer;
pub mod logs;
#[cfg(feature = "services")]
pub use mac_mgmt_agent::managed_service;
pub mod mcp_servers;
#[cfg(feature = "memvault")]
pub use mac_mgmt_agent::memvault;
pub use mac_mgmt_agent::metrics;
pub mod metrics_server;
pub use mac_mgmt_agent::nix;
pub use mac_mgmt_agent::notify;
pub mod os_mgmt;
#[cfg(feature = "relay")]
pub use mac_mgmt_agent::p2p;
pub use mac_mgmt_agent::platform;
pub mod packages;
#[cfg(feature = "relay")]
pub use mac_mgmt_agent::remote_ssh;
pub mod scripts;
#[cfg(feature = "self-update")]
pub mod self_update;
/// Re-export common sentry helpers for backwards compatibility.
pub use mac_mgmt_common::sentry_ext;
pub use mac_mgmt_agent::secrets_cache;
pub use mac_mgmt_agent::server_push;
pub use mac_mgmt_agent::service;
#[cfg(feature = "services")]
pub use mac_mgmt_agent::service_mgmt;
#[cfg(feature = "services")]
pub use mac_mgmt_agent::services;
#[cfg(feature = "relay")]
pub use mac_mgmt_agent::shell_tunnels;
pub mod skills;
pub mod status;
pub mod systemctl;
#[cfg(feature = "services")]
pub use mac_mgmt_agent::unmanaged;
#[cfg(feature = "usb")]
pub mod usb;
#[cfg(feature = "usbd")]
pub mod usb_daemon;
#[cfg(feature = "usb")]
pub mod usb_update;
pub use mac_mgmt_agent::validator;

/// Git commit this binary was built from. Captured at build time by
/// build.rs (GIT_SHA env or `git rev-parse HEAD`); "unknown" when
/// neither is available.
pub const GIT_SHA: &str = env!("GIT_SHA");
