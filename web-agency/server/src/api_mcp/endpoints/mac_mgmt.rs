//! mac-mgmt fleet endpoints: list clusters/machines and mint setting tokens
//! (admin only, via a stored `mac-mgmt` credential holding an *admin* token),
//! plus relay-URL listing which also works with non-admin (setting) tokens and
//! is open to any user who can read the credential. Exists so action templates
//! (and MCP/REST callers) can provision relay webspaces end to end, and so the
//! webspace form can offer a relay-URL picker.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;

#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MacMgmtClusterRow {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MacMgmtClustersInput {
    /// A `mac-mgmt` credential holding the server URL and an admin token.
    pub credential_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MacMgmtMachineRow {
    pub instance_id: String,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MacMgmtMachinesInput {
    pub credential_id: Uuid,
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MacMgmtTokenCreateInput {
    pub credential_id: Uuid,
    pub cluster_id: Uuid,
    pub label: String,
    /// "setting" (default) or "sync".
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String {
    "setting".to_string()
}

/// The plaintext token — returned exactly once; callers must not log it
/// (templates: mark the step `no_log`).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MacMgmtCreatedToken {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MacMgmtRelayUrlsInput {
    /// A `mac-mgmt` credential; its token's scope decides which relay URLs
    /// are visible (cluster, organization, or fleet-wide for admin tokens).
    pub credential_id: Uuid,
}

/// One relay portal URL: an instance tunnel reachable through the relay proxy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MacMgmtRelayUrlRow {
    pub cluster_id: Uuid,
    pub instance_id: String,
    #[serde(default)]
    pub hostname: Option<String>,
    /// Tunnel (service) name, e.g. `openclaw`.
    pub tunnel: String,
    /// Ready-to-use relay URL for this tunnel.
    pub url: String,
    /// The instance's relay proxy base URL (no tunnel subdomain).
    #[serde(default)]
    pub relay_proxy_url: String,
}

#[cfg(feature = "server")]
async fn admin_client(
    pool: &sqlx::PgPool,
    credential_id: Uuid,
) -> Result<(reqwest::Client, String), ApiError> {
    let (server_url, token) = crate::credentials::mac_mgmt_credential(pool, credential_id)
        .await
        .map_err(|e| ApiError::bad_request(format!("mac-mgmt credential: {e}")))?;
    let mut headers = reqwest::header::HeaderMap::new();
    let mut auth: reqwest::header::HeaderValue = format!("Bearer {token}")
        .parse()
        .map_err(|_| ApiError::bad_request("mac-mgmt credential token is not a valid header"))?;
    auth.set_sensitive(true);
    headers.insert(reqwest::header::AUTHORIZATION, auth);
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(super::internal)?;
    Ok((client, server_url.trim_end_matches('/').to_string()))
}

#[cfg(feature = "server")]
async fn read_json<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
    what: &str,
) -> Result<T, ApiError> {
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::bad_request(format!(
            "mac-mgmt {what}: {status} {}",
            body.chars().take(200).collect::<String>()
        )));
    }
    resp.json::<T>()
        .await
        .map_err(|e| ApiError::internal(format!("mac-mgmt {what}: bad response: {e}")))
}

/// Allow access when the caller can read the credential: admins always,
/// org members for org-scoped credentials, anyone signed in for global ones
/// (mirrors what the webspace form exposes in its credential dropdown).
#[cfg(feature = "server")]
async fn require_credential_read(
    pool: &sqlx::PgPool,
    p: &Principal,
    credential_id: Uuid,
) -> Result<(), ApiError> {
    let org: Option<Option<Uuid>> = sqlx::query_scalar(
        "SELECT organization_id FROM credentials WHERE id = $1 AND credential_type = 'mac-mgmt'",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?;
    match org {
        None => Err(ApiError::not_found("mac-mgmt credential not found")),
        Some(Some(org_id)) => p.require_read(&org_id),
        Some(None) => Ok(()),
    }
}

/// List mac-mgmt clusters (admin only).
#[api_mcp_dioxus_server(server = "list_mac_mgmt_clusters")]
pub async fn mac_mgmt_clusters(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: MacMgmtClustersInput,
) -> Result<Vec<MacMgmtClusterRow>, ApiError> {
    p.require_admin()?;
    let (client, base) = admin_client(pool, input.credential_id).await?;
    let resp = client
        .get(format!("{base}/api/admin/clusters"))
        .send()
        .await
        .map_err(super::internal)?;
    read_json(resp, "clusters").await
}

/// List heartbeat-reporting machines of a mac-mgmt cluster (admin only).
#[api_mcp_dioxus_server(server = "list_mac_mgmt_machines")]
pub async fn mac_mgmt_machines(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: MacMgmtMachinesInput,
) -> Result<Vec<MacMgmtMachineRow>, ApiError> {
    p.require_admin()?;
    let (client, base) = admin_client(pool, input.credential_id).await?;
    let resp = client
        .get(format!(
            "{base}/api/admin/clusters/{}/machines",
            input.cluster_id
        ))
        .send()
        .await
        .map_err(super::internal)?;
    read_json(resp, "machines").await
}

/// List the relay portal URLs reachable with a stored mac-mgmt credential's
/// token — a picker for relay webspace URLs. Unlike the fleet-admin endpoints
/// this works with non-admin (setting) tokens and only requires credential
/// read access.
#[api_mcp_dioxus_server(server = "list_mac_mgmt_relay_urls")]
pub async fn mac_mgmt_relay_urls(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: MacMgmtRelayUrlsInput,
) -> Result<Vec<MacMgmtRelayUrlRow>, ApiError> {
    require_credential_read(pool, p, input.credential_id).await?;
    let (client, base) = admin_client(pool, input.credential_id).await?;
    let resp = client
        .get(format!("{base}/api/relay-urls"))
        .send()
        .await
        .map_err(super::internal)?;
    read_json(resp, "relay urls").await
}

/// Create a mac-mgmt token for a cluster (admin only). Returns the plaintext
/// token exactly once.
#[api_mcp_dioxus_server(server = "create_mac_mgmt_token")]
pub async fn mac_mgmt_token_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: MacMgmtTokenCreateInput,
) -> Result<MacMgmtCreatedToken, ApiError> {
    p.require_admin()?;
    if input.kind != "setting" && input.kind != "sync" {
        return Err(ApiError::bad_request("kind must be 'setting' or 'sync'"));
    }
    let (client, base) = admin_client(pool, input.credential_id).await?;
    let resp = client
        .post(format!(
            "{base}/api/admin/clusters/{}/tokens",
            input.cluster_id
        ))
        .json(&serde_json::json!({ "label": input.label, "kind": input.kind }))
        .send()
        .await
        .map_err(super::internal)?;
    read_json(resp, "token create").await
}
