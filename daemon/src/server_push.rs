use std::time::Duration;

use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PushCommand {
    SyncConfig,
    SyncSkills,
    SyncMcpServers,
    SyncSshKeys,
}

const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(60);

/// Connect to the server's SSE endpoint and forward push commands.
/// Reconnects with exponential backoff on failure.
pub fn start(
    server_url: &str,
    server_token: &str,
) -> (JoinHandle<()>, mpsc::Receiver<PushCommand>) {
    let (cmd_tx, cmd_rx) = mpsc::channel(16);
    let url = format!("{server_url}/api/events?token={server_token}");

    let handle = tokio::spawn(async move {
        let mut backoff = MIN_BACKOFF;

        loop {
            tracing::info!("connecting to server SSE");
            match connect_sse(&url, &cmd_tx).await {
                Ok(()) => {
                    tracing::info!("SSE connection closed, reconnecting");
                    backoff = MIN_BACKOFF;
                }
                Err(e) => {
                    tracing::warn!("SSE connection failed: {e:#}, reconnecting in {backoff:?}");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                }
            }
        }
    });

    (handle, cmd_rx)
}

async fn connect_sse(url: &str, cmd_tx: &mpsc::Sender<PushCommand>) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()?;

    let mut resp = client.get(url).send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("SSE endpoint returned {}", resp.status());
    }

    // Read chunks and split into lines. SSE events are "data: {json}\n\n".
    let mut buffer = String::new();

    while let Some(chunk) = resp.chunk().await? {
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        // Process complete lines
        while let Some(newline_pos) = buffer.find('\n') {
            let line: String = buffer.drain(..=newline_pos).collect();
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            if let Some(data) = trimmed.strip_prefix("data:") {
                let data = data.trim();
                match serde_json::from_str::<PushCommand>(data) {
                    Ok(cmd) => {
                        if cmd_tx.send(cmd).await.is_err() {
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        tracing::warn!("unknown SSE message: {e} — {data}");
                    }
                }
            }
        }
    }

    Ok(())
}
