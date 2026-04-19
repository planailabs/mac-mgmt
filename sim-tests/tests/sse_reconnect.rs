//! Test: SSE push delivery and reconnection behaviour.
//!
//! Validates that daemons receive push commands via SSE and that
//! push events trigger the expected daemon behaviour.

use std::time::Duration;

fn init_tracing() {
    let _ = tracing_subscriber::fmt().with_env_filter("warn").try_init();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sse_push_triggers_skills_sync() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, _instance_id) = sim_tests::start_sim_daemon(addr).await;

    // Wait for daemon to connect to SSE
    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.request_count("/api/events") > 0
    })
    .await;
    assert!(ok, "daemon should connect to SSE endpoint");

    let skills_before = state.request_count("/api/skills");

    // Push SyncSkills via SSE
    state.push(mac_mgmt_common::PushEvent::SyncSkills);

    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.request_count("/api/skills") > skills_before
    })
    .await;
    assert!(ok, "daemon should fetch /api/skills after SyncSkills push");

    let _ = shutdown_tx.send(());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sse_push_triggers_mcp_sync() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, _instance_id) = sim_tests::start_sim_daemon(addr).await;

    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.request_count("/api/events") > 0
    })
    .await;
    assert!(ok, "daemon should connect to SSE endpoint");

    let mcp_before = state.request_count("/api/mcp-servers");
    state.push(mac_mgmt_common::PushEvent::SyncMcpServers);

    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.request_count("/api/mcp-servers") > mcp_before
    })
    .await;
    assert!(
        ok,
        "daemon should fetch /api/mcp-servers after SyncMcpServers push"
    );

    let _ = shutdown_tx.send(());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sse_push_triggers_config_reload() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, instance_id) = sim_tests::start_sim_daemon(addr).await;

    // Wait for SSE connection and first heartbeat
    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.request_count("/api/events") > 0 && !state.heartbeats_from(&instance_id).is_empty()
    })
    .await;
    assert!(ok, "daemon should connect to SSE and send heartbeat");

    // Push SyncConfig
    state.push(mac_mgmt_common::PushEvent::SyncConfig);
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Daemon should still be alive and sending heartbeats after config push
    let hb_count = state.heartbeats_from(&instance_id).len();
    tokio::time::sleep(Duration::from_secs(3)).await;
    let hb_count_after = state.heartbeats_from(&instance_id).len();
    assert!(
        hb_count_after > hb_count,
        "daemon should still send heartbeats after SyncConfig push (before={hb_count}, after={hb_count_after})"
    );

    let _ = shutdown_tx.send(());
}
