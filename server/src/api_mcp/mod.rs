//! Hybrid HTTP-API + MCP surface for mac-mgmt, built on `plan-ai-api-mcp`.
//!
//! Every endpoint is written once as a plain handler
//! `(pool, principal, input) -> Result<Out, ApiError>` and exposed three ways:
//! REST under `/api/v1/*` (with OpenAPI + Swagger UI), MCP tools under `/mcp`,
//! and macro-generated Dioxus `#[server]` wrappers the web UI calls.
//!
//! `endpoints` is compiled for both client and server (it holds the DTOs and
//! the `#[server]` wrappers); the registry + auth are server-only.

pub mod endpoints;

#[cfg(feature = "server")]
pub mod access;
#[cfg(feature = "server")]
pub mod auth;

#[cfg(feature = "server")]
static REGISTRY: std::sync::OnceLock<std::sync::Arc<plan_ai_api_mcp::Registry<sqlx::PgPool>>> =
    std::sync::OnceLock::new();

/// Build (once) and share the registry. Consumers beyond HTTP/MCP (e.g. the
/// chat agent's tool bridge) must dispatch from the same registry, so it
/// lives in a global instead of being rebuilt per consumer.
#[cfg(feature = "server")]
pub fn shared_registry(
    pool: sqlx::PgPool,
) -> std::sync::Arc<plan_ai_api_mcp::Registry<sqlx::PgPool>> {
    REGISTRY
        .get_or_init(|| std::sync::Arc::new(build_registry(pool)))
        .clone()
}

/// The shared registry, for handlers that dispatch other tools at runtime.
#[cfg(feature = "server")]
pub fn registry()
-> Result<std::sync::Arc<plan_ai_api_mcp::Registry<sqlx::PgPool>>, plan_ai_api_mcp::ApiError> {
    REGISTRY
        .get()
        .cloned()
        .ok_or_else(|| plan_ai_api_mcp::ApiError::internal("api-mcp registry not initialized"))
}

/// Build the endpoint registry: one authenticator, every domain's endpoints.
#[cfg(feature = "server")]
pub fn build_registry(pool: sqlx::PgPool) -> plan_ai_api_mcp::Registry<sqlx::PgPool> {
    use plan_ai_api_mcp::Registry;
    use std::sync::Arc;

    let auth = Arc::new(auth::TokenAuthenticator::new(pool.clone()));
    let reg = Registry::new(auth)
        .info("mac-mgmt API", env!("CARGO_PKG_VERSION"))
        .instructions(
            "mac-mgmt fleet management API. Authenticate with a Bearer token \
             (an admin token, or an org-scoped api token). Tools are named \
             <resource>_<action>; list/get are read-only, create/update mutate, \
             delete is destructive.",
        );

    // Domain endpoints are registered here as they are converted from the
    // legacy dioxus #[server] functions (one module per domain under
    // `endpoints/`).
    let mut reg = reg;

    {
        let mut c = reg.resource("clusters", "cluster", "Clusters");
        c.list(
            "List clusters visible to the caller (admins: all; else clusters of the caller's orgs).",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::ClusterListInput| async move {
                endpoints::clusters::cluster_list(&pool, &p, input).await
            },
        );
    }

    reg
}

#[cfg(all(test, feature = "server"))]
mod tests {
    /// The generated OpenAPI document must parse into utoipa's typed model —
    /// `http_router` panics otherwise, so catch it at test time.
    #[tokio::test]
    async fn openapi_document_parses_into_utoipa() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/unused")
            .expect("lazy pool");
        let reg = super::build_registry(pool);
        let doc = reg.openapi_json();
        let parsed: Result<utoipa::openapi::OpenApi, _> = serde_json::from_value(doc);
        assert!(parsed.is_ok(), "openapi must parse: {:?}", parsed.err());
    }
}
