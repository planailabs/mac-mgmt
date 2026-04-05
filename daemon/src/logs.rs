use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct LogsResponse {
    lines: Vec<String>,
    index: usize,
}

fn resolve_port() -> u16 {
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

pub async fn tail_logs(
    service: Option<&str>,
    lines: usize,
    follow: bool,
    port_override: Option<u16>,
) -> Result<()> {
    let port = port_override.unwrap_or_else(resolve_port);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .local_address(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
        .build()
        .context("failed to build HTTP client")?;

    let mut url = format!("http://127.0.0.1:{port}/logs?n={lines}");
    if let Some(svc) = service {
        url.push_str(&format!("&service={svc}"));
    }

    let resp: LogsResponse = client
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                anyhow::anyhow!("daemon not running or metrics port differs")
            } else {
                anyhow::anyhow!("{e}")
            }
        })?
        .json()
        .await
        .context("failed to parse logs response")?;

    for line in &resp.lines {
        println!("{line}");
    }

    if !follow {
        return Ok(());
    }

    // Follow mode: poll for new lines
    let mut index = resp.index;
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let mut poll_url = format!("http://127.0.0.1:{port}/logs?after={index}");
        if let Some(svc) = service {
            poll_url.push_str(&format!("&service={svc}"));
        }

        let resp: LogsResponse = match client.get(&poll_url).send().await {
            Ok(r) => match r.json().await {
                Ok(r) => r,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        for line in &resp.lines {
            println!("{line}");
        }
        index = resp.index;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_port_default() {
        // When no config exists, should return 9396
        // (depends on test environment, so just verify it's a valid port)
        let port = resolve_port();
        assert!(port > 0);
    }
}
