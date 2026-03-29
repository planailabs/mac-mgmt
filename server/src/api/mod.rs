mod auth;
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
        description = "API for daemon sync (sync tokens) and resource management (setting tokens).",
    ),
    paths(
        routes::get_self,
        // Sync
        routes::get_config,
        routes::get_skills,
        routes::get_mcp_servers,
        // Setting — config
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
    ),
    components(schemas(
        routes::SelfInfo,
        routes::McpServerEntry,
        routes::SetConfigBody,
        routes::CustomerSkillRow,
        routes::AddSkillBody,
        routes::CustomerBundleRow,
        routes::AddBundleBody,
        routes::CustomerMcpServerRow,
        routes::AddMcpServerBody,
        routes::CustomerMcpBundleRow,
        routes::AddMcpBundleBody,
        routes::SkillChannelRow,
        routes::OptionRow,
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
                            "Sync token or setting token. Token kind determines which endpoints are accessible.",
                        ))
                        .build(),
                ),
            );
        }
    }
}

pub fn build_rocket(pool: PgPool, port: u16) -> rocket::Rocket<rocket::Build> {
    let config = Config {
        port,
        address: std::net::Ipv4Addr::UNSPECIFIED.into(),
        shutdown: Shutdown {
            grace: 2,
            mercy: 2,
            ..Shutdown::default()
        },
        ..Config::default()
    };

    rocket::custom(config)
        .manage(pool)
        .mount(
            "/api",
            rocket::routes![
                // Common routes (any token kind)
                routes::get_self,
                // Sync token routes
                routes::get_config,
                routes::get_skills,
                routes::get_mcp_servers,
                // Setting token routes — config
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
            ],
        )
        .mount(
            "/",
            SwaggerUi::new("/api/swagger-ui/<_..>")
                .url("/api/openapi.json", ApiDoc::openapi()),
        )
}
