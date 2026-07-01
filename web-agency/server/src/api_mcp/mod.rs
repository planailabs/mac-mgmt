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
    {
        let mut o = reg.resource("organizations", "organization", "Organizations");
        o.list(
            "List all organizations (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgListInput| async move {
                endpoints::organizations::organization_list(&pool, &p, input).await
            },
        );
        o.create(
            "Create an organization (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgCreateInput| async move {
                endpoints::organizations::organization_create(&pool, &p, input).await
            },
        );
    }
    {
        let mut w = reg.resource("webspaces", "webspace", "Webspaces");
        w.list(
            "List webspace folders visible to the caller.",
            |pool: sqlx::PgPool, p, input: endpoints::webspaces::WebspaceListInput| async move {
                endpoints::webspaces::webspace_list(&pool, &p, input).await
            },
        );
    }
    {
        let mut c = reg.resource("credentials", "credential", "Credentials");
        c.list(
            "List credentials visible to the caller (admins: all; else org + global).",
            |pool: sqlx::PgPool, p, input: endpoints::credentials::CredentialListInput| async move {
                endpoints::credentials::credential_list(&pool, &p, input).await
            },
        );
    }
    {
        let mut h = reg.resource("webspace-hosts", "webspace_host", "Webspace Hosts");
        h.list(
            "List webspace hosts visible to the caller.",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostListInput| async move {
                endpoints::webspace_hosts::host_list(&pool, &p, input).await
            },
        );
        h.create(
            "Create a webspace host (requires org write; cloudflare kind provisions a Pages project).",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostCreateInput| async move {
                endpoints::webspace_hosts::host_create(&pool, &p, input).await
            },
        );
    }
    {
        let mut c = reg.resource("contacts", "contact", "Contacts");
        c.list(
            "List domain contacts visible to the caller.",
            |pool: sqlx::PgPool, p, input: endpoints::contacts::ContactListInput| async move {
                endpoints::contacts::contact_list(&pool, &p, input).await
            },
        );
    }
    {
        let mut c = reg.resource("certificates", "certificate", "Certificates");
        c.list(
            "List TLS certificates (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::CertListInput| async move {
                endpoints::certificates::certificate_list(&pool, &p, input).await
            },
        );
    }
    {
        let mut u = reg.resource("users", "user", "Users");
        u.list(
            "List all users (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserListInput| async move {
                endpoints::users::user_list(&pool, &p, input).await
            },
        );
        u.get(
            "Get a user by id (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserGetInput| async move {
                endpoints::users::user_get(&pool, &p, input).await
            },
        );
        u.delete(
            "Delete a user (admin only; refuses self-deletion).",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserDeleteInput| async move {
                endpoints::users::user_delete(&pool, &p, input).await
            },
        );
    }
    {
        let mut b = reg.resource("basic-auth-lists", "basic_auth_list", "Basic Auth");
        b.list(
            "List basic-auth lists visible to the caller.",
            |pool: sqlx::PgPool, p, input: endpoints::basic_auth::BasicAuthListInput| async move {
                endpoints::basic_auth::basic_auth_list(&pool, &p, input).await
            },
        );
        b.create(
            "Create a basic-auth list in an organization (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::basic_auth::BasicAuthCreateInput| async move {
                endpoints::basic_auth::basic_auth_create(&pool, &p, input).await
            },
        );
        b.delete(
            "Delete a basic-auth list (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::basic_auth::BasicAuthDeleteInput| async move {
                endpoints::basic_auth::basic_auth_delete(&pool, &p, input).await
            },
        );
    }
    {
        let mut b = reg.resource("billing", "billing", "Billing");
        b.list(
            "List billing entries visible to the caller.",
            |pool: sqlx::PgPool, p, input: endpoints::billing::BillingListInput| async move {
                endpoints::billing::billing_list(&pool, &p, input).await
            },
        );
    }

    reg
}
