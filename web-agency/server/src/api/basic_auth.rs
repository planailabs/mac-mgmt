//! Cookie-based basic-auth gate.
//!
//! Replaces the HTTP Basic challenge with an SSO cookie + redirect dance
//! (Cloudflare-Access style). Two surfaces live here:
//!
//!   * **Internal API** (`/api/internal/basic/{verify,session,logout}`) —
//!     Bearer-token authed, called by the proxy on every gated request.
//!   * **Agency HTML pages** (`/agency/basic/...`) — public login/logout/profile
//!     pages on the agency domain; see `agency_router`.
//!
//! Identity model (see migration 027): a browser `sso` proves credentials for a
//! set of lists (`basic_auth_sso_lists`). Cookies on each domain
//! (`basic_auth_sessions`) reference that identity. List authorizations live on
//! the identity, so logging into one site silently covers every site sharing
//! the list.

use dioxus::fullstack::axum::{
    Router,
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config;

// ── small helpers ───────────────────────────────────────────────────────

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(input.as_bytes()))
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes()
        .iter()
        .zip(b.as_bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn extract_cookie<'a>(cookies: &'a str, name: &str) -> Option<&'a str> {
    for part in cookies.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix(name) {
            if let Some(val) = rest.strip_prefix('=') {
                return Some(val);
            }
        }
    }
    None
}

fn cookie_header<'a>(headers: &'a HeaderMap) -> &'a str {
    headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

/// Percent-encode a value for use in a query string.
fn enc(s: &str) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

/// Minimal HTML escaping for interpolating untrusted values into a page.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// The shared HMAC key (same internal token the proxy holds).
fn internal_token() -> Result<String, (StatusCode, String)> {
    let cfg = config::config();
    let proxy_cfg = cfg.proxy.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "proxy not configured".to_string(),
    ))?;
    let tok = std::fs::read_to_string(&proxy_cfg.internal_token_path).map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "internal token not available".to_string(),
        )
    })?;
    Ok(tok.trim().to_string())
}

fn agency_domain() -> Result<String, (StatusCode, String)> {
    let cfg = config::config();
    let proxy_cfg = cfg.proxy.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "proxy not configured".to_string(),
    ))?;
    Ok(proxy_cfg.agency_domain.clone())
}

// ── signed handoff token (agency → proxy → internal /session) ────────────

#[derive(Serialize, Deserialize)]
struct Handoff {
    sso: Uuid,
    list: Uuid,
    u: String,
    keep: bool,
    exp: i64,
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
fn unb64(s: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s).ok()
}

