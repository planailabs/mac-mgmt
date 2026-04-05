use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

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

pub fn print_status(port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let mut stream =
        TcpStream::connect(&addr).context("daemon not running or metrics port differs")?;

    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;

    let request = format!(
        "GET /status HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes())?;

    let reader = BufReader::new(&stream);
    let mut lines: Vec<String> = Vec::new();
    for line in reader.lines() {
        match line {
            Ok(l) => lines.push(l),
            Err(_) => break,
        }
    }

    // Find the body (after the blank line)
    let body_start = lines
        .iter()
        .position(|l| l.is_empty())
        .context("invalid HTTP response")?;
    let body = lines[body_start + 1..].join("\n");

    let status: StatusResponse =
        serde_json::from_str(&body).context("failed to parse status response")?;

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

    #[test]
    fn connection_refused_gives_helpful_error() {
        let result = print_status(19999); // unlikely to be in use
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("daemon not running"),
            "got: {err}"
        );
    }
}
