use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct StatusResponse {
    version: String,
    uptime_secs: u64,
    services: Vec<ServiceStatus>,
}

#[derive(Deserialize)]
struct ServiceStatus {
    name: String,
    healthy: bool,
    upgrade_pending: bool,
    busy: bool,
}

fn format_uptime(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

pub async fn print_status(port: Option<u16>) -> Result<()> {
    let port = match port {
        Some(p) => p,
        None => {
            // Try to read port from config file
            let config_path = crate::config::config_path();
            if config_path.exists() {
                let contents = std::fs::read_to_string(&config_path).unwrap_or_default();
                let val: toml::Value = toml::from_str(&contents)
                    .unwrap_or(toml::Value::Table(Default::default()));
                val.get("metrics")
                    .and_then(|m| m.get("port"))
                    .and_then(|p| p.as_integer())
                    .map(|p| p as u16)
                    .unwrap_or(9396)
            } else {
                9396
            }
        }
    };
    let url = format!("http://[::1]:{port}/status");

    let resp = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .local_address(std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST))
        .build()
        .context("failed to build HTTP client")?
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                anyhow::anyhow!("daemon not running or metrics port differs")
            } else {
                anyhow::anyhow!("{e}")
            }
        })?;

    let status: StatusResponse = resp
        .json()
        .await
        .context("failed to parse status response")?;

    println!(
        "mac-mgmt v{} (up {})\n",
        status.version,
        format_uptime(status.uptime_secs)
    );

    if status.services.is_empty() {
        println!("No services managed.");
        return Ok(());
    }

    println!(
        "{:<12} {:<9} {:<9} {}",
        "SERVICE", "HEALTHY", "UPGRADE", "BUSY"
    );
    for svc in &status.services {
        let healthy = if svc.healthy { "yes" } else { "NO" };
        let upgrade = if svc.upgrade_pending { "pending" } else { "-" };
        let busy = if svc.busy { "yes" } else { "-" };
        println!("{:<12} {:<9} {:<9} {}", svc.name, healthy, upgrade, busy);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_uptime_hours_and_minutes() {
        assert_eq!(format_uptime(3661), "1h 1m");
        assert_eq!(format_uptime(3600), "1h 0m");
        assert_eq!(format_uptime(7200), "2h 0m");
    }

    #[test]
    fn format_uptime_minutes_only() {
        assert_eq!(format_uptime(120), "2m");
        assert_eq!(format_uptime(59), "0m");
        assert_eq!(format_uptime(0), "0m");
    }

    #[test]
    fn parse_status_json() {
        let json = r#"{
            "version": "0.1.5",
            "uptime_secs": 3600,
            "services": [
                {"name": "openclaw", "healthy": true, "upgrade_pending": false, "busy": false},
                {"name": "ollama", "healthy": false, "upgrade_pending": true, "busy": true}
            ]
        }"#;
        let status: StatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(status.version, "0.1.5");
        assert_eq!(status.uptime_secs, 3600);
        assert_eq!(status.services.len(), 2);
        assert!(status.services[0].healthy);
        assert!(!status.services[1].healthy);
        assert!(status.services[1].upgrade_pending);
        assert!(status.services[1].busy);
    }

    #[tokio::test]
    async fn connection_refused_gives_helpful_error() {
        let result = print_status(Some(19999)).await; // unlikely to be in use
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("daemon not running"),
            "got: {err}"
        );
    }
}
