use rocket::{get, routes, State};
use std::sync::Arc;

use crate::metrics::Metrics;

#[get("/metrics")]
fn metrics_endpoint(metrics: &State<Arc<Metrics>>) -> String {
    metrics.render()
}

pub fn build_rocket(metrics: Arc<Metrics>, port: u16) -> rocket::Rocket<rocket::Build> {
    let config = rocket::Config {
        port,
        address: std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
        log_level: rocket::config::LogLevel::Off,
        ..rocket::Config::default()
    };

    rocket::custom(config)
        .manage(metrics)
        .mount("/", routes![metrics_endpoint])
}
