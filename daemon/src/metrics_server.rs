use rocket::serde::json::Json;
use rocket::{get, routes, State};
use serde::Serialize;
use std::sync::Arc;

use crate::log_buffer::LogBuffer;
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

#[derive(Serialize)]
struct LogsResponse {
    lines: Vec<String>,
    /// Current buffer index (pass as `after` to get new lines)
    index: usize,
}

#[get("/logs?<n>&<service>&<after>")]
fn logs_endpoint(
    log_buf: &State<LogBuffer>,
    n: Option<usize>,
    service: Option<&str>,
    after: Option<usize>,
) -> Json<LogsResponse> {
    let (lines, index) = if let Some(after) = after {
        let (idx, lines) = log_buf.since(after);
        (lines, idx)
    } else {
        let count = n.unwrap_or(50);
        let lines = log_buf.tail(count);
        let idx = log_buf.all().len();
        (lines, idx)
    };

    // Filter by service if specified
    let lines = if let Some(svc) = service {
        let prefix = format!("[{svc}]");
        lines
            .into_iter()
            .filter(|l| l.contains(&prefix))
            .collect()
    } else {
        lines
    };

    Json(LogsResponse { lines, index })
}

pub fn build_rocket(
    metrics: Arc<Metrics>,
    log_buf: LogBuffer,
    port: u16,
) -> rocket::Rocket<rocket::Build> {
    let config = rocket::Config {
        port,
        address: std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
        log_level: rocket::config::LogLevel::Off,
        ..rocket::Config::default()
    };

    rocket::custom(config)
        .manage(metrics)
        .manage(log_buf)
        .mount(
            "/",
            routes![metrics_endpoint, status_endpoint, logs_endpoint],
        )
}
