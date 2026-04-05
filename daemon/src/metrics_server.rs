use rocket::serde::json::Json;
use rocket::{get, routes, State};
use serde::Serialize;
use std::sync::Arc;

use crate::metrics::Metrics;

#[derive(Serialize)]
struct ServiceStatus {
    name: String,
    healthy: bool,
    upgrade_pending: bool,
    busy: bool,
}

#[derive(Serialize)]
struct StatusResponse {
    version: String,
    uptime_secs: u64,
    services: Vec<ServiceStatus>,
}

#[get("/metrics")]
fn metrics_endpoint(metrics: &State<Arc<Metrics>>) -> String {
    metrics.render()
}

#[get("/status")]
fn status_endpoint(metrics: &State<Arc<Metrics>>) -> Json<StatusResponse> {
    let (version, uptime_secs, svc_list) = metrics.status();

    let services = svc_list
        .into_iter()
        .map(|(name, healthy, upgrade_pending, busy)| ServiceStatus {
            name,
            healthy,
            upgrade_pending,
            busy,
        })
        .collect();

    Json(StatusResponse {
        version,
        uptime_secs,
        services,
    })
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
        .mount("/", routes![metrics_endpoint, status_endpoint])
}
