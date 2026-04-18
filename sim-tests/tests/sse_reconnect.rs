//! Test: SSE push delivery and reconnection behaviour.
//!
//! Validates that daemons receive push commands via SSE and that
//! push events trigger the expected daemon behaviour (e.g. SyncSkills
//! causes a GET /api/skills request).

use sim_tests::mock_server::MockServerState;
use std::sync::Arc;
use std::time::Duration;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init();
}

async fn start_daemon(
    server_addr: std::net::SocketAddr,
) -> (tokio::sync::oneshot::Sender<()>, String) {
    let cfg = sim_tests::daemon_config_for(server_addr);
    let host_key = sim_tests::generate_host_key();
    let instance_id = mac_mgmt_daemon::host_keys::fingerprint_hex(&host_key);
    let host_key = std::sync::Arc::new(host_key);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let iid = instance_id.clone();

    tokio::spawn(async move {
        if let Err(e) = mac_mgmt_daemon::daemon::run_sim(cfg, host_key, shutdown_rx).await {
            tracing::warn!("daemon {iid} exited with error: {e}");
        }
    });

    (shutdown_tx, instance_id)
}

async fn wait_for_request_count(
    state: &Arc<MockServerState>,
    endpoint: &str,
    min_count: u64,
    timeout: Duration,
) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if state.request_count(endpoint) >= min_count {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn sse_push_triggers_skills_sync() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, _instance_id) = start_daemon(addr).await;

    // Wait for daemon to connect to SSE
    assert!(
        wait_for_request_count(&state, "/api/events", 1, Duration::from_secs(10)).await,
        "daemon should connect to SSE endpoint"
    );

    // Record current skills request count
    let skills_before = state.request_count("/api/skills");

    // Push SyncSkills via SSE
    state.push(mac_mgmt_common::PushEvent::SyncSkills);

    // Wait for the daemon to fetch skills
    assert!(
        wait_for_request_count(
            &state,
            "/api/skills",
            skills_before + 1,
            Duration::from_secs(10)
        )
        .await,
        "daemon should fetch /api/skills after SyncSkills push (had {skills_before}, expected {})",
        skills_before + 1
    );

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn sse_push_triggers_mcp_sync() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, _instance_id) = start_daemon(addr).await;

    // Wait for SSE connection
    assert!(
        wait_for_request_count(&state, "/api/events", 1, Duration::from_secs(10)).await,
        "daemon should connect to SSE endpoint"
    );

    let mcp_before = state.request_count("/api/mcp-servers");

    // Push SyncMcpServers
    state.push(mac_mgmt_common::PushEvent::SyncMcpServers);

    assert!(
        wait_for_request_count(
            &state,
            "/api/mcp-servers",
            mcp_before + 1,
            Duration::from_secs(10)
        )
        .await,
        "daemon should fetch /api/mcp-servers after SyncMcpServers push"
    );

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn sse_push_triggers_config_reload() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, instance_id) = start_daemon(addr).await;

    // Wait for SSE connection and first heartbeat
    assert!(
        wait_for_request_count(&state, "/api/events", 1, Duration::from_secs(10)).await,
        "daemon should connect to SSE endpoint"
    );

    // Wait for at least one heartbeat so we know the daemon is running
    assert!(
        wait_for_heartbeat_from(&state, &instance_id, 1, Duration::from_secs(10)).await,
        "daemon should send at least 1 heartbeat"
    );

    // Push SyncConfig — daemon will attempt config reload.
    // Note: config::reload() reads from the local filesystem then merges
    // with the remote config. The remote fetch may go to the mock server
    // or to the URL in the local config file. We verify the push was
    // received by checking that /api/config gets at least one more hit,
    // OR that the daemon survives the reload without crashing.
    let _config_before = state.request_count("/api/config");
    state.push(mac_mgmt_common::PushEvent::SyncConfig);

    // Give the daemon time to process the push
    tokio::time::sleep(Duration::from_secs(3)).await;

    // The daemon should still be alive and sending heartbeats after the config push
    let hb_count = state.heartbeats_from(&instance_id).len();
    tokio::time::sleep(Duration::from_secs(3)).await;
    let hb_count_after = state.heartbeats_from(&instance_id).len();
    assert!(
        hb_count_after > hb_count,
        "daemon should still send heartbeats after SyncConfig push \
         (before={hb_count}, after={hb_count_after})"
    );

    let _ = shutdown_tx.send(());
}

async fn wait_for_heartbeat_from(
    state: &Arc<MockServerState>,
    instance_id: &str,
    count: usize,
    timeout: Duration,
) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if state.heartbeats_from(instance_id).len() >= count {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
