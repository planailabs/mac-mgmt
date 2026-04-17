use anyhow::{Context, Result};

/// The concrete WebSocket stream type returned by client connections.
pub type ClientWs = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

/// Builder for WebSocket client connections.
///
/// ```no_run
/// # async fn example() -> anyhow::Result<()> {
/// let ws = mac_mgmt_ws::WsConnect::new("wss://relay.example.com/api/daemon/register")
///     .bearer_auth("my-token")
///     .connect()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct WsConnect {
    url: String,
    headers: Vec<(String, String)>,
}

impl WsConnect {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: Vec::new(),
        }
    }

    /// Add a `Authorization: Bearer <token>` header.
    pub fn bearer_auth(self, token: impl AsRef<str>) -> Self {
        self.header("Authorization", format!("Bearer {}", token.as_ref()))
    }

    /// Add a custom header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Open the WebSocket connection.
    pub async fn connect(self) -> Result<ClientWs> {
        use tokio_tungstenite::tungstenite::{
            handshake::client::generate_key,
            http::{Request, Uri},
        };

        let uri: Uri = self.url.parse().context("invalid WebSocket URL")?;
        let host = extract_host(&self.url);

        let mut builder = Request::builder()
            .uri(uri)
            .header("Sec-WebSocket-Key", generate_key())
            .header("Sec-WebSocket-Version", "13")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Host", &host);

        for (name, value) in &self.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }

        let request = builder.body(()).context("failed to build WS request")?;
        let (ws, _response) = tokio_tungstenite::connect_async(request)
            .await
            .context("WebSocket connect failed")?;
        Ok(ws)
    }
}

/// Extract the host (with optional port) from a ws:// or wss:// URL.
pub(crate) fn extract_host(url: &str) -> String {
    let url = url
        .strip_prefix("wss://")
        .or_else(|| url.strip_prefix("ws://"))
        .unwrap_or(url);
    url.split('/').next().unwrap_or(url).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_host_variants() {
        assert_eq!(extract_host("wss://relay.example.com/path"), "relay.example.com");
        assert_eq!(extract_host("ws://localhost:8080/ws"), "localhost:8080");
        assert_eq!(extract_host("relay.example.com/path"), "relay.example.com");
    }
}
