use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use crate::WsConnect;

pub struct WsClientConfig {
    pub url: String,
    pub auth_token: String,
    pub min_backoff: Duration,
    pub max_backoff: Duration,
    /// Human-readable label for log messages (e.g. "relay", "ws").
    pub label: String,
}

impl Default for WsClientConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            auth_token: String::new(),
            min_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            label: "ws".to_string(),
        }
    }
}

/// Spawn a task that maintains a reconnecting WebSocket connection.
///
/// Text messages received on the WebSocket are forwarded to `incoming_tx`.
/// Messages received from `outgoing_rx` are sent to the WebSocket.
/// On clean close (Close frame), reconnects with reset backoff.
/// On error, reconnects with exponential backoff (doubles each time, capped at max_backoff).
pub fn spawn_reconnecting(
    config: WsClientConfig,
    incoming_tx: mpsc::Sender<String>,
    mut outgoing_rx: mpsc::Receiver<String>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut backoff = config.min_backoff;
        let label = &config.label;

        loop {
            tracing::info!("{label}: connecting to {}", config.url);
            match connect_and_run(&config, &incoming_tx, &mut outgoing_rx).await {
                Ok(()) => {
                    tracing::info!("{label}: disconnected, reconnecting");
                    backoff = config.min_backoff;
                }
                Err(e) => {
                    tracing::warn!(
                        "{label}: connection failed: {e:#}, reconnecting in {backoff:?}"
                    );
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(config.max_backoff);
                }
            }
        }
    })
}

async fn connect_and_run(
    config: &WsClientConfig,
    incoming_tx: &mpsc::Sender<String>,
    outgoing_rx: &mut mpsc::Receiver<String>,
) -> anyhow::Result<()> {
    let ws = WsConnect::new(&config.url)
        .bearer_auth(&config.auth_token)
        .connect()
        .await?;

    tracing::info!("{}: connected", config.label);

    let (mut ws_sink, mut ws_stream) = ws.split();

    loop {
        tokio::select! {
            msg = ws_stream.next() => {
                let msg = match msg {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => anyhow::bail!("WS read error: {e}"),
                    None => anyhow::bail!("WebSocket stream ended unexpectedly"),
                };
                match msg {
                    Message::Text(text) => {
                        if incoming_tx.send(text.to_string()).await.is_err() {
                            tracing::info!("{}: receiver dropped, stopping", config.label);
                            return Ok(());
                        }
                    }
                    Message::Close(_) => {
                        tracing::info!("{}: received close frame", config.label);
                        return Ok(());
                    }
                    _ => {}
                }
            }
            Some(text) = outgoing_rx.recv() => {
                ws_sink.send(Message::Text(text.into()))
                    .await
                    .context("WS send error")?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_backoff() {
        let config = WsClientConfig::default();
        assert_eq!(config.min_backoff, Duration::from_secs(1));
        assert_eq!(config.max_backoff, Duration::from_secs(60));
    }

    #[test]
    fn backoff_doubles_and_caps() {
        let min = Duration::from_secs(1);
        let max = Duration::from_secs(60);
        let mut backoff = min;

        backoff = (backoff * 2).min(max);
        assert_eq!(backoff, Duration::from_secs(2));

        backoff = (backoff * 2).min(max);
        assert_eq!(backoff, Duration::from_secs(4));

        backoff = (backoff * 2).min(max);
        assert_eq!(backoff, Duration::from_secs(8));

        backoff = Duration::from_secs(32);
        backoff = (backoff * 2).min(max);
        assert_eq!(backoff, Duration::from_secs(60));

        backoff = (backoff * 2).min(max);
        assert_eq!(backoff, Duration::from_secs(60));
    }
}
