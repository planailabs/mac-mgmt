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

use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use dioxus::fullstack::axum::{
    Router,
    extract::{Json, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::config;

// ── small helpers ───────────────────────────────────────────────────────

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(input.as_bytes()))
}

/// Hash a user password for storage. Argon2id (salted, memory-hard) — never a
/// bare hash. Returns the PHC-encoded string (`$argon2id$...`) which embeds the
/// salt and parameters.
pub fn hash_password(password: &str) -> Result<String, String> {
    use argon2::password_hash::{PasswordHasher, SaltString, rand_core::OsRng};
    use argon2::Argon2;
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

/// Verify a password against a stored hash. Accepts both the new Argon2 PHC
/// format and the legacy unsalted `sha256_hex` format (64 lowercase hex chars)
/// so existing credentials keep working until they are re-hashed on next login.
/// Returns `(is_valid, needs_rehash)`.
fn verify_password(stored: &str, password: &str) -> (bool, bool) {
    if let Some(rest) = stored.strip_prefix("$argon2") {
        let _ = rest;
        use argon2::password_hash::{PasswordHash, PasswordVerifier};
        use argon2::Argon2;
        let ok = PasswordHash::new(stored)
            .map(|parsed| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &parsed)
                    .is_ok()
            })
            .unwrap_or(false);
        (ok, false)
    } else {
        // Legacy unsalted SHA-256. Constant-time compare, flag for upgrade.
        let ok = constant_time_eq(stored, &sha256_hex(password));
        (ok, ok)
    }
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


/// Percent-encode a value for use in a query string.
fn enc(s: &str) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

use plan_ai_html::escape as esc;

// ── brute-force protection ───────────────────────────────────────────────
//
// Failed logins are tracked per client subnet: a /32 for IPv4 (the exact host)
// and a /48 for IPv6 (the typical end-site allocation, so one customer can't
// dodge the limit by hopping addresses inside their prefix). The first two
// fails are free; the 3rd locks the subnet for 30s, and each further fail
// doubles the wait (30s, 60s, 120s, …). A success clears the counter.
//
// State is in-process only. ponytail: a single proxy/server instance shares one
// map; move to Redis if the agency server is ever horizontally scaled.

const FREE_ATTEMPTS: u32 = 2;
const BASE_LOCK_SECS: u64 = 30;
/// Forget a subnet once it's been idle this long — bounds the map and resets
/// honest users who simply walked away.
const RESET_AFTER: Duration = Duration::from_secs(3600);

struct Attempt {
    fails: u32,
    locked_until: Option<Instant>,
    last_seen: Instant,
}

static ATTEMPTS: LazyLock<Mutex<HashMap<String, Attempt>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Lockout duration for the n-th consecutive failure (0 = no lock yet).
fn lock_secs(fails: u32) -> u64 {
    if fails <= FREE_ATTEMPTS {
        return 0;
    }
    // 3rd fail → shift 0 (30s), 4th → 1 (60s)… capped so the shift never overflows.
    let shift = (fails - FREE_ATTEMPTS - 1).min(20);
    BASE_LOCK_SECS.saturating_mul(1u64 << shift)
}

/// Collapse a client IP to its rate-limit key: /32 for v4, /48 for v6.
fn subnet_key(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => {
            let o = v6.octets();
            format!(
                "{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}::/48",
                o[0], o[1], o[2], o[3], o[4], o[5]
            )
        }
    }
}

/// ponytail: O(n) sweep on every mutation — fine at login-form request volume.
fn prune(map: &mut HashMap<String, Attempt>, now: Instant) {
    map.retain(|_, e| now.duration_since(e.last_seen) < RESET_AFTER);
}

/// Seconds remaining if this subnet is currently locked out, else None.
fn locked_for(ip: IpAddr) -> Option<u64> {
    let now = Instant::now();
    let mut map = ATTEMPTS.lock().unwrap();
    prune(&mut map, now);
    let e = map.get(&subnet_key(ip))?;
    match e.locked_until {
        Some(until) if until > now => Some((until - now).as_secs() + 1),
        _ => None,
    }
}

/// Record a failed login; returns the resulting lockout in seconds (0 if free).
fn record_failure(ip: IpAddr) -> u64 {
    let now = Instant::now();
    let mut map = ATTEMPTS.lock().unwrap();
    prune(&mut map, now);
    let e = map.entry(subnet_key(ip)).or_insert(Attempt {
        fails: 0,
        locked_until: None,
        last_seen: now,
    });
    e.fails += 1;
    e.last_seen = now;
    let secs = lock_secs(e.fails);
    e.locked_until = (secs > 0).then(|| now + Duration::from_secs(secs));
    secs
}

