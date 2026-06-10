// The lib target is the daemon — it is not compiled for WASM.
// When dx builds with --features web, only main.rs (the WASM client entry) is needed.
#![cfg(not(target_arch = "wasm32"))]

#[cfg(feature = "services")]
pub mod ai_proxy;
pub mod assessment;
pub mod canary;
pub mod cmd;
pub mod config;
#[cfg(feature = "services")]
pub mod config_providers;
pub mod config_watch;
#[cfg(feature = "services")]
pub mod connectors;
pub mod crash;
pub mod daemon;
pub mod dashboard;
pub mod embed_write;
pub mod events;
#[cfg(feature = "relay")]
pub mod file_tunnels;
#[cfg(feature = "healer")]
pub mod healer_bridge;
pub mod host_keys;
pub mod local_client;
pub mod log_buffer;
pub mod log_layer;
pub mod logs;
#[cfg(feature = "services")]
pub mod managed_service;
pub mod mcp_servers;
#[cfg(feature = "memvault")]
pub mod memvault;
pub mod metrics;
pub mod metrics_server;
pub mod nix;
pub mod notify;
pub mod os_mgmt;
#[cfg(feature = "relay")]
pub mod p2p;
pub mod packages;
#[cfg(feature = "relay")]
pub mod remote_ssh;
pub mod scripts;
#[cfg(feature = "self-update")]
pub mod self_update;
/// Re-export common sentry helpers for backwards compatibility.
pub use mac_mgmt_common::sentry_ext;
pub mod secrets_cache;
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
pub mod systemctl;
#[cfg(feature = "services")]
pub mod unmanaged;
#[cfg(feature = "usb")]
pub mod usb;
#[cfg(feature = "usbd")]
pub mod usb_daemon;
#[cfg(any(feature = "usb", feature = "usbd"))]
pub mod usb_update;
pub mod validator;

/// Git commit this binary was built from. Captured at build time by
/// build.rs (GIT_SHA env or `git rev-parse HEAD`); "unknown" when
/// neither is available.
pub const GIT_SHA: &str = env!("GIT_SHA");
