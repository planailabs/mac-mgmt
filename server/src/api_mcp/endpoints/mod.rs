//! API-MCP endpoint modules, one per domain. Each module holds the DTOs
//! (Input/Output structs with `schemars::JsonSchema`), the domain-logic
//! handlers (`#[api_mcp_dioxus_server]`-annotated), and thereby the generated
//! Dioxus `#[server]` wrappers the UI calls.

pub mod ai;
pub mod certificates;
pub mod clusters;
pub mod daemon_versions;
pub mod docs;
pub mod fleet;
pub mod healer;
pub mod imports;
pub mod mcp_servers;
pub mod organizations;
pub mod rollouts;
pub mod skill_centers;
pub mod skills;
pub mod tokens;
pub mod users;

/// Map an internal error to a framework 500.
#[cfg(feature = "server")]
pub(crate) fn internal<E: std::fmt::Display>(e: E) -> plan_ai_api_mcp::ApiError {
    plan_ai_api_mcp::ApiError::internal(e.to_string())
}

/// Map a failed xzar sync to a framework error.
///
/// An unconfigured xzar is an expected deployment state (no binary cache
/// wired up), not a server fault; `fetch_pins` also reports an unreachable
/// cache the same way. Both are 400s the caller can act on, so they must not
/// surface through the generic 500 path.
#[cfg(feature = "server")]
pub(crate) fn xzar_sync_error(e: String) -> plan_ai_api_mcp::ApiError {
    let actionable = e == "xzar not configured" || e.starts_with("xzar request failed");
    if actionable {
        plan_ai_api_mcp::ApiError::bad_request(e)
    } else {
        internal(e)
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    fn status(e: &str) -> u16 {
        xzar_sync_error(e.to_string()).status().as_u16()
    }

    #[test]
    fn xzar_unavailable_is_a_client_error() {
        // An unconfigured / unreachable xzar is an expected deployment state.
        assert_eq!(status("xzar not configured"), 400);
        assert_eq!(status("xzar request failed: connection refused"), 400);
        // Anything else (e.g. a malformed cache response) stays a 500.
        assert_eq!(status("failed to parse xzar response: bad json"), 500);
    }
}
