//! Minimal xzar HTTP upload client.
//!
//! Uploads a single nix store path to xzar and pins it. Uses the existing
//! `[xzar]` config (url + token) for authentication. The NAR is streamed
//! from `nix-store --dump` through xz compression directly to the HTTP
//! request — no temp files.

use reqwest::multipart;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio_util::io::ReaderStream;

// ── xzar API types ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LockResponse {
    lock: i32,
    #[allow(dead_code)]
    deadline: String,
}

#[derive(Serialize)]
struct LockClearRequest {
    lock: i32,
}

#[derive(Serialize)]
struct FinalizePinRequest {
    roots: Vec<String>,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    desc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "leaveAfterAbandon")]
    leave_after_abandon: Option<i64>,
}

// ── Nix helpers ────────────────────────────────────────────────────────

/// Query a single nix-store property (--hash, --size, etc.)
async fn nix_query(store_path: &str, flag: &str) -> Result<String, String> {
    let output = Command::new("nix-store")
        .args(["--query", flag, store_path])
        .output()
        .await
        .map_err(|e| format!("failed to run nix-store --query {flag}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("nix-store --query {flag} failed: {stderr}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Extract the basename from a store path (e.g. "/nix/store/abc-foo" → "abc-foo").
fn store_basename(store_path: &str) -> &str {
    store_path
        .strip_prefix("/nix/store/")
        .unwrap_or(store_path)
}

// ── Public API ─────────────────────────────────────────────────────────

/// Upload a single nix store path to xzar and create a named pin for it.
///
/// The NAR is streamed from `nix-store --dump` through xz compression
/// directly into the HTTP multipart upload — no temp files on disk.
pub async fn upload_and_pin(
    xzar_url: &str,
    xzar_token: &str,
    store_path: &str,
    pin_name: &str,
    pin_desc: Option<&str>,
) -> Result<(), String> {
    let url = xzar_url.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    // 1. Get NAR metadata
    let hash = nix_query(store_path, "--hash").await?;
    let size = nix_query(store_path, "--size").await?;
    let drv_full = store_basename(store_path).to_string();

    // 2. Request lock
    let lock_resp: LockResponse = client
        .post(format!("{url}/lock/request"))
        .bearer_auth(xzar_token)
        .send()
        .await
        .map_err(|e| format!("lock request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("lock request rejected: {e}"))?
        .json()
        .await
        .map_err(|e| format!("failed to parse lock response: {e}"))?;

    let lock_id = lock_resp.lock;

    // Helper to clear lock on error
    let clear_lock = |client: &reqwest::Client, url: &str, token: &str, lock: i32| {
        let client = client.clone();
        let url = url.to_string();
        let token = token.to_string();
        async move {
            let _ = client
                .post(format!("{url}/lock/clear"))
                .bearer_auth(&token)
                .json(&LockClearRequest { lock })
                .send()
                .await;
        }
    };

    // 3. Upload NAR (streaming: nix-store --dump | xz → HTTP)
    let upload_result = upload_nar(
        &client, url, xzar_token, store_path, &drv_full, &hash, &size, lock_id,
    )
    .await;

    if let Err(e) = upload_result {
        clear_lock(&client, url, xzar_token, lock_id).await;
        return Err(e);
    }

    // 4. Clear lock before finalizing
    clear_lock(&client, url, xzar_token, lock_id).await;

    // 5. Create pin
    let pin_body = FinalizePinRequest {
        roots: vec![drv_full],
        name: pin_name.to_string(),
        desc: pin_desc.map(|s| s.to_string()),
        leave_after_abandon: Some(60_000), // 1 minute
    };

    let resp = client
        .post(format!("{url}/finalizePin"))
        .bearer_auth(xzar_token)
        .json(&pin_body)
        .send()
        .await
        .map_err(|e| format!("finalizePin failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("finalizePin returned {status}: {body}"));
    }

    Ok(())
}

async fn upload_nar(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    store_path: &str,
    drv_full: &str,
    hash: &str,
    size: &str,
    lock_id: i32,
) -> Result<(), String> {
    use async_compression::tokio::bufread::XzEncoder;
    use tokio::io::BufReader;

    // Spawn nix-store --dump and stream its stdout through xz compression
    let mut child = Command::new("nix-store")
        .args(["--dump", store_path])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn nix-store --dump: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture nix-store stdout".to_string())?;

    // Pipe through xz compression
    let reader = BufReader::new(stdout);
    let compressed = XzEncoder::new(reader);
    let stream = ReaderStream::new(compressed);

    // Build multipart form — drvFull MUST come before file (xzar server requirement)
    let file_part = multipart::Part::stream(reqwest::Body::wrap_stream(stream))
        .file_name("nar.xz")
        .mime_str("application/octet-stream")
        .map_err(|e| format!("failed to set mime type: {e}"))?;

    let form = multipart::Form::new()
        .text("lock", lock_id.to_string())
        .text("drvFull", drv_full.to_string())
        .text("hash", hash.to_string())
        .text("size", size.to_string())
        .text("compression", "xz".to_string())
        .part("file", file_part);

    let resp = client
        .put(format!("{url}/uploadNar"))
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("uploadNar failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("uploadNar returned {status}: {body}"));
    }

    // Wait for nix-store --dump to finish
    let status = child
        .wait()
        .await
        .map_err(|e| format!("nix-store --dump wait failed: {e}"))?;

    if !status.success() {
        return Err("nix-store --dump exited with non-zero status".to_string());
    }

    Ok(())
}
