mod auth;
pub mod push;
mod routes;

use rocket::config::Shutdown;
use rocket::Config;
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
        routes::get_skills,
        routes::get_mcp_servers,
        // Setting — config
        routes::setting_config_schema,
        routes::setting_get_config,
        routes::setting_set_config,
        // Setting — skills
        routes::setting_list_skills,
        routes::setting_add_skill,
        routes::setting_remove_skill,
        // Setting — bundles
        routes::setting_list_bundles,
        routes::setting_add_bundle,
        routes::setting_remove_bundle,
        // Setting — MCP servers
        routes::setting_list_mcp_servers,
        routes::setting_add_mcp_server,
        routes::setting_remove_mcp_server,
        // Setting — MCP bundles
        routes::setting_list_mcp_bundles,
        routes::setting_add_mcp_bundle,
        routes::setting_remove_mcp_bundle,
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
        // Sync — Heartbeat
        routes::post_heartbeat,
        // Admin
        routes::admin_list_customers,
        routes::admin_create_token,
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
    ),
    components(schemas(
        routes::SelfInfo,
        routes::SetConfigBody,
        routes::AddSkillBody,
        routes::CustomerBundleRow,
        routes::AddBundleBody,
        routes::AddMcpServerBody,
        routes::McpServerOptionRow,
        routes::CustomerMcpBundleRow,
        routes::AddMcpBundleBody,
        routes::SkillChannelRow,
        routes::BundleSkillChannelRow,
        routes::BundleMcpServerRow,
        routes::OptionRow,
        routes::Catalog,
        routes::CatalogBundle,
        routes::CatalogMcpBundle,
        routes::AdminCustomerRow,
        routes::CreateTokenForCustomerBody,
        routes::CreatedToken,
        routes::SshKeyRow,
        routes::AddSshKeyBody,
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

pub fn build_rocket(pool: PgPool, port: u16, push_channels: push::PushChannels) -> rocket::Rocket<rocket::Build> {
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

    rocket::custom(config)
        .manage(pool)
        .manage(push_channels)
        .mount(
            "/api",
            rocket::routes![
                // Common routes (any token kind)
                routes::get_self,
                // Sync token routes
                routes::get_config,
                routes::get_update_target,
                routes::get_skills,
                routes::get_mcp_servers,
                // Setting token routes — config
                routes::setting_config_schema,
                routes::setting_get_config,
                routes::setting_set_config,
                // Setting token routes — skills
                routes::setting_list_skills,
                routes::setting_add_skill,
                routes::setting_remove_skill,
                // Setting token routes — bundles
                routes::setting_list_bundles,
                routes::setting_add_bundle,
                routes::setting_remove_bundle,
                // Setting token routes — MCP servers
                routes::setting_list_mcp_servers,
                routes::setting_add_mcp_server,
                routes::setting_remove_mcp_server,
                // Setting token routes — MCP bundles
                routes::setting_list_mcp_bundles,
                routes::setting_add_mcp_bundle,
                routes::setting_remove_mcp_bundle,
                // Setting token routes — available resources
                routes::setting_available_skill_channels,
                routes::setting_available_bundles,
                routes::setting_available_mcp_servers,
                routes::setting_available_mcp_bundles,
                // Setting token routes — bundle contents
                routes::setting_bundle_skills,
                routes::setting_mcp_bundle_servers,
                // Setting token routes — catalog
                routes::setting_catalog,
                // Sync token routes — SSH keys
                routes::get_ssh_keys,
                // Setting token routes — SSH keys
                routes::setting_list_ssh_keys,
                routes::setting_add_ssh_key,
                routes::setting_remove_ssh_key,
                // Admin token routes
                routes::admin_list_customers,
                routes::admin_create_token,
                // Admin — skill MCP dependencies
                routes::admin_list_skill_mcp_deps,
                routes::admin_add_skill_mcp_dep,
                routes::admin_remove_skill_mcp_dep,
                // Sync — Heartbeat
                routes::post_heartbeat,
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
                // SSE push
                push::sse_events,
            ],
        )
        .mount(
            "/",
            SwaggerUi::new("/api/swagger-ui/<_..>")
                .url("/api/openapi.json", ApiDoc::openapi()),
        )
}