fn hmac_hex(key: &str, msg: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("hmac key");
    mac.update(msg.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn mint_handoff(key: &str, h: &Handoff) -> String {
    let payload = b64(serde_json::to_string(h).unwrap().as_bytes());
    let sig = hmac_hex(key, &payload);
    format!("{payload}.{sig}")
}

fn verify_handoff(key: &str, token: &str) -> Option<Handoff> {
    let (payload, sig) = token.split_once('.')?;
    if !constant_time_eq(&hmac_hex(key, payload), sig) {
        return None;
    }
    let h: Handoff = serde_json::from_slice(&unb64(payload)?).ok()?;
    if h.exp <= chrono::Utc::now().timestamp() {
        return None;
    }
    Some(h)
}

// ── session DB helpers ──────────────────────────────────────────────────

const KEEP_SECS: i64 = 365 * 24 * 3600;
const SESSION_SECS: i64 = 12 * 3600;

fn random_token() -> String {
    use rand::RngCore;
    let mut b = [0u8; 32];
    rand::rng().fill_bytes(&mut b);
    hex::encode(b)
}

/// Verify a cookie token grants access to `list_id`. Returns the username if so.
async fn verify_session(pool: &PgPool, token: &str, list_id: Uuid) -> Option<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT sl.username \
         FROM basic_auth_sessions s \
         JOIN basic_auth_sso o ON o.id = s.sso_id \
         JOIN basic_auth_sso_lists sl ON sl.sso_id = o.id \
         WHERE s.token_hash = $1 AND s.expires_at > now() AND o.expires_at > now() \
           AND sl.list_id = $2",
    )
    .bind(sha256_hex(token))
    .bind(list_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// Create a cookie session for an existing identity. Returns (raw token, max_age).
async fn create_session(pool: &PgPool, sso_id: Uuid, keep: bool) -> Result<(String, Option<i64>), String> {
    let token = random_token();
    let ttl = if keep { KEEP_SECS } else { SESSION_SECS };
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(ttl);
    sqlx::query(
        "INSERT INTO basic_auth_sessions (sso_id, token_hash, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(sso_id)
    .bind(sha256_hex(&token))
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok((token, if keep { Some(KEEP_SECS) } else { None }))
}

// ── internal API (Bearer-authed, called by the proxy) ────────────────────

pub fn internal_router(pool: PgPool) -> Router<()> {
    Router::new()
        .route("/api/internal/basic/verify", post(verify_ep))
        .route("/api/internal/basic/session", post(session_ep))
        .route("/api/internal/basic/logout", post(logout_ep))
        .with_state(pool)
}

#[derive(Deserialize)]
struct VerifyReq {
    session_token: String,
    list_id: Uuid,
}
#[derive(Serialize)]
struct VerifyResp {
    valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
}

async fn verify_ep(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    Json(req): Json<VerifyReq>,
) -> Result<Json<VerifyResp>, (StatusCode, String)> {
    crate::api::internal::authenticate(&headers)?;
    let username = verify_session(&pool, &req.session_token, req.list_id).await;
    Ok(Json(VerifyResp {
        valid: username.is_some(),
        username,
    }))
}

#[derive(Deserialize)]
struct SessionReq {
    handoff_token: String,
}
#[derive(Serialize)]
struct SessionResp {
    session_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_age_secs: Option<i64>,
}

async fn session_ep(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    Json(req): Json<SessionReq>,
) -> Result<Json<SessionResp>, (StatusCode, String)> {
    crate::api::internal::authenticate(&headers)?;
    let key = internal_token()?;
    let h = verify_handoff(&key, &req.handoff_token)
        .ok_or((StatusCode::BAD_REQUEST, "invalid handoff token".to_string()))?;
    let (token, max_age) = create_session(&pool, h.sso, h.keep)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(SessionResp {
        session_token: token,
        max_age_secs: max_age,
    }))
}

#[derive(Deserialize)]
struct LogoutReq {
    session_token: String,
    list_id: Uuid,
}

async fn logout_ep(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    Json(req): Json<LogoutReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::api::internal::authenticate(&headers)?;
    // Drop only the current list's authorization for this identity.
    sqlx::query(
        "DELETE FROM basic_auth_sso_lists sl \
         USING basic_auth_sessions s \
         WHERE sl.sso_id = s.sso_id AND s.token_hash = $1 AND sl.list_id = $2",
    )
    .bind(sha256_hex(&req.session_token))
    .bind(req.list_id)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Tidy up identities that no longer authorize any list.
    let _ = sqlx::query(
        "DELETE FROM basic_auth_sso o \
         WHERE NOT EXISTS (SELECT 1 FROM basic_auth_sso_lists sl WHERE sl.sso_id = o.id)",
    )
    .execute(&pool)
    .await;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_roundtrip_and_tamper() {
        let key = "secret-internal-token";
        let h = Handoff {
            sso: Uuid::nil(),
            list: Uuid::nil(),
            u: "alice".into(),
            keep: true,
            exp: chrono::Utc::now().timestamp() + 60,
        };
        let tok = mint_handoff(key, &h);
        let got = verify_handoff(key, &tok).expect("valid");
        assert_eq!(got.u, "alice");
        assert!(got.keep);
        // wrong key rejected
        assert!(verify_handoff("other", &tok).is_none());
        // tampered payload rejected
        let mut bad = tok.clone();
        bad.insert(0, 'x');
        assert!(verify_handoff(key, &bad).is_none());
    }

    #[test]
    fn handoff_expired_rejected() {
        let key = "k";
        let h = Handoff {
            sso: Uuid::nil(),
            list: Uuid::nil(),
            u: "x".into(),
            keep: false,
            exp: chrono::Utc::now().timestamp() - 1,
        };
        let tok = mint_handoff(key, &h);
        assert!(verify_handoff(key, &tok).is_none());
    }
}