/// Clear a subnet's failure history after a successful login.
fn record_success(ip: IpAddr) {
    ATTEMPTS.lock().unwrap().remove(&subnet_key(ip));
}

/// Real client IP from the proxy-stamped `X-Forwarded-For` (first hop).
/// Returns None when absent — protection then fails open rather than locking
/// every user behind a shared proxy address.
fn client_ip(headers: &HeaderMap) -> Option<IpAddr> {
    let raw = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))?
        .to_str()
        .ok()?;
    raw.split(',').next()?.trim().parse().ok()
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

/// Verify a cookie token grants access to `list_id`. Returns (username,
/// list_name) if so.
async fn verify_session(pool: &PgPool, token: &str, list_id: Uuid) -> Option<(String, String)> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT sl.username, l.name \
         FROM basic_auth_sessions s \
         JOIN basic_auth_sso o ON o.id = s.sso_id \
         JOIN basic_auth_sso_lists sl ON sl.sso_id = o.id \
         JOIN basic_auth_lists l ON l.id = sl.list_id \
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
    #[serde(skip_serializing_if = "Option::is_none")]
    list_name: Option<String>,
}

async fn verify_ep(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    Json(req): Json<VerifyReq>,
) -> Result<Json<VerifyResp>, (StatusCode, String)> {
    crate::api::internal::authenticate(&headers)?;
    let found = verify_session(&pool, &req.session_token, req.list_id).await;
    let valid = found.is_some();
    let (username, list_name) = match found {
        Some((u, n)) => (Some(u), Some(n)),
        None => (None, None),
    };
    Ok(Json(VerifyResp {
        valid,
        username,
        list_name,
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

// ── agency HTML pages (public; NOT behind OIDC require_auth) ─────────────

pub fn agency_router(pool: PgPool) -> Router<()> {
    Router::new()
        .route(
            "/agency/basic/{list_id}/login",
            get(login_page).post(login_submit),
        )
        .route("/agency/basic/{list_id}/logout", get(logout_page))
        .route("/agency/basic/{list_id}/profile", get(profile_page))
        .route("/agency/basic/profile", get(global_profile))
        .route("/agency/basic/profile/logout", post(global_logout))
        .with_state(pool)
}

// -- form / url helpers --

fn url_host(u: &str) -> String {
    u.strip_prefix("https://")
        .or_else(|| u.strip_prefix("http://"))
        .and_then(|s| s.split('/').next())
        .map(|s| s.split(':').next().unwrap_or(s))
        .unwrap_or("")
        .to_lowercase()
}

fn urldecode(s: &str) -> String {
    let s = s.replace('+', " ");
    percent_encoding::percent_decode_str(&s)
        .decode_utf8_lossy()
        .into_owned()
}

fn parse_form(body: &str) -> HashMap<String, String> {
    body.split('&')
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            Some((urldecode(k), urldecode(v)))
        })
        .collect()
}

/// True if `cb` points at a domain actually bound to a webspace using `list_id`.
/// Blocks open-redirect via a forged callback host.
async fn cb_host_ok(pool: &PgPool, list_id: Uuid, cb: &str) -> bool {
    let host = url_host(cb);
    if host.is_empty() {
        return false;
    }
    sqlx::query_scalar::<_, i32>(
        "SELECT 1 FROM webspaces w \
         JOIN webspace_hosts h ON h.id = w.webspace_host_id \
         JOIN webspace_host_domains whd ON whd.webspace_host_id = h.id \
         JOIN domains d ON d.id = whd.domain_id \
         LEFT JOIN subdomains s ON s.id = whd.subdomain_id \
         WHERE w.auth_basic_list_id = $1 \
           AND (CASE WHEN s.name IS NOT NULL AND s.name != '@' \
                THEN s.name || '.' || d.name ELSE d.name END) = $2 \
         LIMIT 1",
    )
    .bind(list_id)
    .bind(host)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .is_some()
}

// -- sso/identity DB helpers --

const AGENCY_COOKIE: &str = "__agency_basic_sso";
const SSO_TRY_COOKIE: &str = "__basic_sso_try";

async fn sso_from_cookie(pool: &PgPool, jar: &CookieJar) -> Option<Uuid> {
    let tok = jar.get(AGENCY_COOKIE)?.value().to_string();
    sqlx::query_scalar::<_, Uuid>(
        "SELECT s.sso_id FROM basic_auth_sessions s \
         JOIN basic_auth_sso o ON o.id = s.sso_id \
         WHERE s.token_hash = $1 AND s.expires_at > now() AND o.expires_at > now()",
    )
    .bind(sha256_hex(&tok))
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

async fn sso_list_username(pool: &PgPool, sso_id: Uuid, list_id: Uuid) -> Option<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT username FROM basic_auth_sso_lists WHERE sso_id = $1 AND list_id = $2",
    )
    .bind(sso_id)
    .bind(list_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

async fn creds_ok(pool: &PgPool, list_id: Uuid, username: &str, password: &str) -> bool {
    let stored = sqlx::query_scalar::<_, String>(
        "SELECT password_hash FROM basic_auth_credentials WHERE list_id = $1 AND username = $2",
    )
    .bind(list_id)
    .bind(username)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    let Some(hash) = stored else { return false };
    let (ok, needs_rehash) = verify_password(&hash, password);
    if ok && needs_rehash {
        // Transparently upgrade the legacy SHA-256 hash to Argon2 on login.
        if let Ok(new_hash) = hash_password(password) {
            let _ = sqlx::query(
                "UPDATE basic_auth_credentials SET password_hash = $3 \
                 WHERE list_id = $1 AND username = $2",
            )
            .bind(list_id)
            .bind(username)
            .bind(&new_hash)
            .execute(pool)
            .await;
        }
    }
    ok
}

/// Resolve (or create) the browser identity. Returns the identity id and, when
/// a new one was created, the agency SSO cookie to add to the response jar.
async fn ensure_sso(
    pool: &PgPool,
    jar: &CookieJar,
    keep: bool,
) -> Result<(Uuid, Option<Cookie<'static>>), String> {
    let ttl = if keep { KEEP_SECS } else { SESSION_SECS };
    let new_exp = chrono::Utc::now() + chrono::Duration::seconds(ttl);
    if let Some(sso_id) = sso_from_cookie(pool, jar).await {
        // Extend the identity's life; reuse the existing agency cookie.
        let _ = sqlx::query("UPDATE basic_auth_sso SET expires_at = GREATEST(expires_at, $2) WHERE id = $1")
            .bind(sso_id)
            .bind(new_exp)
            .execute(pool)
            .await;
        return Ok((sso_id, None));
    }
    let sso_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO basic_auth_sso (expires_at) VALUES ($1) RETURNING id",
    )
    .bind(new_exp)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;
    let (cookie_token, _) = create_session(pool, sso_id, keep).await?;
    Ok((sso_id, Some(agency_cookie(&cookie_token, keep))))
}

fn base_cookie(name: &'static str, value: String) -> Cookie<'static> {
    Cookie::build((name, value))
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .build()
}

fn agency_cookie(token: &str, keep: bool) -> Cookie<'static> {
    let mut c = base_cookie(AGENCY_COOKIE, token.to_string());
    if keep {
        c.set_max_age(Some(time::Duration::seconds(KEEP_SECS)));
    }
    c
}

fn sso_try_cookie() -> Cookie<'static> {
    let mut c = base_cookie(SSO_TRY_COOKIE, "1".to_string());
    c.set_max_age(Some(time::Duration::seconds(15)));
    c
}

/// Mint a handoff token and redirect the browser back to the webspace callback.
fn handoff_redirect(
    sso_id: Uuid,
    list_id: Uuid,
    username: &str,
    keep: bool,
    cb: &str,
    back: &str,
) -> Response {
    let key = match internal_token() {
        Ok(k) => k,
        Err(e) => return e.into_response(),
    };
    let h = Handoff {
        sso: sso_id,
        list: list_id,
        u: username.to_string(),
        keep,
        exp: chrono::Utc::now().timestamp() + 120,
    };
    let token = mint_handoff(&key, &h);
    let url = format!("{cb}/login?token={token}&back={}", enc(back));
    Redirect::to(&url).into_response()
}

// -- HTML rendering (plan-ai-design via plan_ai_html) --

use plan_ai_html::{Lang, tr, tr_args};

/// Detect the UI language from the request's Accept-Language header.
fn lang(headers: &HeaderMap) -> Lang {
    Lang::from_accept_language(
        headers
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    )
}

/// Wrap a pre-rendered body in the plan-ai-design page chrome (localized lang).
fn page(lang: Lang, title: &str, body: &str) -> Html<String> {
    Html(plan_ai_html::Page::new(title, body).lang(lang).render())
}

const LOGIN_TPL: &str = r#"<h1 class="h-page">{{l_title}}</h1>
<form method="post" action="/agency/basic/{{list_id}}/login">
<input type="hidden" name="cb" value="{{cb}}">
<input type="hidden" name="back" value="{{back}}">
<label class="label" for="u">{{l_user}}</label>
<input class="input" id="u" type="text" name="username" autofocus autocomplete="username">
<label class="label" for="p" style="margin-top:.75rem">{{l_pass}}</label>
<input class="input" id="p" type="password" name="password" autocomplete="current-password">
<label class="help" style="display:flex;align-items:center;gap:.45rem;margin-top:.8rem"><input type="checkbox" name="keep" value="on"> {{l_keep}}</label>
{{#has_error}}<div class="err" style="margin-top:.6rem">{{error}}</div>{{/has_error}}
<button class="btn btn-primary btn-lg" type="submit" style="width:100%;margin-top:1rem">{{l_title}}</button>
</form>"#;

fn login_form_html(lang: Lang, list_id: Uuid, cb: &str, back: &str, error: Option<&str>) -> Html<String> {
    let data = plan_ai_html::mustache::MapBuilder::new()
        .insert_str("list_id", list_id.to_string())
        .insert_str("cb", cb)
        .insert_str("back", back)
        .insert_str("l_title", tr(lang, "sign-in"))
        .insert_str("l_user", tr(lang, "username"))
        .insert_str("l_pass", tr(lang, "password"))
        .insert_str("l_keep", tr(lang, "keep-signed-in"))
        .insert_bool("has_error", error.is_some())
        .insert_str("error", error.unwrap_or(""))
        .build();
    page(lang, &tr(lang, "sign-in"), &plan_ai_html::render_data(LOGIN_TPL, &data))
}

// The `<meta http-equiv="refresh">` reloads the GET login page (carrying cb/back)
// once the countdown elapses — browsers honour it inside <body>. cb/back are
// percent-encoded before templating, so the auto-escape leaves them intact.
const LOCKED_TPL: &str = r#"<meta http-equiv="refresh" content="{{secs}};url=/agency/basic/{{list_id}}/login?cb={{cb}}&back={{back}}">
<h1 class="h-page">{{l_title}}</h1>
<div class="err">{{l_msg}}</div>
<p class="help" style="margin-top:.6rem">{{l_retry}}</p>"#;

fn locked_form_html(lang: Lang, list_id: Uuid, cb: &str, back: &str, secs: u64) -> Html<String> {
    let data = plan_ai_html::mustache::MapBuilder::new()
        .insert_str("list_id", list_id.to_string())
        .insert_str("cb", enc(cb))
        .insert_str("back", enc(back))
        .insert_str("secs", secs.to_string())
        .insert_str("l_title", tr(lang, "sign-in"))
        .insert_str("l_msg", tr_args(lang, "too-many-attempts", &[("secs", &secs.to_string())]))
        .insert_str("l_retry", tr(lang, "locked-retry"))
        .build();
    page(lang, &tr(lang, "sign-in"), &plan_ai_html::render_data(LOCKED_TPL, &data))
}

// -- handlers --

async fn login_page(
    State(pool): State<PgPool>,
    Path(list_id): Path<Uuid>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
    jar: CookieJar,
) -> (CookieJar, Response) {
    let lang = lang(&headers);
    let back = q.get("back").cloned().unwrap_or_default();
    let cb = q.get("cb").cloned().unwrap_or_default();
    if !cb_host_ok(&pool, list_id, &cb).await {
        return (jar, (StatusCode::BAD_REQUEST, "invalid callback").into_response());
    }
    // Brute-force gate: a locked-out subnet only sees the countdown (which the
    // meta refresh reloads here when it expires).
    if let Some(secs) = client_ip(&headers).and_then(locked_for) {
        return (jar, locked_form_html(lang, list_id, &cb, &back, secs).into_response());
    }
    // Silent SSO: already proved this list on this browser → hand off, no form.
    // One-shot: guarded by a short-lived marker so that if the handed-off
    // session still can't satisfy the gate (e.g. stale cookies), we fall back to
    // the form instead of looping the redirect dance forever.
    if jar.get(SSO_TRY_COOKIE).is_none() {
        if let Some(sso_id) = sso_from_cookie(&pool, &jar).await {
            if let Some(username) = sso_list_username(&pool, sso_id, list_id).await {
                let resp = handoff_redirect(sso_id, list_id, &username, true, &cb, &back);
                return (jar.add(sso_try_cookie()), resp);
            }
        }
    }
    // Show the form and clear the one-shot marker.
    let resp = login_form_html(lang, list_id, &cb, &back, None).into_response();
    (jar.remove(Cookie::build(SSO_TRY_COOKIE).path("/")), resp)
}

async fn login_submit(
    State(pool): State<PgPool>,
    Path(list_id): Path<Uuid>,
    headers: HeaderMap,
    jar: CookieJar,
    body: String,
) -> (CookieJar, Response) {
    let lang = lang(&headers);
    let form = parse_form(&body);
    let username = form.get("username").cloned().unwrap_or_default();
    let password = form.get("password").cloned().unwrap_or_default();
    let keep = form.get("keep").map(|v| v == "on").unwrap_or(false);
    let back = form.get("back").cloned().unwrap_or_default();
    let cb = form.get("cb").cloned().unwrap_or_default();

    if !cb_host_ok(&pool, list_id, &cb).await {
        return (jar, (StatusCode::BAD_REQUEST, "invalid callback").into_response());
    }
    let ip = client_ip(&headers);
    // Refuse before touching credentials while the subnet is locked.
    if let Some(secs) = ip.and_then(locked_for) {
        return (jar, locked_form_html(lang, list_id, &cb, &back, secs).into_response());
    }
    if !creds_ok(&pool, list_id, &username, &password).await {
        let secs = ip.map(record_failure).unwrap_or(0);
        let resp = if secs > 0 {
            locked_form_html(lang, list_id, &cb, &back, secs)
        } else {
            login_form_html(lang, list_id, &cb, &back, Some(&tr(lang, "invalid-credentials")))
        };
        return (jar, resp.into_response());
    }
    if let Some(ip) = ip {
        record_success(ip);
    }

    let (sso_id, set_cookie) = match ensure_sso(&pool, &jar, keep).await {
        Ok(v) => v,
        Err(e) => return (jar, (StatusCode::INTERNAL_SERVER_ERROR, e).into_response()),
    };
    let _ = sqlx::query(
        "INSERT INTO basic_auth_sso_lists (sso_id, list_id, username) VALUES ($1, $2, $3) \
         ON CONFLICT (sso_id, list_id) DO UPDATE SET username = $3",
    )
    .bind(sso_id)
    .bind(list_id)
    .bind(&username)
    .execute(&pool)
    .await;

    let resp = handoff_redirect(sso_id, list_id, &username, keep, &cb, &back);
    let jar = match set_cookie {
        Some(c) => jar.add(c),
        None => jar,
    };
    (jar, resp)
}

async fn logout_page(
    State(pool): State<PgPool>,
    Path(list_id): Path<Uuid>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    // The proxy already dropped this list from the identity before redirecting
    // here; this page is purely informational.
    let lang = lang(&headers);
    let back = q.get("back").cloned().unwrap_or_default();
    let cb = q.get("cb").cloned().unwrap_or_default();
    if !cb.is_empty() && !cb_host_ok(&pool, list_id, &cb).await {
        return (StatusCode::BAD_REQUEST, "invalid callback").into_response();
    }
    let login = format!("{cb}/login?back={}", enc(&back));
    let body = format!(
        "<h1 class=\"h-page\">{title}</h1>\
         <p class=\"help\">{body_text}</p>\
         <p style=\"margin-top:1rem\"><a class=\"btn btn-primary btn-lg\" href=\"{login}\">{again}</a></p>\
         <p class=\"help\" style=\"margin-top:1rem\"><a class=\"link\" href=\"/agency/basic/profile\">{manage}</a>{back_link}</p>",
        title = tr(lang, "signed-out-title"),
        body_text = tr(lang, "signed-out-body"),
        login = esc(&login),
        again = tr(lang, "sign-in-again"),
        manage = tr(lang, "manage-sessions"),
        back_link = if back.is_empty() {
            String::new()
        } else {
            format!(
                " · <a class=\"link\" href=\"{}\">{}</a>",
                esc(&back),
                tr(lang, "back-to-site")
            )
        },
    );
    page(lang, &tr(lang, "signed-out-title"), &body).into_response()
}

async fn profile_page(
    State(pool): State<PgPool>,
    Path(list_id): Path<Uuid>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
    jar: CookieJar,
) -> Response {
    let lang = lang(&headers);
    let back = q.get("back").cloned().unwrap_or_default();
    let cb = q.get("cb").cloned().unwrap_or_default();
    if !cb_host_ok(&pool, list_id, &cb).await {
        return (StatusCode::BAD_REQUEST, "invalid callback").into_response();
    }
    let sso_id = sso_from_cookie(&pool, &jar).await;
    let this_user = match sso_id {
        Some(id) => sso_list_username(&pool, id, list_id).await,
        None => None,
    };

    let status = match &this_user {
        Some(u) => format!(
            "<p>{signed_in}</p>\
             <p style=\"margin-top:1rem\"><a class=\"btn btn-secondary btn-lg\" href=\"{cb}/logout?back={back}\">{sign_out}</a></p>",
            signed_in = tr_args(lang, "signed-in-as", &[("user", &esc(u))]),
            cb = esc(&cb),
            back = enc(&back),
            sign_out = tr(lang, "sign-out-area"),
        ),
        None => format!(
            "<p class=\"help\">{not_signed_in}</p>\
             <p style=\"margin-top:1rem\"><a class=\"btn btn-primary btn-lg\" href=\"{cb}/login?back={back}\">{sign_in}</a></p>",
            not_signed_in = tr(lang, "not-signed-in-area"),
            cb = esc(&cb),
            back = enc(&back),
            sign_in = tr(lang, "sign-in"),
        ),
    };

    // Other lists this identity is signed into.
    let mut others = String::new();
    if let Some(id) = sso_id {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT l.name, sl.username FROM basic_auth_sso_lists sl \
             JOIN basic_auth_lists l ON l.id = sl.list_id \
             WHERE sl.sso_id = $1 AND sl.list_id <> $2 ORDER BY l.name",
        )
        .bind(id)
        .bind(list_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        if !rows.is_empty() {
            others.push_str(&format!(
                "<p class=\"help\" style=\"margin-top:1rem\">{}</p><ul class=\"help\">",
                tr(lang, "also-signed-in")
            ));
            for (name, user) in rows {
                others.push_str(&format!(
                    "<li>{}</li>",
                    tr_args(lang, "item-as", &[("name", &esc(&name)), ("user", &esc(&user))])
                ));
            }
            others.push_str("</ul>");
        }
    }

    let body = format!(
        "<h1 class=\"h-page\">{title}</h1>{status}{others}\
         <p class=\"help\" style=\"margin-top:1rem\"><a class=\"link\" href=\"/agency/basic/profile\">{manage}</a>{back_link}</p>",
        title = tr(lang, "profile-title"),
        manage = tr(lang, "manage-sessions"),
        back_link = if back.is_empty() {
            String::new()
        } else {
            format!(
                " · <a class=\"link\" href=\"{}\">{}</a>",
                esc(&back),
                tr(lang, "back-to-site")
            )
        },
    );
    page(lang, &tr(lang, "profile-title"), &body).into_response()
}

async fn global_profile(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    jar: CookieJar,
) -> Response {
    let lang = lang(&headers);
    let empty = || {
        page(
            lang,
            &tr(lang, "sessions-title"),
            &format!(
                "<h1 class=\"h-page\">{}</h1><p class=\"help\">{}</p>",
                tr(lang, "sessions-title"),
                tr(lang, "not-signed-in-anything"),
            ),
        )
        .into_response()
    };

    let Some(sso_id) = sso_from_cookie(&pool, &jar).await else {
        return empty();
    };
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT l.id, l.name, sl.username FROM basic_auth_sso_lists sl \
         JOIN basic_auth_lists l ON l.id = sl.list_id \
         WHERE sl.sso_id = $1 ORDER BY l.name",
    )
    .bind(sso_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return empty();
    }

    let sign_out = tr(lang, "sign-out");
    let mut items = String::from("<ul style=\"list-style:none;padding:0;margin:.5rem 0 0\">");
    for (id, name, user) in rows {
        items.push_str(&format!(
            "<li style=\"display:flex;justify-content:space-between;align-items:center;\
             gap:1rem;padding:.6rem 0;border-bottom:1px solid rgb(var(--c-line))\">\
             <span>{label}</span>\
             <form method=post action=\"/agency/basic/profile/logout\" style=\"margin:0\">\
             <input type=hidden name=list_id value=\"{id}\">\
             <button class=\"btn btn-secondary btn-sm\" type=submit>{sign_out}</button></form></li>",
            label = tr_args(lang, "item-as", &[("name", &esc(&name)), ("user", &esc(&user))]),
        ));
    }
    items.push_str("</ul>");

    let body = format!(
        "<h1 class=\"h-page\">{title}</h1>\
         <p class=\"help\">{areas}</p>{items}\
         <form method=post action=\"/agency/basic/profile/logout\" style=\"margin-top:1.25rem\">\
         <input type=hidden name=all value=1>\
         <button class=\"btn btn-danger btn-lg\" type=submit style=\"width:100%\">{all_out}</button></form>",
        title = tr(lang, "sessions-title"),
        areas = tr(lang, "areas-signed-in"),
        all_out = tr(lang, "sign-out-everything"),
    );
    page(lang, &tr(lang, "sessions-title"), &body).into_response()
}

async fn global_logout(
    State(pool): State<PgPool>,
    jar: CookieJar,
    body: String,
) -> (CookieJar, Response) {
    let form = parse_form(&body);
    let Some(sso_id) = sso_from_cookie(&pool, &jar).await else {
        return (jar, Redirect::to("/agency/basic/profile").into_response());
    };

    if form.get("all").map(|v| v == "1").unwrap_or(false) {
        // Nuclear: drop the whole identity everywhere + clear the agency cookie.
        let _ = sqlx::query("DELETE FROM basic_auth_sso WHERE id = $1")
            .bind(sso_id)
            .execute(&pool)
            .await;
        let jar = jar.remove(Cookie::build(AGENCY_COOKIE).path("/"));
        return (jar, Redirect::to("/agency/basic/profile").into_response());
    }

    if let Some(list_id) = form.get("list_id").and_then(|s| Uuid::parse_str(s).ok()) {
        let _ = sqlx::query(
            "DELETE FROM basic_auth_sso_lists WHERE sso_id = $1 AND list_id = $2",
        )
        .bind(sso_id)
        .bind(list_id)
        .execute(&pool)
        .await;
    }
    (jar, Redirect::to("/agency/basic/profile").into_response())
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
    fn lock_secs_doubles_after_free_attempts() {
        assert_eq!(lock_secs(1), 0);
        assert_eq!(lock_secs(2), 0);
        assert_eq!(lock_secs(3), 30);
        assert_eq!(lock_secs(4), 60);
        assert_eq!(lock_secs(5), 120);
        // never overflows
        assert!(lock_secs(u32::MAX) >= 30);
    }

    #[test]
    fn subnet_key_masks_v6_to_48_keeps_v4_host() {
        let a: IpAddr = "2001:db8:abcd:1234::1".parse().unwrap();
        let b: IpAddr = "2001:db8:abcd:ffff::9".parse().unwrap();
        // same /48 → same key
        assert_eq!(subnet_key(a), subnet_key(b));
        let c: IpAddr = "2001:db8:abce:0::1".parse().unwrap();
        assert_ne!(subnet_key(a), subnet_key(c));
        // v4 keyed to the exact host
        let v4a: IpAddr = "203.0.113.7".parse().unwrap();
        let v4b: IpAddr = "203.0.113.8".parse().unwrap();
        assert_ne!(subnet_key(v4a), subnet_key(v4b));
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

#[cfg(test)]
mod password_tests {
    use super::{hash_password, sha256_hex, verify_password};

    #[test]
    fn argon2_roundtrip() {
        let h = hash_password("correct horse battery staple").unwrap();
        assert!(h.starts_with("$argon2"));
        let (ok, rehash) = verify_password(&h, "correct horse battery staple");
        assert!(ok && !rehash);
        let (bad, _) = verify_password(&h, "wrong");
        assert!(!bad);
    }

    #[test]
    fn legacy_sha256_verifies_and_flags_rehash() {
        let legacy = sha256_hex("hunter2");
        let (ok, rehash) = verify_password(&legacy, "hunter2");
        assert!(ok, "legacy hash must still verify");
        assert!(rehash, "legacy hash must be flagged for upgrade");
        let (bad, _) = verify_password(&legacy, "nope");
        assert!(!bad);
    }
}
