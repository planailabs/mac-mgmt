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
        .mount("/api", rocket::routes![routes::get_config, routes::get_skills])
}
