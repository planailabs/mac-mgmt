//! The signed phone-home heartbeat POST, shared by the daemon and the
//! plan-ai-usb `usbd` control plane.
//!
//! Extracted from the daemon's event loop; the bits that used to be baked-in
//! compile-time constants of the daemon crate (version / environment / git
//! sha) are an explicit [`HeartbeatIdentity`] so each consumer reports its
//! own build identity.

/// Build identity reported in every heartbeat. The daemon fills this from its
/// `CARGO_PKG_VERSION` / `ENVIRONMENT` / `GIT_SHA`; usbd from its own.
#[derive(Clone, Debug)]
pub struct HeartbeatIdentity {
    pub version: String,
    pub environment: String,
    pub git_sha: String,
}

/// Returns `true` when the POST lands with a 2xx. Used by the caller to
/// gate one-shot follow-up work (e.g. the initial assessment inventory
/// send, which would otherwise race the heartbeat that creates its FK
/// parent row — see migration 031).
#[allow(clippy::too_many_arguments)]
pub async fn do_send_heartbeat(
    identity: &HeartbeatIdentity,
    server_url: &str,
    server_token: &str,
    instance_id: &str,
    host_key: &russh::keys::PrivateKey,
    services: Vec<serde_json::Value>,
    tunnels: Vec<serde_json::Value>,
    file_tunnels: Vec<serde_json::Value>,
    shell_tunnels: Vec<serde_json::Value>,
    relay_proxy_hostname: Option<String>,
    relay_proxy_url: Option<String>,
    sample: Option<mac_mgmt_common::DynamicSample>,
    services_extended: Vec<mac_mgmt_common::ServiceExtState>,
    service_samples: Vec<mac_mgmt_common::ServiceSample>,
    failure_signals: Vec<mac_mgmt_common::FailureSignal>,
) -> bool {
    use russh::keys::PublicKeyBase64;
    use russh::keys::signature::Signer;

    let client = reqwest::Client::new();
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();

    let signed_at = chrono::Utc::now().timestamp();
    let message = format!("{instance_id}:{signed_at}");
    let sig = host_key.try_sign(message.as_bytes());
    let (public_key_b64, sig_b64) = match sig {
        Ok(sig) => {
            use base64::Engine;
            let pk_b64 = host_key.public_key_base64();
            let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig.as_bytes());
            (pk_b64, sig_b64)
        }
        Err(e) => {
            tracing::warn!("failed to sign heartbeat: {e}");
            return false;
        }
    };

    let svc_count = services.len();
    let tunnel_count = tunnels.len();

    let body = mac_mgmt_common::HeartbeatBody {
        instance_id: instance_id.to_string(),
        version: identity.version.clone(),
        hostname: hostname.clone(),
        environment: identity.environment.clone(),
        services: serde_json::Value::Array(services),
        tunnels: serde_json::Value::Array(tunnels),
        file_tunnels: serde_json::Value::Array(file_tunnels),
        shell_tunnels: serde_json::Value::Array(shell_tunnels),
        relay_proxy_hostname,
        relay_proxy_url,
        nixpkgs_commit: crate::nix::current_nixpkgs_commit(),
        git_sha: Some(identity.git_sha.clone()),
        public_key: public_key_b64,
        signature: sig_b64,
        signed_at,
        sample,
        services_extended,
        service_samples,
        failure_signals,
    };

    let url = format!("{server_url}/api/heartbeat");
    tracing::debug!(
        "heartbeat → {url} instance={instance_id} host={hostname} signed_at={signed_at} services={svc_count} tunnels={tunnel_count} relay_proxy_url={:?}",
        body.relay_proxy_url,
    );
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client
            .post(&url)
            .bearer_auth(server_token)
            .json(&body)
            .send(),
    )
    .await
    {
        Ok(Ok(resp)) if resp.status().is_success() => {
            tracing::debug!("heartbeat accepted");
            true
        }
        Ok(Ok(resp)) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!("heartbeat rejected: {status} — {body}");
            false
        }
        Ok(Err(e)) => {
            tracing::warn!("heartbeat failed: {e}");
            false
        }
        Err(_) => {
            tracing::warn!("heartbeat timed out");
            false
        }
    }
}
