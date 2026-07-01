//! Hybrid HTTP-API + MCP surface for the web-agency, built on `plan-ai-api-mcp`.
//!
//! `endpoints` is compiled for both client and server (it holds the DTOs and the
//! macro-generated `#[server]` wrappers the UI calls); the registry + auth are
//! server-only.

pub mod endpoints;

#[cfg(feature = "server")]
pub mod auth;

/// Build the endpoint registry: one authenticator, every entity's CRUD.
#[cfg(feature = "server")]
pub fn build_registry(pool: sqlx::PgPool) -> plan_ai_api_mcp::Registry<sqlx::PgPool> {
    use plan_ai_api_mcp::Registry;
    use std::sync::Arc;

    let auth = Arc::new(auth::TokenAuthenticator::new(pool.clone()));
    let mut reg = Registry::new(auth)
        .info("Web Agency API", env!("CARGO_PKG_VERSION"))
        .instructions(
            "Web Agency admin API. Authenticate with a Bearer token (kind='admin' for global \
             access, or kind='api' scoped to an organization). Tools are named <entity>_<action>, \
             e.g. domain_list.",
        );

    {
        let mut d = reg.resource("domains", "domain", "Domains");
        d.list(
            "List domains visible to the caller (admins: all; else the caller's orgs').",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainListInput| async move {
                endpoints::domains::domain_list(&pool, &p, input).await
            },
        );
    }

    reg
}
