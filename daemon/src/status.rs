use anyhow::{Context, Result};
use mac_mgmt_common::StatusResponse;

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
    let port = port.unwrap_or_else(crate::config::read_metrics_port);
    let url = format!("http://[::1]:{port}/status");

    let resp = crate::local_client::build()?
        .get(&url)
        .send()
        .await
        .map_err(crate::local_client::map_connect_error)?;

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
        "{:<12} {:<10} {:<9} {:<9} {}",
        "SERVICE", "PHASE", "UPGRADE", "BUSY", "HEALTHY"
    );
    for svc in &status.services {
        let phase = if svc.phase.is_empty() {
            if svc.healthy { "healthy" } else { "unhealthy" }
        } else {
            &svc.phase
        };
        let upgrade = if svc.upgrade_pending { "pending" } else { "-" };
        let busy = if svc.busy { "yes" } else { "-" };
        let healthy = if svc.healthy { "yes" } else { "NO" };
        println!(
            "{:<12} {:<10} {:<9} {:<9} {}",
            svc.name, phase, upgrade, busy, healthy
        );
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
        let _ = rustls::crypto::ring::default_provider().install_default();
        // In sandboxed builds (e.g. Nix), binding to IPv6 loopback may fail
        // at client construction time — skip if we can't even build the client.
        if crate::local_client::build().is_err() {
            return;
        }
        let result = print_status(Some(19999)).await; // unlikely to be in use
        let err = result.unwrap_err();
        assert!(err.to_string().contains("daemon not running"), "got: {err}");
    }
}
