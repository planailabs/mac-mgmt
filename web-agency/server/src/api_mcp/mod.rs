//! Hybrid HTTP-API + MCP surface for the web-agency, built on `plan-ai-api-mcp`.
//!
//! `endpoints` is compiled for both client and server (it holds the DTOs and the
//! macro-generated `#[server]` wrappers the UI calls); the registry + auth are
//! server-only.

pub mod endpoints;

#[cfg(feature = "server")]
pub mod auth;

#[cfg(feature = "server")]
static REGISTRY: std::sync::OnceLock<std::sync::Arc<plan_ai_api_mcp::Registry<sqlx::PgPool>>> =
    std::sync::OnceLock::new();

#[cfg(feature = "server")]
static RUN_MANAGER: std::sync::OnceLock<std::sync::Arc<plan_ai_actions::manager::RunManager>> =
    std::sync::OnceLock::new();

/// Build (once) and share the registry. The action-template executor needs to
/// dispatch tools from the same registry that serves HTTP/MCP, so it lives in
/// a global instead of being rebuilt per consumer.
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

/// The durable action-template run queue.
#[cfg(feature = "server")]
pub fn run_manager()
-> Result<std::sync::Arc<plan_ai_actions::manager::RunManager>, plan_ai_api_mcp::ApiError> {
    RUN_MANAGER
        .get()
        .cloned()
        .ok_or_else(|| plan_ai_api_mcp::ApiError::internal("action-template queue not initialized"))
}

