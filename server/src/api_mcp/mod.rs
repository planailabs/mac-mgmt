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
