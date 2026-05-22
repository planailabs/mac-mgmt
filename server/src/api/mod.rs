mod auth;
pub(crate) mod healer_routes;
pub(crate) mod metrics_routes;
pub mod push;
pub(crate) mod routes;
#[cfg(feature = "server")]
pub(crate) mod secrets;
#[cfg(feature = "server")]
pub(crate) mod federation;
#[cfg(feature = "server")]
pub(crate) mod importer;

use rocket::Config;
use rocket::config::Shutdown;
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "mac-mgmt API",
        description = "API for daemon sync (sync tokens), resource management (setting tokens), and administration (admin tokens).",
    ),
    paths(
        routes::get_self,
        // Sync
        routes::get_config,
        routes::get_update_target,
        routes::get_nixpkgs_pin,
        routes::get_nix_caches,
        routes::get_skills,
        routes::get_mcp_servers,
        routes::get_packages,
        // Setting — config
        routes::setting_config_schema,
        routes::setting_get_config,
        routes::setting_set_config,
        routes::setting_patch_config,
        // Setting — skills
        routes::setting_list_skills,
        routes::setting_add_skill,
        routes::setting_remove_skill,
        routes::setting_batch_skills,
        // Setting — bundles
        routes::setting_list_bundles,
        routes::setting_add_bundle,
        routes::setting_remove_bundle,
        routes::setting_batch_bundles,
        // Setting — MCP servers
        routes::setting_list_mcp_servers,
        routes::setting_add_mcp_server,
        routes::setting_remove_mcp_server,
        routes::setting_batch_mcp_servers,
        // Setting — MCP bundles
        routes::setting_list_mcp_bundles,
        routes::setting_add_mcp_bundle,
        routes::setting_remove_mcp_bundle,
        routes::setting_batch_mcp_bundles,
        // Setting — packages
        routes::setting_list_packages,
        routes::setting_add_package,
        routes::setting_remove_package,
        routes::setting_batch_packages,
        // Setting — available
        routes::setting_available_skill_channels,
        routes::setting_available_bundles,
        routes::setting_available_mcp_servers,
        routes::setting_available_mcp_bundles,
        // Setting — bundle contents
        routes::setting_bundle_skills,
        routes::setting_mcp_bundle_servers,
        // Setting — catalog
        routes::setting_catalog,
        // Sync — SSH keys
        routes::get_ssh_keys,
        // Setting — SSH keys
        routes::setting_list_ssh_keys,
        routes::setting_add_ssh_key,
        routes::setting_remove_ssh_key,
        // Setting — Client certs
        routes::setting_list_client_certs,
        routes::setting_add_client_cert,
        routes::setting_remove_client_cert,
        // Relay — Cert auth validation
        routes::cert_auth,
        // Sync — Heartbeat
        routes::post_heartbeat,
        // Sync — System assessment
        routes::post_assessment,
        routes::post_assessment_probe,
        // Admin
        routes::admin_list_clusters,
        routes::admin_create_cluster,
        routes::admin_delete_cluster,
        routes::admin_list_cluster_machines,
        routes::admin_create_token,
        routes::admin_create_org_token,
        // Setting — cloud-init
        routes::setting_cloud_init,
        // Admin — Rollouts
        routes::admin_create_rollout_group,
        routes::admin_list_rollout_groups,
        routes::admin_add_group_member,
        routes::admin_remove_group_member,
        routes::admin_create_rollout,
        routes::admin_list_rollouts,
        routes::admin_get_rollout,
        routes::admin_start_rollout,
        routes::admin_advance_rollout,
        routes::admin_pause_rollout,
        routes::admin_complete_rollout,
        routes::admin_resume_rollout,
        routes::admin_delete_rollout,
        routes::admin_delete_rollout_group,
        routes::admin_get_rollout_group,
        routes::admin_stage_health,
        routes::admin_request_stage_assessment,
        routes::admin_rollback_rollout,
    ),
    components(schemas(
        routes::SelfInfo,
        routes::SetConfigBody,
        routes::PatchConfigBody,
        routes::AddSkillBody,
        routes::BatchSkillsBody,
        routes::ClusterBundleRow,
        routes::AddBundleBody,
        routes::BatchBundlesBody,
        routes::AddMcpServerBody,
        routes::BatchMcpServersBody,
        routes::McpServerOptionRow,
        routes::ClusterMcpBundleRow,
        routes::AddMcpBundleBody,
        routes::BatchMcpBundlesBody,
        routes::SkillChannelRow,
        routes::BundleSkillChannelRow,
        routes::BundleMcpServerRow,
        routes::OptionRow,
        routes::Catalog,
        routes::CatalogBundle,
        routes::CatalogMcpBundle,
        routes::AdminClusterRow,
        routes::CreateClusterForOrgBody,
        routes::CreatedCluster,
        routes::AdminMachineRow,
        routes::CloudInitBody,
        routes::CloudInitResponse,
        routes::CreateTokenForClusterBody,
        routes::CreateOrgTokenBody,
        routes::CreatedToken,
        routes::SshKeyRow,
        routes::AddSshKeyBody,
        routes::ClusterPackageRow,
        routes::AddPackageBody,
        routes::BatchPackagesBody,
        routes::CreateRolloutGroupBody,
        routes::RolloutGroupRow,
        routes::AddGroupMemberBody,
        routes::CreateRolloutBody,
        routes::RolloutRow,
        routes::RolloutDetail,
        routes::StageDetail,
        routes::GroupDetail,
        routes::GroupMemberRow,
    )),
    security(("bearer" = [])),
    modifiers(&SecurityAddon),
)]
struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("token")
                        .description(Some(
                            "Sync, setting, or admin token. Token kind determines which endpoints are accessible.",
                        ))
                        .build(),
                ),
            );
        }
    }
}

