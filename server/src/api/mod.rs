mod auth;
mod routes;

use rocket::config::Shutdown;
use rocket::Config;
use sqlx::PgPool;

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
}
