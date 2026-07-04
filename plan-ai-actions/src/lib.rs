//! Ansible-like action templates: YAML task lists executed against a
//! `plan-ai-api-mcp` registry (or anything else implementing
//! [`engine::ActionDispatcher`]).
//!
//! - `spec` / `report`: the template definition and run-result types. Always
//!   compiled, wasm-safe — they cross the client/server wire.
//! - `engine` (feature `engine`): parsing, validation, jinja rendering,
//!   step execution with loops/retries/conditions, and the durable
//!   [`manager::RunManager`] job queue.
//! - `ui` (feature `ui`): Dioxus components — parameter form, live run
//!   progress view, run report viewer.

pub mod report;
pub mod spec;

#[cfg(feature = "engine")]
pub mod engine;
#[cfg(feature = "engine")]
pub mod manager;

#[cfg(feature = "ui")]
pub mod ui;
