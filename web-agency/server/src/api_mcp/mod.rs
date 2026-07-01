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
    use plan_ai_api_mcp::{OnItem, Registry, Risk};
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
        o.get(
            "Get an organization by id, including its members (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgGetInput| async move {
                endpoints::organizations::organization_get(&pool, &p, input).await
            },
        );
        o.update(
            "Update an organization: show_billing and/or default changedetection credential (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgUpdateInput| async move {
                endpoints::organizations::organization_update(&pool, &p, input).await
            },
        );
        // Collection verbs: the inputs carry organization_id + user selector, so
        // no `{id}` path segment to merge.
        o.custom(
            "add_member",
            Risk::Mutating,
            OnItem::No,
            "Add or re-role an organization member by user_id or email (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgMemberAddInput| async move {
                endpoints::organizations::organization_member_add(&pool, &p, input).await
            },
        );
        o.custom(
            "remove_member",
            Risk::Mutating,
            OnItem::No,
            "Remove a member from an organization (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgMemberRemoveInput| async move {
                endpoints::organizations::organization_member_remove(&pool, &p, input).await
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
        w.delete(
            "Delete a webspace folder (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::webspaces::WebspaceDeleteInput| async move {
                endpoints::webspaces::webspace_delete(&pool, &p, input).await
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
        c.get(
            "Get a credential's metadata, never the secret (org read; global credentials admin-only).",
            |pool: sqlx::PgPool, p, input: endpoints::credentials::CredentialGetInput| async move {
                endpoints::credentials::credential_get(&pool, &p, input).await
            },
        );
        c.create(
            "Create a credential (org write; global requires admin). Secret JSON is encrypted at rest.",
            |pool: sqlx::PgPool, p, input: endpoints::credentials::CredentialCreateInput| async move {
                endpoints::credentials::credential_create(&pool, &p, input).await
            },
        );
        c.update(
            "Update a credential's name/org and optionally replace its secret data.",
            |pool: sqlx::PgPool, p, input: endpoints::credentials::CredentialUpdateInput| async move {
                endpoints::credentials::credential_update(&pool, &p, input).await
            },
        );
        c.delete(
            "Delete a credential (requires credential write).",
            |pool: sqlx::PgPool, p, input: endpoints::credentials::CredentialDeleteInput| async move {
                endpoints::credentials::credential_delete(&pool, &p, input).await
            },
        );
        c.custom(
            "test",
            Risk::ReadOnly,
            OnItem::Yes,
            "Test a credential against its upstream API; returns a status string.",
            |pool: sqlx::PgPool, p, input: endpoints::credentials::CredentialTestInput| async move {
                endpoints::credentials::credential_test(&pool, &p, input).await
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
        h.get(
            "Get a webspace host with folders, domain bindings (CNAME + CF custom-domain status), and ChangeDetection assignment (requires org read).",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostGetInput| async move {
                endpoints::webspace_hosts::host_get(&pool, &p, input).await
            },
        );
        h.update(
            "Rename a webspace host (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostUpdateInput| async move {
                endpoints::webspace_hosts::host_update(&pool, &p, input).await
            },
        );
        h.delete(
            "Delete a webspace host and clean up its CNAMEs and CF custom domains (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostDeleteInput| async move {
                endpoints::webspace_hosts::host_delete(&pool, &p, input).await
            },
        );
        h.custom(
            "move",
            Risk::Mutating,
            OnItem::Yes,
            "Move a host and its folders to another organization (requires write on both orgs).",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostMoveInput| async move {
                endpoints::webspace_hosts::host_move(&pool, &p, input).await
            },
        );
        h.custom(
            "bind_domain",
            Risk::Mutating,
            OnItem::Yes,
            "Bind a domain/subdomain to a host and create its CNAME; proxy hosts get certs issued in the background (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostBindDomainInput| async move {
                endpoints::webspace_hosts::host_bind_domain(&pool, &p, input).await
            },
        );
        h.custom(
            "unbind_domain",
            Risk::Destructive,
            OnItem::Yes,
            "Remove a domain binding and its CNAME (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostUnbindDomainInput| async move {
                endpoints::webspace_hosts::host_unbind_domain(&pool, &p, input).await
            },
        );
        h.custom(
            "fix_cname",
            Risk::Mutating,
            OnItem::Yes,
            "Re-create the CNAME for a domain binding, kind-aware (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostFixCnameInput| async move {
                endpoints::webspace_hosts::host_fix_cname(&pool, &p, input).await
            },
        );
        // Mutating: a failed retry re-registers the custom domain in CF Pages.
        h.custom(
            "recheck_custom_domain",
            Risk::Mutating,
            OnItem::Yes,
            "Recheck (or re-register) a CF Pages custom domain's verification status (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostRecheckCustomDomainInput| async move {
                endpoints::webspace_hosts::host_recheck_custom_domain(&pool, &p, input).await
            },
        );
        h.custom(
            "set_changedetection",
            Risk::Mutating,
            OnItem::Yes,
            "Set or clear the host's ChangeDetection credential (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostSetChangedetectionInput| async move {
                endpoints::webspace_hosts::host_set_changedetection(&pool, &p, input).await
            },
        );
        h.custom(
            "bulk_set_changedetection",
            Risk::Mutating,
            OnItem::No,
            "Assign a ChangeDetection credential to many hosts (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostBulkSetChangedetectionInput| async move {
                endpoints::webspace_hosts::host_bulk_set_changedetection(&pool, &p, input).await
            },
        );
        h.custom(
            "bulk_clear_changedetection",
            Risk::Mutating,
            OnItem::No,
            "Clear the ChangeDetection credential on many hosts (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::webspace_hosts::HostBulkClearChangedetectionInput| async move {
                endpoints::webspace_hosts::host_bulk_clear_changedetection(&pool, &p, input).await
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
        c.create(
            "Create a domain contact (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::contacts::ContactCreateInput| async move {
                endpoints::contacts::contact_create(&pool, &p, input).await
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
        c.custom(
            "issue",
            Risk::Mutating,
            OnItem::No,
            "Issue (or reissue) the ACME certificate for a domain (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::CertIssueInput| async move {
                endpoints::certificates::certificate_issue(&pool, &p, input).await
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
        u.create(
            "Create a user (admin only). Returns the new user's id.",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserCreateInput| async move {
                endpoints::users::user_create(&pool, &p, input).await
            },
        );
        u.update(
            "Set a user's admin flag (admin only; refuses self-demotion).",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserUpdateInput| async move {
                endpoints::users::user_update(&pool, &p, input).await
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
        b.get(
            "Get a basic-auth list with its credential rows (requires org read).",
            |pool: sqlx::PgPool, p, input: endpoints::basic_auth::BasicAuthGetInput| async move {
                endpoints::basic_auth::basic_auth_get(&pool, &p, input).await
            },
        );
        b.custom(
            "add_credential",
            Risk::Mutating,
            OnItem::Yes,
            "Add or replace a username/password credential on a basic-auth list (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::basic_auth::BasicAuthAddCredentialInput| async move {
                endpoints::basic_auth::basic_auth_add_credential(&pool, &p, input).await
            },
        );
        b.custom(
            "remove_credential",
            Risk::Destructive,
            OnItem::Yes,
            "Remove a credential from a basic-auth list (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::basic_auth::BasicAuthRemoveCredentialInput| async move {
                endpoints::basic_auth::basic_auth_remove_credential(&pool, &p, input).await
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
    {
        let mut s = reg.resource("sync", "sync", "Sync");
        s.custom(
            "trigger",
            Risk::Mutating,
            OnItem::No,
            "Trigger a background provider sync (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::sync::SyncTriggerInput| async move {
                endpoints::sync::sync_trigger(&pool, &p, input).await
            },
        );
    }

    reg
}
