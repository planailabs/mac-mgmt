use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::ws_reconnect::{self, WsClientConfig};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PushCommand {
    SyncConfig,
    SyncSkills,
    SyncMcpServers,
    SyncSshKeys,
}

pub fn start(
    server_url: &str,
    server_token: &str,
) -> (JoinHandle<()>, mpsc::Receiver<PushCommand>) {
    let (raw_tx, mut raw_rx) = mpsc::channel::<String>(16);
    let (cmd_tx, cmd_rx) = mpsc::channel::<PushCommand>(16);

    let ws_url = format!(
        "{}/api/ws?token={}",
        server_url.replace("https://", "wss://").replace("http://", "ws://"),
        server_token
    );

    let ws_config = WsClientConfig {
        url: ws_url,
        auth_token: server_token.to_string(),
        ..Default::default()
    };

    let _ws_handle = ws_reconnect::spawn_reconnecting(ws_config, raw_tx);

    let handle = tokio::spawn(async move {
        while let Some(text) = raw_rx.recv().await {
            match serde_json::from_str::<PushCommand>(&text) {
                Ok(cmd) => {
                    if cmd_tx.send(cmd).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("unknown server push message: {e} — {text}");
                }
            }
        }
    });

    (handle, cmd_rx)
}
