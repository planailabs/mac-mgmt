//! mac-mgmt fleet-admin endpoints (admin only): list clusters/machines and
//! mint setting tokens through a stored `mac-mgmt` credential whose secret
//! holds `{server_url, token}` with an *admin* token. Exists so action
//! templates (and MCP/REST callers) can provision relay webspaces end to end.

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
        .get(format!("{base}/api/admin/clusters/{}/machines", input.cluster_id))
        .send()
        .await
        .map_err(super::internal)?;
    read_json(resp, "machines").await
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
        .post(format!("{base}/api/admin/clusters/{}/tokens", input.cluster_id))
        .json(&serde_json::json!({ "label": input.label, "kind": input.kind }))
        .send()
        .await
        .map_err(super::internal)?;
    read_json(resp, "token create").await
}
