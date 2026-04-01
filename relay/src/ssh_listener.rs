use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::bridge;
use crate::daemon_registry::{ControlMsg, DaemonRegistry};

/// Spawn a TCP listener on the given port. For each SSH client connection,
/// send a session request to the daemon and wait for the data channel.
pub fn spawn(
    registry: Arc<DaemonRegistry>,
    instance_id: String,
    port: u16,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = listen(registry, &instance_id, port).await {
            tracing::error!("SSH listener on port {port} failed: {e:#}");
        }
    })
}

async fn listen(
    registry: Arc<DaemonRegistry>,
    instance_id: &str,
    port: u16,
) -> Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!("SSH listener started on port {port} for {instance_id}");

    loop {
        let (tcp_stream, peer_addr) = listener.accept().await?;
        tracing::info!("SSH client connected from {peer_addr} on port {port}");

        let session_id = uuid::Uuid::new_v4().to_string();

        // Send session request to daemon
        let control_tx = match registry.get_control_tx(instance_id) {
            Some(tx) => tx,
            None => {
                tracing::warn!("daemon {instance_id} not found, dropping connection");
                continue;
            }
        };

        if control_tx
            .send(ControlMsg::SessionRequest {
                session_id: session_id.clone(),
            })
            .await
            .is_err()
        {
            tracing::warn!("failed to send session request to daemon {instance_id}");
            continue;
        }

        // Register pending session and spawn bridge when data channel arrives
        bridge::register_pending_session(session_id, tcp_stream);
    }
}