pub fn build_rocket(
    pool: PgPool,
    port: u16,
    push_channels: push::PushChannels,
    healer_state: mac_mgmt_healer::HealerState,
    pg_healer_store: std::sync::Arc<mac_mgmt_healer::store::pg::PgHealerStore>,
    federation_push: push::FederationPushChannel,
    skill_center_cache: crate::skill_center_cache::SkillCenterCache,
) -> rocket::Rocket<rocket::Build> {
    let config = Config {
        port,
        address: std::net::Ipv4Addr::UNSPECIFIED.into(),
        shutdown: Shutdown {
            ctrlc: false, // Dioxus owns signal handling; don't let Rocket intercept
            grace: 2,
            mercy: 2,
            ..Shutdown::default()
        },
        ..Config::default()
    };

    // Build route list incrementally based on features
    let mut api_routes: Vec<rocket::Route> = rocket::routes![
        // Common routes (any token kind)
        routes::get_self,
        // Sync token routes (always needed — daemons call these)
        routes::get_config,
        routes::get_update_target,
        routes::get_nixpkgs_pin,
        routes::get_nix_caches,
        routes::get_skills,
        routes::get_mcp_servers,
        routes::get_packages,
        // Setting token routes — config
        routes::setting_config_schema,
        routes::setting_get_config,
        routes::setting_set_config,
        routes::setting_patch_config,
        // Sync/Setting — SSH keys
        routes::get_ssh_keys,
        routes::setting_list_ssh_keys,
        routes::setting_add_ssh_key,
        routes::setting_remove_ssh_key,
        // Setting — Client certs
        routes::setting_list_client_certs,
        routes::setting_add_client_cert,
        routes::setting_remove_client_cert,
        // Relay — Cert auth validation
        routes::cert_auth,
        // Public server info (unauthenticated)
        routes::get_server_info,
        // Daemon binary download (public)
        routes::download_daemon,
        // Nixpkgs source archive (public)
        routes::get_nixpkgs_archive,
        // Metrics
        metrics_routes::get_metrics,
    ];

    let mode = &crate::config::config().mode;
    use crate::config::ServerMode;

    // Skill center routes — catalog CRUD for skills, bundles, MCP servers, MCP bundles
    if matches!(mode, ServerMode::Monolith | ServerMode::SkillCenter | ServerMode::SkillImporter) {
    api_routes.append(&mut rocket::routes![
        // Setting — skills
        routes::setting_list_skills,
        routes::setting_add_skill,
        routes::setting_remove_skill,
        routes::setting_batch_skills,
        // Setting — bundles
        routes::setting_list_bundles,
        routes::setting_add_bundle,
        routes::setting_remove_bundle,
        routes::setting_batch_bundles,
        // Setting — MCP servers
        routes::setting_list_mcp_servers,
        routes::setting_add_mcp_server,
        routes::setting_remove_mcp_server,
        routes::setting_batch_mcp_servers,
        // Setting — MCP bundles
        routes::setting_list_mcp_bundles,
        routes::setting_add_mcp_bundle,
        routes::setting_remove_mcp_bundle,
        routes::setting_batch_mcp_bundles,
        // Setting — available resources
        routes::setting_available_skill_channels,
        routes::setting_available_bundles,
        routes::setting_available_mcp_servers,
        routes::setting_available_mcp_bundles,
        // Setting — bundle contents
        routes::setting_bundle_skills,
        routes::setting_mcp_bundle_servers,
        // Setting — catalog
        routes::setting_catalog,
        // Setting — manual packages
        routes::setting_list_packages,
        routes::setting_add_package,
        routes::setting_remove_package,
        routes::setting_batch_packages,
        // Admin — skill MCP dependencies
        routes::admin_list_skill_mcp_deps,
        routes::admin_add_skill_mcp_dep,
        routes::admin_remove_skill_mcp_dep,
        // Admin — skill nix packages
        routes::admin_get_skill_nix_packages,
        routes::admin_set_skill_nix_packages,
    ]);
    // Federation API (requires the full server feature for the federation module)
    #[cfg(feature = "server")]
    api_routes.append(&mut rocket::routes![
        federation::federation_catalog,
        federation::federation_resolve_skills,
        federation::federation_resolve_mcp_servers,
        federation::federation_events,
    ]);
    }

    // Skill importer routes — import from git repos and ClawHub
    #[cfg(feature = "server")]
    if matches!(mode, ServerMode::Monolith | ServerMode::SkillImporter) {
    api_routes.append(&mut rocket::routes![
        importer::create_source,
        importer::list_sources,
        importer::delete_source,
        importer::sync_source,
        importer::list_jobs,
        importer::get_job,
        importer::search_clawhub,
    ]);
    }

    // Management server routes — fleet orchestration, clusters, rollouts, healer
    if matches!(mode, ServerMode::Monolith | ServerMode::Mgmt) {
    api_routes.append(&mut rocket::routes![
        // Admin — clusters
        routes::admin_list_clusters,
        routes::admin_create_cluster,
        routes::admin_delete_cluster,
        routes::admin_list_cluster_machines,
        routes::admin_create_token,
        routes::admin_create_org_token,
        // Setting — cloud-init
        routes::setting_cloud_init,
        // Proxy token
        routes::create_proxy_token,
        // Sync — Heartbeat
        routes::post_heartbeat,
        // Sync — System assessment
        routes::post_assessment,
        routes::post_assessment_probe,
        // Admin — Rollouts
        routes::admin_create_rollout_group,
        routes::admin_list_rollout_groups,
        routes::admin_add_group_member,
        routes::admin_remove_group_member,
        routes::admin_create_rollout,
        routes::admin_list_rollouts,
        routes::admin_get_rollout,
        routes::admin_start_rollout,
        routes::admin_advance_rollout,
        routes::admin_pause_rollout,
        routes::admin_complete_rollout,
        routes::admin_resume_rollout,
        routes::admin_delete_rollout,
        routes::admin_delete_rollout_group,
        routes::admin_get_rollout_group,
        routes::admin_stage_health,
        routes::admin_request_stage_assessment,
        routes::admin_rollback_rollout,
        // SSE push
        push::sse_events,
        // Healer sessions
        healer_routes::create_session,
        healer_routes::list_sessions,
        healer_routes::get_session,
        healer_routes::cancel_session,
        healer_routes::pause_session,
        healer_routes::resume_session,
        healer_routes::approve_session,
        healer_routes::extend_budget,
        healer_routes::get_healer_settings,
        healer_routes::put_healer_settings,
        healer_routes::stream_session,
        // Admin — staff pings
        healer_routes::admin_list_staff_pings,
        healer_routes::admin_get_staff_ping,
        healer_routes::admin_resolve_staff_ping,
        healer_routes::admin_unresolve_staff_ping,
        // Admin — skill centers
        routes::admin_list_skill_centers,
        routes::admin_create_skill_center,
        routes::admin_update_skill_center,
        routes::admin_delete_skill_center,
        // Admin — federation tokens
        routes::admin_create_federation_token,
    ]);
    }

    // Secrets vault (requires the full server feature for the secrets module)
    #[cfg(feature = "server")]
    if matches!(mode, ServerMode::Monolith | ServerMode::Mgmt) {
    api_routes.append(&mut rocket::routes![
        secrets::get_secrets,
        secrets::create_secret,
        secrets::update_secret,
        secrets::delete_secret,
    ]);
    }

    rocket::custom(config)
        .manage(pool)
        .manage(push_channels)
        .manage(healer_state)
        .manage(pg_healer_store)
        .manage(federation_push)
        .manage(skill_center_cache)
        .mount("/api", api_routes)
        .mount(
            "/",
            SwaggerUi::new("/api/swagger-ui/<_..>").url("/api/openapi.json", ApiDoc::openapi()),
        )
}
