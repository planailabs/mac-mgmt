use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct LogsResponse {
    lines: Vec<String>,
    index: usize,
}

pub async fn tail_logs(
    service: Option<&str>,
    lines: usize,
    follow: bool,
    port_override: Option<u16>,
) -> Result<()> {
    let port = port_override.unwrap_or_else(crate::config::read_metrics_port);

    let client = crate::local_client::build()?;

    let mut url = format!("http://[::1]:{port}/logs?n={lines}");
    if let Some(svc) = service {
        url.push_str(&format!("&service={svc}"));
    }

    let resp: LogsResponse = client
        .get(&url)
        .send()
        .await
        .map_err(crate::local_client::map_connect_error)?
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

        let mut poll_url = format!("http://[::1]:{port}/logs?after={index}");
        if let Some(svc) = service {
            poll_url.push_str(&format!("&service={svc}"));
        }

        let Ok(r) = client.get(&poll_url).send().await else {
            continue;
        };
        let Ok(resp) = r.json::<LogsResponse>().await else {
            continue;
        };

        for line in &resp.lines {
            println!("{line}");
        }
        index = resp.index;
    }
}

pub async fn trigger_sync(port_override: Option<u16>) -> Result<()> {
    let port = port_override.unwrap_or_else(crate::config::read_metrics_port);

    let client = crate::local_client::build()?;

    let url = format!("http://[::1]:{port}/sync");

    let resp = client
        .post(&url)
        .send()
        .await
        .map_err(crate::local_client::map_connect_error)?;

    if !resp.status().is_success() {
        anyhow::bail!("sync request failed: {}", resp.status());
    }

    println!("sync triggered");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn resolve_port_default() {
        let port = crate::config::read_metrics_port();
        assert!(port > 0);
    }
}
