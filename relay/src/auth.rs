//! Token validation and auth types shared across relay endpoints.

use axum::http::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

/// Information about the authenticated token holder, from the server's /api/self endpoint.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SelfInfo {
    pub cluster_id: Option<Uuid>,
    pub cluster_name: Option<String>,
    #[allow(dead_code)]
    pub organization_id: Option<Uuid>,
    pub token_kind: String,
    #[serde(default)]
    pub cluster_ids: Vec<Uuid>,
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// Validate a Bearer token against the server's /api/self endpoint.
pub async fn validate_token(server_api_url: &str, token: &str) -> Result<SelfInfo, StatusCode> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{server_api_url}/api/self"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("token validation request failed: {e}");
            StatusCode::BAD_GATEWAY
        })?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if !resp.status().is_success() {
        tracing::error!("server API returned {}", resp.status());
        return Err(StatusCode::BAD_GATEWAY);
    }

    resp.json::<SelfInfo>().await.map_err(|e| {
        tracing::error!("failed to parse self info: {e}");
        StatusCode::BAD_GATEWAY
    })
}
