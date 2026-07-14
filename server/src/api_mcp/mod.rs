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
    use plan_ai_api_mcp::{OnItem, Registry, Risk};
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
        c.get(
            "Get a cluster (id, name, pinned daemon version, nixpkgs commit, created_at); requires cluster read.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::ClusterGetInput| async move {
                endpoints::clusters::cluster_get(&pool, &p, input).await
            },
        );
        c.create(
            "Create a cluster with the given name (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::ClusterCreateInput| async move {
                endpoints::clusters::cluster_create(&pool, &p, input).await
            },
        );
        c.update(
            "Rename a cluster (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::ClusterUpdateInput| async move {
                endpoints::clusters::cluster_update(&pool, &p, input).await
            },
        );
        c.delete(
            "Delete a cluster and all its dependent rows (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::ClusterDeleteInput| async move {
                endpoints::clusters::cluster_delete(&pool, &p, input).await
            },
        );
        c.custom(
            "can_write",
            Risk::ReadOnly,
            OnItem::Yes,
            "Whether the caller has write access to the cluster (via org membership or admin).",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::ClusterCanWriteInput| async move {
                endpoints::clusters::cluster_can_write(&pool, &p, input).await
            },
        );
        c.custom(
            "can_admin",
            Risk::ReadOnly,
            OnItem::Yes,
            "Whether the caller is an org-admin of an organization owning the cluster (or global admin).",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::ClusterCanAdminInput| async move {
                endpoints::clusters::cluster_can_admin(&pool, &p, input).await
            },
        );
        c.custom(
            "pinned_rollout",
            Risk::ReadOnly,
            OnItem::No,
            "Resolve the most recent rollout id targeting the given daemon version, if any.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::PinnedRolloutInput| async move {
                endpoints::clusters::cluster_pinned_rollout(&pool, &p, input).await
            },
        );
        c.custom(
            "set_pinned_version",
            Risk::Mutating,
            OnItem::Yes,
            "Pin the cluster's daemon version (empty string clears the pin); requires cluster write.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::SetPinnedVersionInput| async move {
                endpoints::clusters::cluster_set_pinned_version(&pool, &p, input).await
            },
        );
        c.custom(
            "set_nixpkgs",
            Risk::Mutating,
            OnItem::Yes,
            "Pin the cluster's nixpkgs commit (7-40 hex chars, must not be older than the current pin; empty string clears it) and push a nixpkgs sync; requires cluster write.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::SetNixpkgsInput| async move {
                endpoints::clusters::cluster_set_nixpkgs(&pool, &p, input).await
            },
        );
        c.custom(
            "active_rollouts",
            Risk::ReadOnly,
            OnItem::Yes,
            "List rollouts currently targeting the cluster (status rolling or paused); requires cluster read.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::ActiveRolloutsInput| async move {
                endpoints::clusters::cluster_active_rollouts(&pool, &p, input).await
            },
        );
        c.custom(
            "cloud_init",
            Risk::Mutating,
            OnItem::Yes,
            "Generate a cloud-init YAML for enrolling a new machine into the cluster; mints a fresh sync token as a side effect. Requires cluster write.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::CloudInitInput| async move {
                endpoints::clusters::cluster_cloud_init(&pool, &p, input).await
            },
        );
        c.custom(
            "secrets_list",
            Risk::ReadOnly,
            OnItem::Yes,
            "List the cluster's secret names and creation times (values are never returned); requires cluster read.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::SecretsListInput| async move {
                endpoints::clusters::cluster_secrets_list(&pool, &p, input).await
            },
        );
        c.custom(
            "secret_create",
            Risk::Mutating,
            OnItem::Yes,
            "Create a cluster secret (stored encrypted, referenced from config as secret:<name>) and push a config sync; requires cluster write.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::SecretCreateInput| async move {
                endpoints::clusters::cluster_secret_create(&pool, &p, input).await
            },
        );
        c.custom(
            "secret_update",
            Risk::Mutating,
            OnItem::Yes,
            "Replace the value of an existing cluster secret and push a config sync; requires cluster write.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::SecretUpdateInput| async move {
                endpoints::clusters::cluster_secret_update(&pool, &p, input).await
            },
        );
        c.custom(
            "secret_delete",
            Risk::Destructive,
            OnItem::Yes,
            "Delete a cluster secret by name and push a config sync; requires cluster write.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::SecretDeleteInput| async move {
                endpoints::clusters::cluster_secret_delete(&pool, &p, input).await
            },
        );
        c.custom(
            "packages_manual",
            Risk::ReadOnly,
            OnItem::Yes,
            "List the cluster's manually added nix packages (row id + package name); requires cluster read.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::ManualPackagesInput| async move {
                endpoints::clusters::cluster_packages_manual(&pool, &p, input).await
            },
        );
        c.custom(
            "package_add",
            Risk::Mutating,
            OnItem::Yes,
            "Add a manual nix package to the cluster (idempotent) and push a package sync; requires cluster write.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::PackageAddInput| async move {
                endpoints::clusters::cluster_package_add(&pool, &p, input).await
            },
        );
        c.custom(
            "package_remove",
            Risk::Mutating,
            OnItem::Yes,
            "Remove a manual package (by its packages_manual row id) from the cluster and push a package sync; requires cluster write.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::PackageRemoveInput| async move {
                endpoints::clusters::cluster_package_remove(&pool, &p, input).await
            },
        );
        c.custom(
            "packages_all",
            Risk::ReadOnly,
            OnItem::Yes,
            "List every nix package the cluster installs, with sources (manual, mcp:<slug>, skill:<slug>); requires cluster read.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::AllPackagesInput| async move {
                endpoints::clusters::cluster_packages_all(&pool, &p, input).await
            },
        );
        c.custom(
            "healer_settings_get",
            Risk::ReadOnly,
            OnItem::Yes,
            "Get the cluster's healer override settings plus the selectable model list; requires cluster read.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::HealerSettingsInput| async move {
                endpoints::clusters::cluster_healer_settings_get(&pool, &p, input).await
            },
        );
        c.custom(
            "healer_settings_set",
            Risk::Mutating,
            OnItem::Yes,
            "Upsert the cluster's healer override settings (enable override, auto-trigger/approve, models); requires cluster write.",
            |pool: sqlx::PgPool, p, input: endpoints::clusters::HealerSettingsSetInput| async move {
                endpoints::clusters::cluster_healer_settings_set(&pool, &p, input).await
            },
        );
    }

    {
        let mut o = reg.resource("organizations", "organization", "Organizations");
        o.list(
            "List all organizations with member and cluster counts (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgListInput| async move {
                endpoints::organizations::organization_list(&pool, &p, input).await
            },
        );
        o.get(
            "Get an organization (name, created_at); requires org membership or admin.",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgGetInput| async move {
                endpoints::organizations::organization_get(&pool, &p, input).await
            },
        );
        o.create(
            "Create an organization with the given name; returns the new org id (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgCreateInput| async move {
                endpoints::organizations::organization_create(&pool, &p, input).await
            },
        );
        o.update(
            "Rename an organization (org-admin or global admin).",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgUpdateInput| async move {
                endpoints::organizations::organization_update(&pool, &p, input).await
            },
        );
        o.delete(
            "Delete an organization and its memberships, cluster links and tokens (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgDeleteInput| async move {
                endpoints::organizations::organization_delete(&pool, &p, input).await
            },
        );
        o.custom(
            "permissions",
            Risk::ReadOnly,
            OnItem::Yes,
            "What the caller can do on the organization: global-admin and org-admin flags.",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgPermissionsInput| async move {
                endpoints::organizations::organization_permissions(&pool, &p, input).await
            },
        );
        o.custom(
            "members",
            Risk::ReadOnly,
            OnItem::Yes,
            "List the organization's members (user id, email, name, role); requires org membership or admin.",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgMembersInput| async move {
                endpoints::organizations::organization_members(&pool, &p, input).await
            },
        );
        o.custom(
            "member_add",
            Risk::Mutating,
            OnItem::Yes,
            "Add a user to the organization with role admin, write or read; requires org-admin.",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgMemberAddInput| async move {
                endpoints::organizations::organization_member_add(&pool, &p, input).await
            },
        );
        o.custom(
            "member_remove",
            Risk::Mutating,
            OnItem::Yes,
            "Remove a user from the organization; requires org-admin.",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgMemberRemoveInput| async move {
                endpoints::organizations::organization_member_remove(&pool, &p, input).await
            },
        );
        o.custom(
            "member_set_role",
            Risk::Mutating,
            OnItem::Yes,
            "Change a member's role in the organization (admin, write or read); requires org-admin.",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgMemberSetRoleInput| async move {
                endpoints::organizations::organization_member_set_role(&pool, &p, input).await
            },
        );
        o.custom(
            "clusters",
            Risk::ReadOnly,
            OnItem::Yes,
            "List the clusters linked to the organization; requires org membership or admin.",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgClustersInput| async move {
                endpoints::organizations::organization_clusters(&pool, &p, input).await
            },
        );
        o.custom(
            "cluster_add",
            Risk::Mutating,
            OnItem::Yes,
            "Link a cluster to the organization, granting the org's members access to it (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgClusterAddInput| async move {
                endpoints::organizations::organization_cluster_add(&pool, &p, input).await
            },
        );
        o.custom(
            "cluster_remove",
            Risk::Mutating,
            OnItem::Yes,
            "Unlink a cluster from the organization, revoking the org's access to it (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgClusterRemoveInput| async move {
                endpoints::organizations::organization_cluster_remove(&pool, &p, input).await
            },
        );
        o.custom(
            "available_users",
            Risk::ReadOnly,
            OnItem::Yes,
            "List users that are not yet members of the organization (picker for member_add); requires org-admin.",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgAvailableUsersInput| async move {
                endpoints::organizations::organization_available_users(&pool, &p, input).await
            },
        );
        o.custom(
            "available_clusters",
            Risk::ReadOnly,
            OnItem::Yes,
            "List clusters not yet linked to the organization (picker for cluster_add); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgAvailableClustersInput| async move {
                endpoints::organizations::organization_available_clusters(&pool, &p, input).await
            },
        );
        o.custom(
            "tokens_list",
            Risk::ReadOnly,
            OnItem::Yes,
            "List the organization's API tokens (label, kind, revoked, expiry; never the token value); requires org-admin.",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgTokensListInput| async move {
                endpoints::organizations::organization_tokens_list(&pool, &p, input).await
            },
        );
        o.custom(
            "token_create",
            Risk::Mutating,
            OnItem::Yes,
            "Create an org-scoped API token with a label and optional expiry (seconds from now). Returns the plaintext token, shown only this once — store it securely, it cannot be retrieved again. Requires org-admin.",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgTokenCreateInput| async move {
                endpoints::organizations::organization_token_create(&pool, &p, input).await
            },
        );
        o.custom(
            "token_revoke",
            Risk::Mutating,
            OnItem::Yes,
            "Revoke an organization API token (irreversible); requires org-admin.",
            |pool: sqlx::PgPool, p, input: endpoints::organizations::OrgTokenRevokeInput| async move {
                endpoints::organizations::organization_token_revoke(&pool, &p, input).await
            },
        );
    }

    {
        let mut u = reg.resource("users", "user", "Users");
        u.list(
            "List all users with admin flag and organization names (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserListInput| async move {
                endpoints::users::user_list(&pool, &p, input).await
            },
        );
        u.get(
            "Get a user (email, name, admin flag, created_at); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserGetInput| async move {
                endpoints::users::user_get(&pool, &p, input).await
            },
        );
        u.create(
            "Create a user with email, display name and optional global-admin flag; returns the new user id (admin only).",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserCreateInput| async move {
                endpoints::users::user_create(&pool, &p, input).await
            },
        );
        u.delete(
            "Delete a user and their org memberships (admin only; you cannot delete yourself).",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserDeleteInput| async move {
                endpoints::users::user_delete(&pool, &p, input).await
            },
        );
        u.custom(
            "set_admin",
            Risk::Mutating,
            OnItem::Yes,
            "Grant or revoke a user's global-admin status (admin only; you cannot revoke your own).",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserSetAdminInput| async move {
                endpoints::users::user_set_admin(&pool, &p, input).await
            },
        );
        u.custom(
            "orgs",
            Risk::ReadOnly,
            OnItem::Yes,
            "List the organizations the user belongs to, with their role in each; admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserOrgsInput| async move {
                endpoints::users::user_orgs(&pool, &p, input).await
            },
        );
        u.custom(
            "available_orgs",
            Risk::ReadOnly,
            OnItem::Yes,
            "List organizations the user is not yet a member of (picker for org_add); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserAvailableOrgsInput| async move {
                endpoints::users::user_available_orgs(&pool, &p, input).await
            },
        );
        u.custom(
            "org_add",
            Risk::Mutating,
            OnItem::Yes,
            "Add the user to an organization with role admin, write or read (idempotent); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserOrgAddInput| async move {
                endpoints::users::user_org_add(&pool, &p, input).await
            },
        );
        u.custom(
            "org_remove",
            Risk::Mutating,
            OnItem::Yes,
            "Remove the user from an organization; admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::users::UserOrgRemoveInput| async move {
                endpoints::users::user_org_remove(&pool, &p, input).await
            },
        );
    }

    {
        let mut t = reg.resource("tokens", "token", "Tokens");
        t.custom(
            "sync_list",
            Risk::ReadOnly,
            OnItem::No,
            "List a cluster's sync tokens (label, revoked, expiry; never the token value); requires cluster read.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::SyncTokensListInput| async move {
                endpoints::tokens::token_sync_list(&pool, &p, input).await
            },
        );
        t.custom(
            "sync_create",
            Risk::Mutating,
            OnItem::No,
            "Create a cluster sync token (daemon enrollment/sync) with a label and optional expiry (seconds from now). Returns the plaintext token, shown only this once — store it securely, it cannot be retrieved again. Requires cluster write.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::SyncTokenCreateInput| async move {
                endpoints::tokens::token_sync_create(&pool, &p, input).await
            },
        );
        t.custom(
            "sync_revoke",
            Risk::Mutating,
            OnItem::Yes,
            "Revoke a cluster sync token (irreversible); requires write access to the owning cluster.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::SyncTokenRevokeInput| async move {
                endpoints::tokens::token_sync_revoke(&pool, &p, input).await
            },
        );
        t.custom(
            "setting_list",
            Risk::ReadOnly,
            OnItem::No,
            "List a cluster's setting tokens (label, revoked, expiry; never the token value); requires cluster read.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::SettingTokensListInput| async move {
                endpoints::tokens::token_setting_list(&pool, &p, input).await
            },
        );
        t.custom(
            "setting_create",
            Risk::Mutating,
            OnItem::No,
            "Create a cluster setting token with a label and optional expiry (seconds from now). Returns the plaintext token, shown only this once — store it securely, it cannot be retrieved again. Requires cluster write.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::SettingTokenCreateInput| async move {
                endpoints::tokens::token_setting_create(&pool, &p, input).await
            },
        );
        t.custom(
            "setting_revoke",
            Risk::Mutating,
            OnItem::Yes,
            "Revoke a cluster setting token (irreversible); requires write access to the owning cluster.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::SettingTokenRevokeInput| async move {
                endpoints::tokens::token_setting_revoke(&pool, &p, input).await
            },
        );
        t.custom(
            "admin_list",
            Risk::ReadOnly,
            OnItem::No,
            "List global admin tokens (label, revoked, expiry; never the token value); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::AdminTokensListInput| async move {
                endpoints::tokens::token_admin_list(&pool, &p, input).await
            },
        );
        t.custom(
            "admin_create",
            Risk::Mutating,
            OnItem::No,
            "Create a global admin token with a label and optional expiry (seconds from now). Returns the plaintext token, shown only this once — store it securely, it cannot be retrieved again. Admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::AdminTokenCreateInput| async move {
                endpoints::tokens::token_admin_create(&pool, &p, input).await
            },
        );
        t.custom(
            "admin_revoke",
            Risk::Mutating,
            OnItem::Yes,
            "Revoke a global admin token (irreversible); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::AdminTokenRevokeInput| async move {
                endpoints::tokens::token_admin_revoke(&pool, &p, input).await
            },
        );
        t.custom(
            "federation_list",
            Risk::ReadOnly,
            OnItem::No,
            "List federation tokens (label, revoked, expiry; never the token value); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::FederationTokensListInput| async move {
                endpoints::tokens::token_federation_list(&pool, &p, input).await
            },
        );
        t.custom(
            "federation_create",
            Risk::Mutating,
            OnItem::No,
            "Create a federation token (fed_-prefixed) with a label and optional expiry (seconds from now). Returns the plaintext token, shown only this once — store it securely, it cannot be retrieved again. Admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::FederationTokenCreateInput| async move {
                endpoints::tokens::token_federation_create(&pool, &p, input).await
            },
        );
        t.custom(
            "federation_revoke",
            Risk::Mutating,
            OnItem::Yes,
            "Revoke a federation token (irreversible); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::FederationTokenRevokeInput| async move {
                endpoints::tokens::token_federation_revoke(&pool, &p, input).await
            },
        );
        t.custom(
            "list_all",
            Risk::ReadOnly,
            OnItem::No,
            "List every token across the system with its kind and resolved org/cluster scope (never the token value); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::AllTokensListInput| async move {
                endpoints::tokens::token_list_all(&pool, &p, input).await
            },
        );
        t.custom(
            "revoke_any",
            Risk::Mutating,
            OnItem::Yes,
            "Revoke any token by id, regardless of kind or scope (irreversible); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::tokens::AnyTokenRevokeInput| async move {
                endpoints::tokens::token_revoke_any(&pool, &p, input).await
            },
        );
    }

    {
        let mut s = reg.resource("ssh_keys", "ssh_key", "SSH keys");
        s.list(
            "List a cluster's authorized SSH public keys (fingerprint + comment); requires cluster read.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::SshKeysListInput| async move {
                endpoints::certificates::ssh_key_list(&pool, &p, input).await
            },
        );
        s.create(
            "Add an OpenSSH public key to a cluster and push an SSH-key sync; requires org-admin of an owning organization.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::SshKeyAddInput| async move {
                endpoints::certificates::ssh_key_add(&pool, &p, input).await
            },
        );
        s.delete(
            "Remove an SSH key from its cluster and push an SSH-key sync; requires org-admin of an owning organization.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::SshKeyRemoveInput| async move {
                endpoints::certificates::ssh_key_remove(&pool, &p, input).await
            },
        );
    }

    {
        let mut c = reg.resource("certificates", "certificate", "Client certificates");
        c.custom(
            "cluster_certs_list",
            Risk::ReadOnly,
            OnItem::No,
            "List a cluster's client certificates (fingerprint + label); requires cluster read.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::ClusterCertsListInput| async move {
                endpoints::certificates::cert_cluster_list(&pool, &p, input).await
            },
        );
        c.custom(
            "cluster_cert_add",
            Risk::Mutating,
            OnItem::No,
            "Add a client certificate to a cluster, by PEM (fingerprint derived) or bare fingerprint; requires org-admin of an owning organization.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::ClusterCertAddInput| async move {
                endpoints::certificates::cert_cluster_add(&pool, &p, input).await
            },
        );
        c.custom(
            "cluster_cert_remove",
            Risk::Destructive,
            OnItem::Yes,
            "Remove a client certificate from its cluster; requires org-admin of an owning organization.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::ClusterCertRemoveInput| async move {
                endpoints::certificates::cert_cluster_remove(&pool, &p, input).await
            },
        );
        c.custom(
            "cluster_cas_list",
            Risk::ReadOnly,
            OnItem::No,
            "List a cluster's client CAs (fingerprint + label); requires cluster read.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::ClusterCasListInput| async move {
                endpoints::certificates::ca_cluster_list(&pool, &p, input).await
            },
        );
        c.custom(
            "cluster_ca_add",
            Risk::Mutating,
            OnItem::No,
            "Add a client CA certificate (PEM) to a cluster; requires org-admin of an owning organization.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::ClusterCaAddInput| async move {
                endpoints::certificates::ca_cluster_add(&pool, &p, input).await
            },
        );
        c.custom(
            "cluster_ca_remove",
            Risk::Destructive,
            OnItem::Yes,
            "Remove a client CA from its cluster; requires org-admin of an owning organization.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::ClusterCaRemoveInput| async move {
                endpoints::certificates::ca_cluster_remove(&pool, &p, input).await
            },
        );
        c.custom(
            "org_certs_list",
            Risk::ReadOnly,
            OnItem::No,
            "List an organization's client certificates (fingerprint + label); requires org membership or admin.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::OrgCertsListInput| async move {
                endpoints::certificates::cert_org_list(&pool, &p, input).await
            },
        );
        c.custom(
            "org_cert_add",
            Risk::Mutating,
            OnItem::No,
            "Add a client certificate to an organization, by PEM (fingerprint derived) or bare fingerprint; requires org-admin.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::OrgCertAddInput| async move {
                endpoints::certificates::cert_org_add(&pool, &p, input).await
            },
        );
        c.custom(
            "org_cert_remove",
            Risk::Destructive,
            OnItem::Yes,
            "Remove a client certificate from an organization; requires org-admin.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::OrgCertRemoveInput| async move {
                endpoints::certificates::cert_org_remove(&pool, &p, input).await
            },
        );
        c.custom(
            "org_cas_list",
            Risk::ReadOnly,
            OnItem::No,
            "List an organization's client CAs (fingerprint + label); requires org membership or admin.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::OrgCasListInput| async move {
                endpoints::certificates::ca_org_list(&pool, &p, input).await
            },
        );
        c.custom(
            "org_ca_add",
            Risk::Mutating,
            OnItem::No,
            "Add a client CA certificate (PEM) to an organization; requires org-admin.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::OrgCaAddInput| async move {
                endpoints::certificates::ca_org_add(&pool, &p, input).await
            },
        );
        c.custom(
            "org_ca_remove",
            Risk::Destructive,
            OnItem::Yes,
            "Remove a client CA from an organization; requires org-admin.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::OrgCaRemoveInput| async move {
                endpoints::certificates::ca_org_remove(&pool, &p, input).await
            },
        );
        c.custom(
            "admin_certs_list",
            Risk::ReadOnly,
            OnItem::No,
            "List server-wide (admin-scope) client certificates (fingerprint + label); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::AdminCertsListInput| async move {
                endpoints::certificates::cert_admin_list(&pool, &p, input).await
            },
        );
        c.custom(
            "admin_cert_add",
            Risk::Mutating,
            OnItem::No,
            "Add a server-wide (admin-scope) client certificate, by PEM (fingerprint derived) or bare fingerprint; admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::AdminCertAddInput| async move {
                endpoints::certificates::cert_admin_add(&pool, &p, input).await
            },
        );
        c.custom(
            "admin_cert_remove",
            Risk::Destructive,
            OnItem::Yes,
            "Remove a server-wide (admin-scope) client certificate; admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::AdminCertRemoveInput| async move {
                endpoints::certificates::cert_admin_remove(&pool, &p, input).await
            },
        );
        c.custom(
            "admin_cas_list",
            Risk::ReadOnly,
            OnItem::No,
            "List server-wide (admin-scope) client CAs (fingerprint + label); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::AdminCasListInput| async move {
                endpoints::certificates::ca_admin_list(&pool, &p, input).await
            },
        );
        c.custom(
            "admin_ca_add",
            Risk::Mutating,
            OnItem::No,
            "Add a server-wide (admin-scope) client CA certificate (PEM); admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::AdminCaAddInput| async move {
                endpoints::certificates::ca_admin_add(&pool, &p, input).await
            },
        );
        c.custom(
            "admin_ca_remove",
            Risk::Destructive,
            OnItem::Yes,
            "Remove a server-wide (admin-scope) client CA; admin only.",
            |pool: sqlx::PgPool, p, input: endpoints::certificates::AdminCaRemoveInput| async move {
                endpoints::certificates::ca_admin_remove(&pool, &p, input).await
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
