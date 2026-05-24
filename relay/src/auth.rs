//! Token validation and auth types shared across relay endpoints.

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
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

/// Request body for the server's POST /api/cert-auth endpoint.
#[derive(Debug, Serialize)]
struct CertAuthBody {
    certificate_pem: String,
}

/// Response from the server's /api/cert-auth endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct CertAuthInfo {
    #[serde(default)]
    pub cluster_ids: Vec<Uuid>,
    #[serde(default)]
    pub organization_ids: Vec<Uuid>,
    pub token_kind: String,
}

/// Cache for cert-auth results (fingerprint -> (result, timestamp)).
/// 5-minute TTL, same as token cache in proxy_handler.
static CERT_CACHE: std::sync::LazyLock<
    tokio::sync::RwLock<std::collections::HashMap<String, (CertAuthResult, std::time::Instant)>>,
> = std::sync::LazyLock::new(Default::default);

const CERT_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(300);

/// Cached result: either a successful CertAuthInfo or a "not found" marker.
#[derive(Debug, Clone)]
enum CertAuthResult {
    Ok(CertAuthInfo),
    NotFound,
}

/// Validate a client certificate against the server's /api/cert-auth endpoint.
/// The full PEM is sent; the server computes the fingerprint.
/// Results are cached for 5 minutes, keyed by the locally-computed fingerprint.
pub async fn validate_cert(
    server_api_url: &str,
    fingerprint: &str,
    certificate_pem: &str,
) -> Result<CertAuthInfo, StatusCode> {
    // Check cache first.
    {
        let cache = CERT_CACHE.read().await;
        if let Some((result, created)) = cache.get(fingerprint) {
            if created.elapsed() < CERT_CACHE_TTL {
                return match result {
                    CertAuthResult::Ok(info) => Ok(info.clone()),
                    CertAuthResult::NotFound => Err(StatusCode::FORBIDDEN),
                };
            }
        }
    }

    // Cache miss — validate against server.
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{server_api_url}/api/cert-auth"))
        .json(&CertAuthBody {
            certificate_pem: certificate_pem.to_string(),
        })
        .send()
        .await
        .map_err(|e| {
            tracing::error!("cert-auth request failed: {e}");
            StatusCode::BAD_GATEWAY
        })?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        let mut cache = CERT_CACHE.write().await;
        cache.insert(
            fingerprint.to_string(),
            (CertAuthResult::NotFound, std::time::Instant::now()),
        );
        return Err(StatusCode::FORBIDDEN);
    }
    if !resp.status().is_success() {
        tracing::error!("cert-auth API returned {}", resp.status());
        return Err(StatusCode::BAD_GATEWAY);
    }

    let info: CertAuthInfo = resp.json().await.map_err(|e| {
        tracing::error!("failed to parse cert-auth response: {e}");
        StatusCode::BAD_GATEWAY
    })?;

    {
        let mut cache = CERT_CACHE.write().await;
        cache.insert(
            fingerprint.to_string(),
            (CertAuthResult::Ok(info.clone()), std::time::Instant::now()),
        );
        if cache.len() > 1000 {
            cache.retain(|_, (_, t)| t.elapsed() < CERT_CACHE_TTL);
        }
    }

    Ok(info)
}