/// Baked-template tool-name validation (the half build.rs can't do) plus the
/// run queue: requeue runs interrupted by the previous shutdown and start the
/// workers. Panics on an invalid template — a template referencing an unknown
/// tool must fail the boot, not the first run.
#[cfg(feature = "server")]
pub async fn init_action_runtime(pool: sqlx::PgPool) {
    use plan_ai_actions::engine::{ActionDispatcher, BuiltinRegistry, validate_template};
    use std::sync::Arc;

    let registry = shared_registry(pool.clone());
    let builtins = BuiltinRegistry::standard();
    let tools: Vec<String> = registry
        .tool_names()
        .into_iter()
        .filter(|t| !t.starts_with("action_template_"))
        .collect();

    let mut errors = Vec::new();
    for (name, spec) in endpoints::action_templates::baked_templates() {
        if let Err(errs) = validate_template(spec, Some(&tools), &builtins) {
            errors.push(format!("template '{name}': {}", errs.join("; ")));
        }
        for (input, ispec) in &spec.inputs {
            if let Some(resource) = &ispec.reference {
                if registry.list_tool_for_resource(resource).is_none() {
                    errors.push(format!(
                        "template '{name}': input '{input}' ref '{resource}' has no list endpoint"
                    ));
                }
            }
        }
    }
    if !errors.is_empty() {
        panic!("invalid action templates:\n{}", errors.join("\n"));
    }

    // ponytail: single-server queue — blanket requeue of 'running' rows at
    // boot; multi-instance would need claim leases instead.
    if let Err(e) =
        sqlx::query("UPDATE action_template_runs SET status = 'queued' WHERE status = 'running'")
            .execute(&pool)
            .await
    {
        tracing::error!("failed to requeue interrupted action-template runs: {e}");
    }

    let store = Arc::new(endpoints::action_templates::PgRunStore { pool: pool.clone() });
    let factory_pool = pool.clone();
    let manager = plan_ai_actions::manager::RunManager::new(
        store,
        Arc::new(move |principal: &serde_json::Value| {
            let principal: plan_ai_api_mcp::Principal = serde_json::from_value(principal.clone())
                .map_err(|e| {
                    plan_ai_actions::engine::EngineError::Store(format!("bad principal: {e}"))
                })?;
            Ok(Arc::new(endpoints::action_templates::RegistryDispatcher {
                pool: factory_pool.clone(),
                principal: Arc::new(principal),
            }) as Arc<dyn ActionDispatcher>)
        }),
        Arc::new(|name: &str| {
            endpoints::action_templates::baked_templates()
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, spec)| spec.clone())
        }),
        builtins,
    );
    manager.spawn_workers(2);
    let _ = RUN_MANAGER.set(manager);
}

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
        d.get(
            "Get a domain with subdomains, DNS records and live Cloudflare zone info (requires org read).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainGetInput| async move {
                endpoints::domains::domain_get(&pool, &p, input).await
            },
        );
        d.create(
            "Add a domain (requires org write); creates/finds a Cloudflare zone when a credential is given.",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainCreateInput| async move {
                endpoints::domains::domain_create(&pool, &p, input).await
            },
        );
        d.update(
            "Update a domain's ssl_mode, dnssec_enabled and/or ai_bots_protection (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainUpdateInput| async move {
                endpoints::domains::domain_update(&pool, &p, input).await
            },
        );
        d.delete(
            "Delete a domain and all its records (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainDeleteInput| async move {
                endpoints::domains::domain_delete(&pool, &p, input).await
            },
        );
        d.custom(
            "move",
            Risk::Mutating,
            OnItem::Yes,
            "Move a domain to another organization (requires write on both orgs).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainMoveInput| async move {
                endpoints::domains::domain_move(&pool, &p, input).await
            },
        );
        d.custom(
            "deploy_cloudflare",
            Risk::Mutating,
            OnItem::Yes,
            "Deploy a domain to Cloudflare and best-effort set registrar nameservers (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainDeployCloudflareInput| async move {
                endpoints::domains::domain_deploy_cloudflare(&pool, &p, input).await
            },
        );
        d.custom(
            "set_nameservers",
            Risk::Mutating,
            OnItem::Yes,
            "Set the given nameservers at the domain's registrar (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainSetNameserversInput| async move {
                endpoints::domains::domain_set_nameservers(&pool, &p, input).await
            },
        );
        d.custom(
            "sync_records",
            Risk::Mutating,
            OnItem::Yes,
            "Replace local DNS records with the domain's Cloudflare zone records (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainSyncRecordsInput| async move {
                endpoints::domains::domain_sync_records(&pool, &p, input).await
            },
        );
        d.custom(
            "check_availability",
            Risk::ReadOnly,
            OnItem::No,
            "Check a domain's availability and registration pricing via a registrar credential (credential org read; global credentials admin-only).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainCheckAvailabilityInput| async move {
                endpoints::domains::domain_check_availability(&pool, &p, input).await
            },
        );
        d.custom(
            "discover",
            Risk::ReadOnly,
            OnItem::No,
            "Discover importable domains on a credential (Cloudflare zones or Spaceship domains), flagging ones already in the organization (requires org read).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainDiscoverInput| async move {
                endpoints::domains::domain_discover(&pool, &p, input).await
            },
        );
        d.custom(
            "import",
            Risk::Mutating,
            OnItem::No,
            "Import domains from a credential into an organization; existing names are skipped, per-domain failures collected (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainImportInput| async move {
                endpoints::domains::domain_import(&pool, &p, input).await
            },
        );
        d.custom(
            "bulk_deploy_cloudflare",
            Risk::Mutating,
            OnItem::No,
            "Deploy multiple domains to Cloudflare under one credential (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainBulkDeployCloudflareInput| async move {
                endpoints::domains::domain_bulk_deploy_cloudflare(&pool, &p, input).await
            },
        );
        d.custom(
            "bulk_set_nameservers",
            Risk::Mutating,
            OnItem::No,
            "Set Cloudflare nameservers at the registrar for multiple domains (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainBulkSetNameserversInput| async move {
                endpoints::domains::domain_bulk_set_nameservers(&pool, &p, input).await
            },
        );
        d.custom(
            "bulk_set_ssl_mode",
            Risk::Mutating,
            OnItem::No,
            "Set the SSL mode for multiple Cloudflare-deployed domains (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainBulkSetSslModeInput| async move {
                endpoints::domains::domain_bulk_set_ssl_mode(&pool, &p, input).await
            },
        );
        d.custom(
            "bulk_set_ai_bots",
            Risk::Mutating,
            OnItem::No,
            "Set AI bot protection for multiple Cloudflare-deployed domains (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainBulkSetAiBotsInput| async move {
                endpoints::domains::domain_bulk_set_ai_bots(&pool, &p, input).await
            },
        );
        d.custom(
            "bulk_create_pages",
            Risk::Mutating,
            OnItem::No,
            "Create Cloudflare Pages projects + hosts for multiple domains (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DomainBulkCreatePagesInput| async move {
                endpoints::domains::domain_bulk_create_pages(&pool, &p, input).await
            },
        );
    }
    {
        let mut s = reg.resource("subdomains", "subdomain", "Domains");
        s.create(
            "Create a subdomain on a domain (requires org write); upserts on (domain, name).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::SubdomainCreateInput| async move {
                endpoints::domains::subdomain_create(&pool, &p, input).await
            },
        );
        s.delete(
            "Delete a subdomain and its DNS records, incl. Cloudflare cleanup (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::SubdomainDeleteInput| async move {
                endpoints::domains::subdomain_delete(&pool, &p, input).await
            },
        );
    }
    {
        let mut r = reg.resource("dns-records", "dns_record", "Domains");
        r.create(
            "Add a DNS record to a subdomain, also in Cloudflare when deployed (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DnsRecordCreateInput| async move {
                endpoints::domains::dns_record_create(&pool, &p, input).await
            },
        );
        r.delete(
            "Delete a DNS record, incl. best-effort Cloudflare delete (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::domains::DnsRecordDeleteInput| async move {
                endpoints::domains::dns_record_delete(&pool, &p, input).await
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
        w.get(
            "Get a webspace folder: host info, auth settings and live CF Pages details (requires org read).",
            |pool: sqlx::PgPool, p, input: endpoints::webspaces::WebspaceGetInput| async move {
                endpoints::webspaces::webspace_get(&pool, &p, input).await
            },
        );
        w.create(
            "Create a webspace folder on a proxy host (requires org write). Returns the new folder id.",
            |pool: sqlx::PgPool, p, input: endpoints::webspaces::WebspaceCreateInput| async move {
                endpoints::webspaces::webspace_create(&pool, &p, input).await
            },
        );
        w.update(
            "Update a webspace folder: name, mount path, upstream URL and/or auth mode + basic-auth list; only provided fields change (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::webspaces::WebspaceUpdateInput| async move {
                endpoints::webspaces::webspace_update(&pool, &p, input).await
            },
        );
        w.custom(
            "move",
            Risk::Mutating,
            OnItem::Yes,
            "Move a webspace folder to another organization (requires write on both orgs).",
            |pool: sqlx::PgPool, p, input: endpoints::webspaces::WebspaceMoveInput| async move {
                endpoints::webspaces::webspace_move(&pool, &p, input).await
            },
        );
        w.custom(
            "deploy_pages",
            Risk::Mutating,
            OnItem::Yes,
            "Create (or link an existing) Cloudflare Pages project for this webspace (requires org write). Returns a status string.",
            |pool: sqlx::PgPool, p, input: endpoints::webspaces::WebspaceDeployPagesInput| async move {
                endpoints::webspaces::webspace_deploy_pages(&pool, &p, input).await
            },
        );
        w.custom(
            "connect_git",
            Risk::Mutating,
            OnItem::Yes,
            "Connect a GitHub/GitLab repo and build config to the webspace's CF Pages project (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::webspaces::WebspaceConnectGitInput| async move {
                endpoints::webspaces::webspace_connect_git(&pool, &p, input).await
            },
        );
        w.custom(
            "set_production_branch",
            Risk::Mutating,
            OnItem::Yes,
            "Update the production branch of the webspace's direct-upload CF Pages project (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::webspaces::WebspaceSetProductionBranchInput| async move {
                endpoints::webspaces::webspace_set_production_branch(&pool, &p, input).await
            },
        );
        w.custom(
            "deployments",
            Risk::ReadOnly,
            OnItem::Yes,
            "List the webspace's 20 most recent deployments (requires org read).",
            |pool: sqlx::PgPool, p, input: endpoints::webspaces::WebspaceDeploymentsInput| async move {
                endpoints::webspaces::webspace_deployments(&pool, &p, input).await
            },
        );
        w.custom(
            "discover_pages",
            Risk::ReadOnly,
            OnItem::No,
            "Discover CF Pages projects on a credential's account, flagging those already imported (requires org read).",
            |pool: sqlx::PgPool, p, input: endpoints::webspaces::WebspaceDiscoverPagesInput| async move {
                endpoints::webspaces::webspace_discover_pages(&pool, &p, input).await
            },
        );
        w.custom(
            "import_pages",
            Risk::Mutating,
            OnItem::No,
            "Import CF Pages projects as cloudflare hosts with one main folder each; collects per-project errors (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::webspaces::WebspaceImportPagesInput| async move {
                endpoints::webspaces::webspace_import_pages(&pool, &p, input).await
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
        let mut cd = reg.resource("changedetection", "changedetection", "Change Detection");
        cd.get(
            "Get a webspace's change-detection config: name and bound credential (requires org read).",
            |pool: sqlx::PgPool, p, input: endpoints::changedetection::CdGetInput| async move {
                endpoints::changedetection::changedetection_get(&pool, &p, input).await
            },
        );
        cd.custom(
            "set",
            Risk::Mutating,
            OnItem::Yes,
            "Bind or unbind a ChangeDetection.io credential on a webspace; switching undeploys old tags and clears old notifications (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::changedetection::CdSetInput| async move {
                endpoints::changedetection::changedetection_set(&pool, &p, input).await
            },
        );
        cd.custom(
            "suburls",
            Risk::ReadOnly,
            OnItem::Yes,
            "List a webspace's monitored sub-URLs (requires org read).",
            |pool: sqlx::PgPool, p, input: endpoints::changedetection::SubUrlListInput| async move {
                endpoints::changedetection::changedetection_suburl_list(&pool, &p, input).await
            },
        );
        cd.custom(
            "suburl_create",
            Risk::Mutating,
            OnItem::Yes,
            "Add a monitored sub-URL to a webspace; the path is normalized (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::changedetection::SubUrlCreateInput| async move {
                endpoints::changedetection::changedetection_suburl_create(&pool, &p, input).await
            },
        );
        cd.custom(
            "suburl_get",
            Risk::ReadOnly,
            OnItem::Yes,
            "Get a sub-URL's path and tag settings (requires org read).",
            |pool: sqlx::PgPool, p, input: endpoints::changedetection::SubUrlGetInput| async move {
                endpoints::changedetection::changedetection_suburl_get(&pool, &p, input).await
            },
        );
        cd.custom(
            "suburl_update",
            Risk::Mutating,
            OnItem::Yes,
            "Update a sub-URL's tag settings; pushes to ChangeDetection.io when a tag is deployed (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::changedetection::SubUrlUpdateInput| async move {
                endpoints::changedetection::changedetection_suburl_update(&pool, &p, input).await
            },
        );
        cd.custom(
            "suburl_delete",
            Risk::Destructive,
            OnItem::Yes,
            "Delete a monitored sub-URL, its deployed tag, and its notifications (requires org write).",
            |pool: sqlx::PgPool, p, input: endpoints::changedetection::SubUrlDeleteInput| async move {
                endpoints::changedetection::changedetection_suburl_delete(&pool, &p, input).await
            },
        );
        cd.custom(
            "notifications",
            Risk::ReadOnly,
            OnItem::Yes,
            "List a webspace's latest change notifications, newest first, capped at 100 (requires org read).",
            |pool: sqlx::PgPool, p, input: endpoints::changedetection::NotificationListInput| async move {
                endpoints::changedetection::changedetection_notifications(&pool, &p, input).await
            },
        );
        cd.custom(
            "notification_get",
            Risk::ReadOnly,
            OnItem::Yes,
            "Get a single change notification with full body (requires org read).",
            |pool: sqlx::PgPool, p, input: endpoints::changedetection::NotificationGetInput| async move {
                endpoints::changedetection::changedetection_notification_get(&pool, &p, input).await
            },
        );
    }
    {
        let mut t = reg.resource("tokens", "token", "Tokens");
        t.list(
            "List API tokens: with webspace_id, that webspace's deploy tokens (org admin); without, all tokens capped at 100 (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::TokenListInput| async move {
                endpoints::tokens::token_list(&pool, &p, input).await
            },
        );
        t.create(
            "Create a token; returns the plaintext secret once. kind 'deploy' + webspace_id needs org admin; admin/api/metrics kinds are admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::TokenCreateInput| async move {
                endpoints::tokens::token_create(&pool, &p, input).await
            },
        );
        t.delete(
            "Revoke a token (row kept for audit): webspace-bound deploy tokens need org admin of the owning org; all others admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::TokenRevokeInput| async move {
                endpoints::tokens::token_revoke(&pool, &p, input).await
            },
        );
    }
    {
        let mut a = reg.resource("action-templates", "action_template", "Action Templates");
        a.list(
            "List the action templates baked into this server (Ansible-like YAML task lists).",
            |pool: sqlx::PgPool, p, input: endpoints::action_templates::ActionTemplateListInput| async move {
                endpoints::action_templates::action_template_list(&pool, &p, input).await
            },
        );
        a.get(
            "Get an action template: YAML source plus the parsed spec (inputs + steps). The id is the template name.",
            |pool: sqlx::PgPool, p, input: endpoints::action_templates::ActionTemplateGetInput| async move {
                endpoints::action_templates::action_template_get(&pool, &p, input).await
            },
        );
        a.custom(
            "start",
            Risk::Mutating,
            OnItem::Yes,
            "Start an action-template run on the durable queue; returns the run id. Steps execute as the caller, so per-step org permissions apply.",
            |pool: sqlx::PgPool, p, input: endpoints::action_templates::ActionTemplateStartInput| async move {
                endpoints::action_templates::action_template_start(&pool, &p, input).await
            },
        );
        a.custom(
            "run_status",
            Risk::ReadOnly,
            OnItem::Yes,
            "Long-poll a run's live status: current step, log events after after_seq, and the report once done (own runs; admins any).",
            |pool: sqlx::PgPool, p, input: endpoints::action_templates::ActionTemplateRunStatusInput| async move {
                endpoints::action_templates::action_template_run_status(&pool, &p, input).await
            },
        );
        a.custom(
            "execute",
            Risk::Mutating,
            OnItem::Yes,
            "Execute an action template with parameters and wait for the per-step run report. Step failures are reported in the result (ok=false), not as errors.",
            |pool: sqlx::PgPool, p, input: endpoints::action_templates::ActionTemplateStartInput| async move {
                endpoints::action_templates::action_template_execute(&pool, &p, input).await
            },
        );
        a.custom(
            "runs",
            Risk::ReadOnly,
            OnItem::Yes,
            "List a template's past runs with reports and debug logs (own runs; admins all).",
            |pool: sqlx::PgPool, p, input: endpoints::action_templates::ActionTemplateRunsInput| async move {
                endpoints::action_templates::action_template_runs(&pool, &p, input).await
            },
        );
        a.custom(
            "input_options",
            Risk::ReadOnly,
            OnItem::Yes,
            "Resolve picker options for the template's id-typed inputs from their ref'd resources' list endpoints (scoped to the caller).",
            |pool: sqlx::PgPool, p, input: endpoints::action_templates::ActionTemplateInputOptionsInput| async move {
                endpoints::action_templates::action_template_input_options(&pool, &p, input).await
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

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    /// `http_router` panics if the schemars-generated OpenAPI document does
    /// not parse into utoipa's model (e.g. boolean schemas from
    /// `serde_json::Value` fields) — catch that in CI, not at server boot.
    #[tokio::test]
    async fn openapi_document_parses_into_utoipa() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/unused")
            .expect("lazy pool");
        let registry = build_registry(pool.clone());
        let _ = registry.http_router(pool);
    }

    /// The compile-time half of template validation can't check tool names
    /// (the registry is runtime-only); this test closes that gap in CI.
    #[tokio::test]
    async fn baked_templates_reference_known_tools() {
        use plan_ai_actions::engine::{BuiltinRegistry, validate_template};

        // connect_lazy never touches the network; the registry only needs a
        // pool value, not a database.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/unused")
            .expect("lazy pool");
        let registry = build_registry(pool);
        let builtins = BuiltinRegistry::standard();
        let tools: Vec<String> = registry
            .tool_names()
            .into_iter()
            .filter(|t| !t.starts_with("action_template_"))
            .collect();

        for (name, spec) in endpoints::action_templates::baked_templates() {
            if let Err(errors) = validate_template(spec, Some(&tools), &builtins) {
                panic!("action template '{name}' is invalid:\n  {}", errors.join("\n  "));
            }
            for (input, ispec) in &spec.inputs {
                if let Some(resource) = &ispec.reference {
                    assert!(
                        registry.list_tool_for_resource(resource).is_some(),
                        "template '{name}': input '{input}' ref '{resource}' has no list endpoint"
                    );
                }
            }
        }
    }
}
