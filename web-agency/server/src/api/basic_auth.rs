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


/// Percent-encode a value for use in a query string.
fn enc(s: &str) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

use plan_ai_html::escape as esc;

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
    match stored {
        Some(hash) => constant_time_eq(&hash, &sha256_hex(password)),
        None => false,
    }
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

/// Wrap a pre-rendered body in the plan-ai-design page chrome.
fn page(title: &str, body: &str) -> Html<String> {
    Html(plan_ai_html::Page::new(title, body).render())
}

const LOGIN_TPL: &str = r#"<h1 class="h-page">Sign in</h1>
<form method="post" action="/agency/basic/{{list_id}}/login">
<input type="hidden" name="cb" value="{{cb}}">
<input type="hidden" name="back" value="{{back}}">
<label class="label" for="u">Username</label>
<input class="input" id="u" type="text" name="username" autofocus autocomplete="username">
<label class="label" for="p" style="margin-top:.75rem">Password</label>
<input class="input" id="p" type="password" name="password" autocomplete="current-password">
<label class="help" style="display:flex;align-items:center;gap:.45rem;margin-top:.8rem"><input type="checkbox" name="keep" value="on"> Keep me signed in</label>
{{#has_error}}<div class="err" style="margin-top:.6rem">{{error}}</div>{{/has_error}}
<button class="btn btn-primary btn-lg" type="submit" style="width:100%;margin-top:1rem">Sign in</button>
</form>"#;

fn login_form_html(list_id: Uuid, cb: &str, back: &str, error: Option<&str>) -> Html<String> {
    let data = plan_ai_html::mustache::MapBuilder::new()
        .insert_str("list_id", list_id.to_string())
        .insert_str("cb", cb)
        .insert_str("back", back)
        .insert_bool("has_error", error.is_some())
        .insert_str("error", error.unwrap_or(""))
        .build();
    page("Sign in", &plan_ai_html::render_data(LOGIN_TPL, &data))
}

// -- handlers --

async fn login_page(
    State(pool): State<PgPool>,
    Path(list_id): Path<Uuid>,
    Query(q): Query<HashMap<String, String>>,
    jar: CookieJar,
) -> (CookieJar, Response) {
    let back = q.get("back").cloned().unwrap_or_default();
    let cb = q.get("cb").cloned().unwrap_or_default();
    if !cb_host_ok(&pool, list_id, &cb).await {
        return (jar, (StatusCode::BAD_REQUEST, "invalid callback").into_response());
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
    let resp = login_form_html(list_id, &cb, &back, None).into_response();
    (jar.remove(Cookie::build(SSO_TRY_COOKIE).path("/")), resp)
}

async fn login_submit(
    State(pool): State<PgPool>,
    Path(list_id): Path<Uuid>,
    jar: CookieJar,
    body: String,
) -> (CookieJar, Response) {
    let form = parse_form(&body);
    let username = form.get("username").cloned().unwrap_or_default();
    let password = form.get("password").cloned().unwrap_or_default();
    let keep = form.get("keep").map(|v| v == "on").unwrap_or(false);
    let back = form.get("back").cloned().unwrap_or_default();
    let cb = form.get("cb").cloned().unwrap_or_default();

    if !cb_host_ok(&pool, list_id, &cb).await {
        return (jar, (StatusCode::BAD_REQUEST, "invalid callback").into_response());
    }
    if !creds_ok(&pool, list_id, &username, &password).await {
        let resp = login_form_html(list_id, &cb, &back, Some("Invalid username or password"))
            .into_response();
        return (jar, resp);
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
) -> Response {
    // The proxy already dropped this list from the identity before redirecting
    // here; this page is purely informational.
    let back = q.get("back").cloned().unwrap_or_default();
    let cb = q.get("cb").cloned().unwrap_or_default();
    if !cb.is_empty() && !cb_host_ok(&pool, list_id, &cb).await {
        return (StatusCode::BAD_REQUEST, "invalid callback").into_response();
    }
    let login = format!("{cb}/login?back={}", enc(&back));
    let body = format!(
        "<h1 class=\"h-page\">Signed out</h1>\
         <p class=\"help\">You've been signed out of this area.</p>\
         <p style=\"margin-top:1rem\"><a class=\"btn btn-primary btn-lg\" href=\"{}\">Sign in again</a></p>\
         <p class=\"help\" style=\"margin-top:1rem\"><a class=\"link\" href=\"/agency/basic/profile\">Manage all sessions</a>{}</p>",
        esc(&login),
        if back.is_empty() {
            String::new()
        } else {
            format!(" · <a class=\"link\" href=\"{}\">Back to site</a>", esc(&back))
        },
    );
    page("Signed out", &body).into_response()
}

async fn profile_page(
    State(pool): State<PgPool>,
    Path(list_id): Path<Uuid>,
    Query(q): Query<HashMap<String, String>>,
    jar: CookieJar,
) -> Response {
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
            "<p>Signed in as <strong>{}</strong>.</p>\
             <p style=\"margin-top:1rem\"><a class=\"btn btn-secondary btn-lg\" href=\"{}/logout?back={}\">Sign out of this area</a></p>",
            esc(u),
            esc(&cb),
            enc(&back),
        ),
        None => format!(
            "<p class=\"help\">Not signed in to this area.</p>\
             <p style=\"margin-top:1rem\"><a class=\"btn btn-primary btn-lg\" href=\"{}/login?back={}\">Sign in</a></p>",
            esc(&cb),
            enc(&back),
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
            others.push_str("<p class=\"help\" style=\"margin-top:1rem\">Also signed in:</p><ul class=\"help\">");
            for (name, user) in rows {
                others.push_str(&format!("<li>{} as {}</li>", esc(&name), esc(&user)));
            }
            others.push_str("</ul>");
        }
    }

    let body = format!(
        "<h1 class=\"h-page\">Profile</h1>{status}{others}\
         <p class=\"help\" style=\"margin-top:1rem\"><a class=\"link\" href=\"/agency/basic/profile\">Manage all sessions</a>{}</p>",
        if back.is_empty() {
            String::new()
        } else {
            format!(" · <a class=\"link\" href=\"{}\">Back to site</a>", esc(&back))
        },
    );
    page("Profile", &body).into_response()
}

async fn global_profile(State(pool): State<PgPool>, jar: CookieJar) -> Response {
    let Some(sso_id) = sso_from_cookie(&pool, &jar).await else {
        return page(
            "Sessions",
            "<h1 class=\"h-page\">Sessions</h1><p class=\"help\">You're not signed in to anything.</p>",
        )
        .into_response();
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
        return page(
            "Sessions",
            "<h1 class=\"h-page\">Sessions</h1><p class=\"help\">You're not signed in to anything.</p>",
        )
        .into_response();
    }

    let mut items = String::from("<ul style=\"list-style:none;padding:0;margin:.5rem 0 0\">");
    for (id, name, user) in rows {
        items.push_str(&format!(
            "<li style=\"display:flex;justify-content:space-between;align-items:center;\
             gap:1rem;padding:.6rem 0;border-bottom:1px solid rgb(var(--c-line))\">\
             <span>{} <span class=\"help\">as {}</span></span>\
             <form method=post action=\"/agency/basic/profile/logout\" style=\"margin:0\">\
             <input type=hidden name=list_id value=\"{}\">\
             <button class=\"btn btn-secondary btn-sm\" type=submit>Sign out</button></form></li>",
            esc(&name),
            esc(&user),
            id,
        ));
    }
    items.push_str("</ul>");

    let body = format!(
        "<h1 class=\"h-page\">Sessions</h1>\
         <p class=\"help\">Areas you're currently signed in to:</p>{items}\
         <form method=post action=\"/agency/basic/profile/logout\" style=\"margin-top:1.25rem\">\
         <input type=hidden name=all value=1>\
         <button class=\"btn btn-danger btn-lg\" type=submit style=\"width:100%\">Sign out of everything</button></form>"
    );
    page("Sessions", &body).into_response()
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
