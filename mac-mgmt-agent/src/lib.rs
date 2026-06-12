//! The reusable agent control plane, extracted from the daemon crate.
//!
//! Holds the machinery shared by the full mac-mgmt daemon and the plan-ai-usb
//! `usbd` control plane: host assessment + heartbeat, the service supervisor
//! and catalog, the libp2p relay/tunnel stack, config fetch/merge and the
//! embedded-memvault wrapper. Feature flags mirror the daemon's
//! (`services` / `relay` / `memvault`); the daemon re-exports these modules
//! under their old `crate::` paths.

pub mod assessment;
pub mod canary;
pub mod cmd;
pub mod config;
#[cfg(feature = "services")]
pub mod config_providers;
#[cfg(feature = "services")]
pub mod connectors;
pub mod embed_write;
pub mod events;
#[cfg(feature = "relay")]
pub mod file_tunnels;
pub mod heartbeat;
pub mod host_keys;
pub mod local_client;
pub mod log_buffer;
pub mod log_layer;
#[cfg(feature = "services")]
pub mod managed_service;
#[cfg(feature = "memvault")]
pub mod memvault;
pub mod metrics;
pub mod nix;
pub mod notify;
#[cfg(feature = "relay")]
pub mod p2p;
pub mod platform;
#[cfg(feature = "relay")]
pub mod remote_ssh;
pub mod secrets_cache;
pub mod server_push;
pub mod service;
#[cfg(feature = "services")]
pub mod service_mgmt;
#[cfg(feature = "services")]
pub mod services;
#[cfg(feature = "relay")]
pub mod shell_tunnels;
#[cfg(feature = "services")]
pub mod unmanaged;
pub mod validator;

/// Re-export common sentry helpers (modules here use `crate::sentry_ext`).
pub use mac_mgmt_common::sentry_ext;

// Re-export the foreign types that appear in this crate's public API, so
// consumers (the daemon, plan-ai-usb's usbd) don't have to pin matching
// versions of these crates themselves.
pub use libp2p;
pub use russh;
