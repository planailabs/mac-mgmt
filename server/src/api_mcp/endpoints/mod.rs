//! API-MCP endpoint modules, one per domain. Each module holds the DTOs
//! (Input/Output structs with `schemars::JsonSchema`), the domain-logic
//! handlers (`#[api_mcp_dioxus_server]`-annotated), and thereby the generated
//! Dioxus `#[server]` wrappers the UI calls.

pub mod certificates;
pub mod clusters;
pub mod organizations;
pub mod tokens;
pub mod users;

/// Map an internal error to a framework 500.
#[cfg(feature = "server")]
pub(crate) fn internal<E: std::fmt::Display>(e: E) -> plan_ai_api_mcp::ApiError {
    plan_ai_api_mcp::ApiError::internal(e.to_string())
}
