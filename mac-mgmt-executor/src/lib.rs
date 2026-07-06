//! Ephemeral Incus container orchestration.
//!
//! Exposes the Incus backend trait and its Unix-socket / HTTPS implementations
//! as a library so other crates (e.g. the mmrc orchestration daemon) can drive
//! Incus directly, in addition to the MCP-server binary in `main.rs`.

pub mod backend;
pub mod incus_common;
pub mod incus_https;
pub mod incus_unix;
pub mod server;
pub mod state;
pub mod types;

pub use backend::IncusBackend;
pub use incus_https::HttpsBackend;
pub use incus_unix::UnixBackend;
pub use types::LaunchSpec;
