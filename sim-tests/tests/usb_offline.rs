//! Hermetic integration test for the sovereign-AI USB build's control surface
//! and offline behaviour. No real network, no real nix — it drives the in-process
//! supervisor + the usb control API directly, plus a `run_stack` smoke test.
//!
//! Run with: `cargo test -p sim-tests --features usb`.
#![cfg(feature = "usb")]

use mac_mgmt_daemon::usb;
use mac_mgmt_services::protocol::SpawnSpec;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()))
        .try_init();
    // reqwest is built with `rustls-no-provider`; install the ring provider so
    // the HTTP client can be constructed (it builds a TLS connector eagerly).
    sim_tests::ensure_tls_provider();
}

/// Spawn an in-process supervisor on `socket` and wait until it accepts.
async fn start_supervisor(socket: &Path) {
    let sock = socket.to_path_buf();
    tokio::spawn(async move {
        let _ = mac_mgmt_services::server::run(&sock).await;
    });
    for _ in 0..50 {
        if mac_mgmt_services::Client::connect(socket, Duration::from_millis(200))
            .await
            .is_ok()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("supervisor did not come up");
}

async fn http_get_json(url: &str) -> serde_json::Value {
    reqwest::get(url).await.unwrap().json().await.unwrap()
}

async fn http_post(url: &str, body: serde_json::Value) -> reqwest::Response {
    reqwest::Client::new()
        .post(url)
        .json(&body)
        .send()
        .await
        .unwrap()
}

async fn http_post_put(url: &str, body: serde_json::Value) -> reqwest::Response {
    reqwest::Client::new()
        .put(url)
        .json(&body)
        .send()
        .await
        .unwrap()
}

/// The control API lists supervisor services, start/stop/restart work (offline-
/// safe), and install is refused (409) when offline.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn control_api_offline_lists_and_controls_services() {
    init_tracing();
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("services.sock");
    start_supervisor(&socket).await;

    // Register a trivial long-lived service (no nix needed).
    let mut client = mac_mgmt_services::Client::connect(&socket, Duration::from_secs(5))
        .await
        .unwrap();
    client
        .register(
            "test-svc",
            SpawnSpec {
                program: "sleep".into(),
                args: vec!["3600".into()],
                env: HashMap::new(),
            },
        )
        .await
        .unwrap();

    // Stand up the control server in offline mode.
    let (install_tx, _install_rx) = tokio::sync::mpsc::channel(4);
    let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);
    let cfg_write = tmp.path().join("config.json");
    let state = usb::control::ControlState {
        socket_path: socket.clone(),
        offline: true,
        install_tx,
        shutdown_tx,
        config_read: vec![cfg_write.clone()],
        config_write: cfg_write.clone(),
    };
    let listener = tokio::net::TcpListener::bind(("::1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = usb::control::router(state);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let base = format!("http://[::1]:{port}");

    // Give the service a moment to spawn.
    tokio::time::sleep(Duration::from_millis(300)).await;

    // GET /status — offline true, service present + running.
    let status = http_get_json(&format!("{base}/status")).await;
    assert_eq!(status["offline"], serde_json::json!(true), "offline flag");
    let svc = status["services"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "test-svc")
        .expect("test-svc listed");
    assert_eq!(svc["running"], serde_json::json!(true), "running initially");

    // Stop → not running.
    let r = http_post(&format!("{base}/usb/stop"), serde_json::json!({"name":"test-svc"})).await;
    assert!(r.status().is_success());
    tokio::time::sleep(Duration::from_millis(300)).await;
    let status = http_get_json(&format!("{base}/status")).await;
    let svc = status["services"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "test-svc")
        .unwrap();
    assert_eq!(svc["running"], serde_json::json!(false), "stopped");

    // Start → running again.
    let r = http_post(&format!("{base}/usb/start"), serde_json::json!({"name":"test-svc"})).await;
    assert!(r.status().is_success());
    tokio::time::sleep(Duration::from_millis(300)).await;
    let status = http_get_json(&format!("{base}/status")).await;
    let svc = status["services"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "test-svc")
        .unwrap();
    assert_eq!(svc["running"], serde_json::json!(true), "restarted");

    // Restart → still running.
    let r = http_post(&format!("{base}/usb/restart"), serde_json::json!({"name":"test-svc"})).await;
    assert!(r.status().is_success());

    // Install is refused offline (409, no network).
    let r = http_post(&format!("{base}/usb/install"), serde_json::json!({"name":"test-svc"})).await;
    assert_eq!(
        r.status().as_u16(),
        409,
        "install must be refused (conflict) when offline"
    );

    // ── Config editing (same method as mac-mgmt-server) ────────────────
    // GET /config returns the current config as JSON (defaults, none on disk).
    let cfg = http_get_json(&format!("{base}/config")).await;
    assert!(cfg.get("daemon").is_some(), "config JSON has a daemon section");

    // GET /config/schema returns a JSON Schema.
    let schema = http_get_json(&format!("{base}/config/schema")).await;
    assert!(
        schema.get("properties").is_some() || schema.get("$schema").is_some(),
        "schema looks like JSON Schema"
    );

    // PUT a valid config round-trips and is persisted to config.json.
    let mut edited = cfg.clone();
    edited["daemon"]["health_interval"] = serde_json::json!("45s");
    let r = http_post_put(&format!("{base}/config"), edited).await;
    assert!(r.status().is_success(), "valid config saves");
    assert!(cfg_write.exists(), "config.json written");
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cfg_write).unwrap()).unwrap();
    assert_eq!(saved["daemon"]["health_interval"], serde_json::json!("45s"));

    // PUT garbage is rejected with 422.
    let r = http_post_put(&format!("{base}/config"), serde_json::json!({"daemon": "not-an-object"}))
        .await;
    assert_eq!(r.status().as_u16(), 422, "invalid config rejected");

    // Tidy up the supervisor child so the test leaves no orphan `sleep`.
    let _ = client.unregister("test-svc").await;
}

/// `run_stack` boots headlessly offline with a minimal config and serves the
/// control API reporting offline state.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_stack_boots_headless_offline() {
    init_tracing();
    let tmp = tempfile::tempdir().unwrap();
    // Isolate HOME so the supervisor socket + config live under the temp dir.
    // SAFETY: this is the only test that mutates these globals; it is in its own
    // process-wide test binary run.
    unsafe {
        std::env::set_var("HOME", tmp.path());
    }
    // Minimal config: no services enabled, so no nix install is attempted.
    std::fs::write(tmp.path().join("config.toml"), "").unwrap();

    let opts = usb::StackOpts {
        home: tmp.path().to_path_buf(),
        config_path: Some(tmp.path().join("config.toml")),
        network: usb::NetworkMode::Offline,
        ui: false,
        nixpkgs_rev: None,
    };
    // On a host with nix this resolves to HostNix (no namespace / mount).
    let rt = usb::runtime::prepare(&opts.home, true).expect("prepare runtime");
    let handle = usb::run_stack(opts, rt).await.expect("run_stack");

    let base = format!("http://[::1]:{}", handle.metrics_port);
    // The control server reports offline; the supervisor is reachable (empty list).
    let status = http_get_json(&format!("{base}/status")).await;
    assert_eq!(status["offline"], serde_json::json!(true));
    assert!(status.get("services").is_some());

    handle.shutdown().await;
}
