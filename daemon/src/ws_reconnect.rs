use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::http::Uri;
use tokio_tungstenite::tungstenite::Message;

pub struct WsClientConfig {
    pub url: String,
    pub auth_token: String,
    pub min_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for WsClientConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            auth_token: String::new(),
            min_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
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

        loop {
            tracing::info!("ws_reconnect: connecting to {}", config.url);
            match connect_and_run(&config, &incoming_tx, &mut outgoing_rx).await {
                Ok(()) => {
                    // Clean close — reset backoff and reconnect
                    tracing::info!("ws_reconnect: connection closed cleanly, reconnecting");
                    backoff = config.min_backoff;
                }
                Err(e) => {
                    tracing::warn!(
                        "ws_reconnect: connection error: {e:#}, reconnecting in {backoff:?}"
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
) -> Result<()> {
    let host = extract_host(&config.url)?;
    let ws_url = to_ws_scheme(&config.url);

    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(ws_url.parse::<Uri>()?)
        .header("Authorization", format!("Bearer {}", config.auth_token))
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        )
        .header("Sec-WebSocket-Version", "13")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Host", &host)
        .body(())?;

    let (ws, _) = tokio_tungstenite::connect_async(request)
        .await
        .context("WebSocket connect failed")?;

    tracing::info!("ws_reconnect: connected");

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
                            tracing::info!("ws_reconnect: receiver dropped, stopping");
                            return Ok(());
                        }
                    }
                    Message::Close(_) => {
                        tracing::info!("ws_reconnect: received close frame");
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

/// Normalize a URL to use a WebSocket scheme. `https://` → `wss://`,
/// `http://` → `ws://`, existing `ws(s)://` are left untouched.
pub fn to_ws_scheme(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        url.to_string()
    }
}

pub fn extract_host(url: &str) -> Result<String> {
    let url = url
        .strip_prefix("wss://")
        .or_else(|| url.strip_prefix("ws://"))
        .or_else(|| url.strip_prefix("https://"))
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host = url.split('/').next().unwrap_or(url);
    Ok(host.to_string())
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

        // Verify doubling
        backoff = (backoff * 2).min(max);
        assert_eq!(backoff, Duration::from_secs(2));

        backoff = (backoff * 2).min(max);
        assert_eq!(backoff, Duration::from_secs(4));

        backoff = (backoff * 2).min(max);
        assert_eq!(backoff, Duration::from_secs(8));

        // Jump to near max
        backoff = Duration::from_secs(32);
        backoff = (backoff * 2).min(max);
        assert_eq!(backoff, Duration::from_secs(60)); // capped

        // Already at max stays at max
        backoff = (backoff * 2).min(max);
        assert_eq!(backoff, Duration::from_secs(60));
    }

    #[test]
    fn extract_host_variants() {
        assert_eq!(
            extract_host("wss://relay.example.com/path").unwrap(),
            "relay.example.com"
        );
        assert_eq!(
            extract_host("ws://localhost:8080/ws").unwrap(),
            "localhost:8080"
        );
        assert_eq!(
            extract_host("https://api.example.com").unwrap(),
            "api.example.com"
        );
        assert_eq!(
            extract_host("http://127.0.0.1:3000").unwrap(),
            "127.0.0.1:3000"
        );
        assert_eq!(
            extract_host("bare.host.com/path").unwrap(),
            "bare.host.com"
        );
    }
}
