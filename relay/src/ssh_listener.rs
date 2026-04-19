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

async fn listen(registry: Arc<DaemonRegistry>, instance_id: &str, port: u16) -> Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!("SSH listener started on port {port} for {instance_id}");

    loop {
        let (tcp_stream, peer_addr) = listener.accept().await?;
        let session_id = uuid::Uuid::new_v4().to_string();
        let session_secret = uuid::Uuid::new_v4().to_string();
        tracing::info!(
            "SSH client {peer_addr} -> {instance_id} (port {port}), session {session_id}"
        );

        let Some(control_tx) = registry.get_control_tx(instance_id) else {
            tracing::warn!("daemon {instance_id} not in registry, dropping client {peer_addr}");
            continue;
        };

        if control_tx
            .send(ControlMsg::SessionRequest {
                session_id: session_id.clone(),
                session_secret: session_secret.clone(),
            })
            .await
            .is_err()
        {
            tracing::warn!("control channel closed for daemon {instance_id}, dropping {peer_addr}");
            continue;
        }
        tracing::debug!("session request {session_id} sent to daemon {instance_id}");

        // Register pending session and spawn bridge when data channel arrives
        bridge::register_pending_session(session_id, session_secret, tcp_stream);
    }
}
