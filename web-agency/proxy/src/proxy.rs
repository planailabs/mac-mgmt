use arc_swap::ArcSwap;
use async_trait::async_trait;
use pingora::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

/// A route target — either direct (local) or relay (via mac-mgmt relay).
#[derive(Debug, Clone)]
pub enum Route {
    /// Direct TCP upstream (local webspace, agency server).
    Direct(String),
    /// Static folder served by the agency origin: proxy to `upstream` (the
    /// agency server), passing the Host through and injecting the internal token
    /// + webspace id so the server serves the folder's files.
    StaticOrigin { upstream: String, webspace_id: String },
    /// Relay tunnel — proxy via HTTPS to relay server with token auth.
    Relay {
        /// host:port to connect to
        upstream: String,
        /// Full relay URL (e.g. https://abc123-ollama.relay.plan.ai)
        url: String,
        /// Relay hostname for the Host header
        relay_host: String,
        /// Whether to use TLS to the relay
        tls: bool,
        /// SNI hostname for TLS
        sni: String,
        /// Proxy token to inject as X-Proxy-Token header
        proxy_token: String,
    },
}

/// Auth mode for a route.
#[derive(Debug, Clone)]
pub enum AuthMode {
    None,
    Oidc { org_id: uuid::Uuid },
    /// Cookie-based basic-auth gate. The proxy holds only the credential list
    /// id; credentials are validated on the agency login form, and sessions are
    /// verified per-request via the internal API.
    Basic { list_id: uuid::Uuid },
}

/// Per-request context — carries relay info from upstream_peer to upstream_request_filter.
#[derive(Default)]
pub struct RequestCtx {
    relay: Option<RelayCtx>,
    /// Folder mount prefix (e.g. "/api") to strip from the request path before
    /// forwarding upstream. "/" or empty means pass-through.
    mount_prefix: String,
    /// When set, this request is for a static folder served by the agency
    /// origin: passthrough Host and inject the internal token + this webspace id.
    origin_static: Option<String>,
}

struct RelayCtx {
    host: String,
    proxy_token: String,
    path_prefix: String,
}

/// One mounted folder under a host: its path prefix, route target, and auth.
pub type Folder = (String, Route, AuthMode);

/// Whether a request's host is routable, unknown, or the table is still empty.
enum HostState {
    Known,
    Unknown,
    Starting,
}

/// The Pingora HTTP proxy that routes requests by Host header, then by the
/// longest matching folder path prefix. Each host maps to a list of folders
/// sorted by descending prefix length (longest match wins).
pub struct WebAgencyProxy {
    pub routes: Arc<ArcSwap<HashMap<String, Vec<Folder>>>>,
    pub agency_domain: String,
    pub internal_token: String,
    /// Base URL of the agency server's internal API (basic-auth verify/session).
    pub server_url: String,
    /// HTTP client for internal API calls made during request handling.
    pub client: reqwest::Client,
}

/// True if `prefix` is a path-prefix of `path` ("/" matches everything).
/// Trailing slashes on the prefix are ignored so "/static" and "/static/" both
/// match "/static" and "/static/...".
fn path_matches(prefix: &str, path: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    prefix.is_empty() || path == prefix || path.starts_with(&format!("{prefix}/"))
}

/// Pick the folder whose path prefix best matches `path`. `folders` must be
/// sorted by descending prefix length so the first match is the longest.
fn match_folder<'a>(folders: &'a [Folder], path: &str) -> Option<&'a Folder> {
    folders.iter().find(|(prefix, _, _)| path_matches(prefix, path))
}

/// Reverse the request-path rewrite for a root-relative redirect Location:
/// strip the relay URL's path prefix we prepended, then re-add the folder mount
/// prefix we stripped. Inverse of the rewrite in `upstream_request_filter`.
fn rewrite_redirect_location(loc: &str, relay_prefix: &str, mount_prefix: &str) -> String {
    let mut path = loc;
    if !relay_prefix.is_empty() {
        if let Some(rest) = path.strip_prefix(relay_prefix) {
            path = if rest.is_empty() { "/" } else { rest };
        }
    }
    if !mount_prefix.is_empty() && mount_prefix != "/" {
        format!("{}{path}", mount_prefix.trim_end_matches('/'))
    } else {
        path.to_string()
    }
}

/// Normalize a mount prefix for URL building: "" for the root folder ("/" or
/// ""), otherwise the prefix without a trailing slash (e.g. "/customer").
fn mount_prefix_clean(mount: &str) -> String {
    let m = mount.trim_end_matches('/');
    if m.is_empty() { String::new() } else { m.to_string() }
}

/// Get a raw (still percent-encoded) query parameter value by name.
fn query_param(query: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix(&needle))
        .map(|v| v.to_string())
}

/// True if `back` is same-origin with `host` (blocks open redirects).
fn validate_back(back: &str, host: &str) -> bool {
    back == format!("https://{host}") || back.starts_with(&format!("https://{host}/"))
}

/// Strip a folder mount prefix from a path (a folder at "/api" sees "/").
/// Trailing slashes on the prefix are ignored, so the stripped path keeps its
/// leading slash (e.g. prefix "/static/", path "/static/da/" → "/da/").
fn strip_mount_prefix(path: &str, prefix: &str) -> String {
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        return path.to_string();
    }
    let stripped = path.strip_prefix(prefix).unwrap_or(path);
    if stripped.is_empty() {
        "/".to_string()
    } else {
        stripped.to_string()
    }
}

impl WebAgencyProxy {
    /// Cookie-based basic-auth gate. Intercepts the `cgi-webagency/basic/*`
    /// control paths (login/logout/profile) and otherwise verifies the
    /// `__basic_session` cookie against the internal API, redirecting to the
    /// login dance when it's missing or not authorized for this list.
    async fn handle_basic(
        &self,
        session: &mut Session,
        host: &str,
        mount: &str,
        list_id: uuid::Uuid,
    ) -> Result<bool> {
        let mp = mount_prefix_clean(mount);
        let path_only = session.req_header().uri.path().to_string();
        let sub = strip_mount_prefix(&path_only, mount);

        // JSON whoami at the cgi base path itself.
        if sub == "/cgi-webagency/basic" || sub == "/cgi-webagency/basic/" {
            return self.handle_basic_whoami(session, list_id).await;
        }
        if let Some(action) = sub.strip_prefix("/cgi-webagency/basic/") {
            return self
                .handle_basic_cgi(session, host, &mp, list_id, action)
                .await;
        }

        // Gate: verify the session cookie for this list.
        let cookies = session
            .req_header()
            .headers
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        for tok in cookie_values(cookies, "__basic_session") {
            if self.basic_verify_info(&tok, list_id).await.is_some() {
                return Ok(false); // authorized — pass through
            }
        }

        // Unauthorized → start the login dance at the folder's cgi entrypoint.
        let full = format!("https://{host}{}", safe_path_and_query(&session.req_header().uri));
        let loc = format!(
            "https://{host}{mp}/cgi-webagency/basic/login?back={}",
            urlencoding::encode(&full)
        );
        self.send_redirect(session, &loc, None).await?;
        Ok(true)
    }

    async fn handle_basic_cgi(
        &self,
        session: &mut Session,
        host: &str,
        mp: &str,
        list_id: uuid::Uuid,
        action: &str,
    ) -> Result<bool> {
        let query = safe_path_and_query(&session.req_header().uri)
            .split_once('?')
            .map(|(_, q)| q.to_string())
            .unwrap_or_default();
        let cgi_base = format!("https://{host}{mp}/cgi-webagency/basic");
        let back = match query_param(&query, "back") {
            Some(b) => urlencoding::decode(&b).map(|c| c.into_owned()).unwrap_or(b),
            None => format!("https://{host}{mp}/"),
        };

        match action {
            "login" => {
                if let Some(token) = query_param(&query, "token") {
                    // Callback from the agency form: persist a session, set cookie.
                    match self.basic_create_session(&token).await {
                        Some((sess, max_age)) => {
                            let dest = if validate_back(&back, host) {
                                back
                            } else {
                                format!("https://{host}{mp}/")
                            };
                            let mut builder = cookie::Cookie::build(("__basic_session", sess))
                                .path("/")
                                .http_only(true)
                                .secure(true)
                                .same_site(cookie::SameSite::Lax);
                            if let Some(ma) = max_age {
                                builder = builder.max_age(cookie::time::Duration::seconds(ma));
                            }
                            let cookie = builder.build().to_string();
                            self.send_redirect(session, &dest, Some(&cookie)).await?;
                        }
                        None => {
                            // Handoff invalid/expired — restart the login.
                            let loc = format!(
                                "{cgi_base}/login?back={}",
                                urlencoding::encode(&back)
                            );
                            self.send_redirect(session, &loc, None).await?;
                        }
                    }
                } else {
                    // Initiate: hand off to the agency login form.
                    let loc = format!(
                        "https://{}/agency/basic/{}/login?back={}&cb={}",
                        self.agency_domain,
                        list_id,
                        urlencoding::encode(&back),
                        urlencoding::encode(&cgi_base),
                    );
                    self.send_redirect(session, &loc, None).await?;
                }
            }
            "logout" => {
                let cookies = session
                    .req_header()
                    .headers
                    .get("cookie")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if let Some(tok) = extract_cookie(cookies, "__basic_session") {
                    self.basic_logout(&tok, list_id).await;
                }
                let loc = format!(
                    "https://{}/agency/basic/{}/logout?back={}&cb={}",
                    self.agency_domain,
                    list_id,
                    urlencoding::encode(&back),
                    urlencoding::encode(&cgi_base),
                );
                self.send_redirect(session, &loc, None).await?;
            }
            "profile" => {
                let loc = format!(
                    "https://{}/agency/basic/{}/profile?back={}&cb={}",
                    self.agency_domain,
                    list_id,
                    urlencoding::encode(&back),
                    urlencoding::encode(&cgi_base),
                );
                self.send_redirect(session, &loc, None).await?;
            }
            _ => {
                let mut resp = pingora::http::ResponseHeader::build(404, None).map_err(|e| {
                    pingora::Error::because(pingora::ErrorType::InternalError, "build 404", e)
                })?;
                let _ = resp.insert_header("Content-Length", "0");
                session.write_response_header(Box::new(resp), false).await?;
                session
                    .write_response_body(Some(bytes::Bytes::new()), true)
                    .await?;
            }
        }
        Ok(true)
    }

    /// 302 redirect, optionally setting a cookie. Short-circuits the request.
    async fn send_redirect(
        &self,
        session: &mut Session,
        location: &str,
        set_cookie: Option<&str>,
    ) -> Result<()> {
        let mut resp = pingora::http::ResponseHeader::build(302, None).map_err(|e| {
            pingora::Error::because(pingora::ErrorType::InternalError, "build 302 response", e)
        })?;
        let _ = resp.insert_header("Location", location);
        if let Some(c) = set_cookie {
            let _ = resp.insert_header("Set-Cookie", c);
        }
        let _ = resp.insert_header("Content-Length", "0");
        session.write_response_header(Box::new(resp), false).await?;
        session
            .write_response_body(Some(bytes::Bytes::new()), true)
            .await?;
        Ok(())
    }

    /// Verify a `__basic_session` cookie grants access to `list_id`. Returns
    /// `(username, list_name)` when valid, `None` otherwise. Fails closed (None)
    /// if the internal API is unreachable.
    async fn basic_verify_info(
        &self,
        token: &str,
        list_id: uuid::Uuid,
    ) -> Option<(String, Option<String>)> {
        let url = format!("{}/api/internal/basic/verify", self.server_url);
        let r = self
            .client
            .post(&url)
            .bearer_auth(&self.internal_token)
            .json(&serde_json::json!({ "session_token": token, "list_id": list_id }))
            .send()
            .await
            .ok()?;
        if !r.status().is_success() {
            return None;
        }
        let v = r.json::<serde_json::Value>().await.ok()?;
        if v.get("valid").and_then(|b| b.as_bool()) != Some(true) {
            return None;
        }
        let username = v.get("username").and_then(|x| x.as_str())?.to_string();
        let list_name = v
            .get("list_name")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        Some((username, list_name))
    }

    /// JSON identity at the cgi base path: `{username, list_id, list_name}` when
    /// signed in, else 401 `{error}`.
    async fn handle_basic_whoami(
        &self,
        session: &mut Session,
        list_id: uuid::Uuid,
    ) -> Result<bool> {
        let cookies = session
            .req_header()
            .headers
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let mut info = None;
        for tok in cookie_values(cookies, "__basic_session") {
            info = self.basic_verify_info(&tok, list_id).await;
            if info.is_some() {
                break;
            }
        }
        let (status, body) = match info {
            Some((username, list_name)) => (
                200u16,
                serde_json::json!({
                    "username": username,
                    "list_id": list_id,
                    "list_name": list_name,
                })
                .to_string(),
            ),
            None => (401u16, serde_json::json!({ "error": "not signed in" }).to_string()),
        };
        let bytes = bytes::Bytes::from(body);
        let mut resp = pingora::http::ResponseHeader::build(status, None).map_err(|e| {
            pingora::Error::because(pingora::ErrorType::InternalError, "build whoami response", e)
        })?;
        let _ = resp.insert_header("Content-Type", "application/json");
        let _ = resp.insert_header("Content-Length", &bytes.len().to_string());
        session.write_response_header(Box::new(resp), false).await?;
        session.write_response_body(Some(bytes), true).await?;
        Ok(true)
    }

    /// Exchange a signed handoff token for a persisted session token.
    async fn basic_create_session(&self, handoff: &str) -> Option<(String, Option<i64>)> {
        let url = format!("{}/api/internal/basic/session", self.server_url);
        let r = self
            .client
            .post(&url)
            .bearer_auth(&self.internal_token)
            .json(&serde_json::json!({ "handoff_token": handoff }))
            .send()
            .await
            .ok()?;
        if !r.status().is_success() {
            return None;
        }
        let v = r.json::<serde_json::Value>().await.ok()?;
        let sess = v.get("session_token")?.as_str()?.to_string();
        let max_age = v.get("max_age_secs").and_then(|x| x.as_i64());
        Some((sess, max_age))
    }

    /// Drop the current list's authorization for the session's identity.
    async fn basic_logout(&self, token: &str, list_id: uuid::Uuid) {
        let url = format!("{}/api/internal/basic/logout", self.server_url);
        let _ = self
            .client
            .post(&url)
            .bearer_auth(&self.internal_token)
            .json(&serde_json::json!({ "session_token": token, "list_id": list_id }))
            .send()
            .await;
    }

    async fn handle_oidc_auth(
        &self,
        session: &mut Session,
        host: &str,
        org_id: &uuid::Uuid,
    ) -> Result<bool> {
        // 1. Check for existing proxy-gate cookie
        let cookies = session
            .req_header()
            .headers
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if let Some(cookie_val) = extract_cookie(cookies, "__proxy_gate") {
            if self.validate_gate_cookie(&cookie_val, org_id) {
                return Ok(false); // valid session
            }
        }

        // 2. Check for ?__pg_token= callback from proxy-gate
        let path_query = safe_path_and_query(&session.req_header().uri);
        if let Some((sig, oid, exp_ts)) = parse_gate_params(&path_query) {
            if let Ok(expected_org) = uuid::Uuid::parse_str(&oid) {
                if &expected_org == org_id && self.verify_hmac(&expected_org, exp_ts, &sig) {
                    let now = chrono::Utc::now().timestamp();
                    if exp_ts > now {
                        // Valid gate token — set cookie and redirect to clean URL
                        let clean_url = strip_gate_params(&path_query);
                        let cookie_value = self.make_gate_cookie(org_id);
                        let gate_cookie = cookie::Cookie::build(("__proxy_gate", cookie_value))
                            .path("/")
                            .http_only(true)
                            .secure(true)
                            .same_site(cookie::SameSite::Lax)
                            .max_age(cookie::time::Duration::seconds(86400))
                            .build()
                            .to_string();
                        let location = format!("https://{host}{clean_url}");
                        let mut resp =
                            pingora::http::ResponseHeader::build(302, None).map_err(|e| {
                                pingora::Error::because(
                                    pingora::ErrorType::InternalError,
                                    "build 302 response",
                                    e,
                                )
                            })?;
                        let _ = resp.insert_header("Location", &location);
                        let _ = resp.insert_header("Set-Cookie", &gate_cookie);
                        session.write_response_header(Box::new(resp), false).await?;
                        session
                            .write_response_body(Some(bytes::Bytes::new()), true)
                            .await?;
                        return Ok(true);
                    }
                }
            }
        }

        // 3. Redirect to agency OIDC login
        let return_url = format!("https://{host}{path_query}");
        let encoded = urlencoding::encode(&return_url);
        let redirect_url = format!(
            "https://{}/proxy-gate?return_url={encoded}",
            self.agency_domain
        );
        let mut resp = pingora::http::ResponseHeader::build(302, None).map_err(|e| {
            pingora::Error::because(pingora::ErrorType::InternalError, "build 302 response", e)
        })?;
        let _ = resp.insert_header("Location", &redirect_url);
        session.write_response_header(Box::new(resp), false).await?;
        session
            .write_response_body(Some(bytes::Bytes::new()), true)
            .await?;
        Ok(true)
    }

    fn verify_hmac(&self, org_id: &uuid::Uuid, expiry_ts: i64, sig: &str) -> bool {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let msg = format!("proxy-gate:{org_id}:{expiry_ts}");
        let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(self.internal_token.as_bytes()) else {
            return false;
        };
        mac.update(msg.as_bytes());
        let expected = hex::encode(mac.finalize().into_bytes());
        constant_time_eq(&expected, sig)
    }

    fn make_gate_cookie(&self, org_id: &uuid::Uuid) -> String {
        let expiry_ts = chrono::Utc::now().timestamp() + 86400; // 24 hours
        let sig = {
            use hmac::{Hmac, Mac};
            use sha2::Sha256;
            let msg = format!("proxy-gate:{org_id}:{expiry_ts}");
            let mut mac = Hmac::<Sha256>::new_from_slice(self.internal_token.as_bytes()).unwrap();
            mac.update(msg.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        };
        format!("{sig}:{org_id}:{expiry_ts}")
    }

    fn validate_gate_cookie(&self, cookie_val: &str, expected_org_id: &uuid::Uuid) -> bool {
        // Format: {sig}:{org_id}:{expiry_ts}
        let parts: Vec<&str> = cookie_val.splitn(3, ':').collect();
        if parts.len() != 3 {
            return false;
        }
        let (sig, oid_str, exp_str) = (parts[0], parts[1], parts[2]);
        let Ok(org_id) = uuid::Uuid::parse_str(oid_str) else {
            return false;
        };
        let Ok(expiry_ts) = exp_str.parse::<i64>() else {
            return false;
        };

        if &org_id != expected_org_id {
            return false;
        }
        let now = chrono::Utc::now().timestamp();
        if expiry_ts <= now {
            return false;
        }
        self.verify_hmac(&org_id, expiry_ts, sig)
    }
}

#[async_trait]
impl ProxyHttp for WebAgencyProxy {
    type CTX = RequestCtx;

    fn new_ctx(&self) -> Self::CTX {
        RequestCtx::default()
    }

    async fn request_filter(&self, session: &mut Session, _ctx: &mut Self::CTX) -> Result<bool> {
        // Intercept .well-known/web-agency.json on any routed host (before auth)
        let path = session.req_header().uri.path();
        if path == "/.well-known/web-agency.json" {
            let body = b"{\"service\":\"web-agency-proxy\"}";
            let mut resp = pingora::http::ResponseHeader::build(200, None).map_err(|e| {
                pingora::Error::because(pingora::ErrorType::InternalError, "build 200 response", e)
            })?;
            let _ = resp.insert_header("Content-Type", "application/json");
            let _ = resp.insert_header("Content-Length", &body.len().to_string());
            session.write_response_header(Box::new(resp), false).await?;
            session
                .write_response_body(Some(bytes::Bytes::from_static(body)), true)
                .await?;
            return Ok(true);
        }

        let host = extract_host(session);
        let req_path = session.req_header().uri.path().to_string();

        // Tarball uploads to the deploy API can stream for many minutes. The
        // default downstream read timeout (60s) would abort them mid-upload, so
        // give these requests up to an hour. Scoped to the deploy POST route.
        if req_path.starts_with("/api/v1/deploy/")
            && session.req_header().method == http::Method::POST
        {
            session.set_read_timeout(Some(std::time::Duration::from_secs(3600)));
        }

        // No route for this host at all → friendly "unknown domain" page (or a
        // "starting up" page if the route table hasn't populated yet).
        let host_state = {
            let routes = self.routes.load();
            if routes.is_empty() {
                HostState::Starting
            } else if routes.contains_key(&host) {
                HostState::Known
            } else {
                HostState::Unknown
            }
        };
        match host_state {
            HostState::Starting => {
                let lang = accept_lang(session);
                let html = plan_ai_html::error_page(lang, "starting-title", "starting-body");
                write_html(session, 503, html).await?;
                return Ok(true);
            }
            HostState::Unknown => {
                let lang = accept_lang(session);
                let html = plan_ai_html::error_page(lang, "unknown-host-title", "unknown-host-body");
                write_html(session, 404, html).await?;
                return Ok(true);
            }
            HostState::Known => {}
        }

        // Clone the matched folder's mount + auth so we don't hold the routes
        // guard across awaits. Upstream selection happens in upstream_peer.
        let (mount, auth) = {
            let routes = self.routes.load();
            match routes.get(&host).and_then(|f| match_folder(f, &req_path)) {
                Some((mount, _, auth)) => (mount.clone(), auth.clone()),
                None => return Ok(false), // unmatched path → 404 via fail_to_proxy
            }
        };

        let blocked = match &auth {
            AuthMode::None => false,
            AuthMode::Basic { list_id } => {
                self.handle_basic(session, &host, &mount, *list_id).await?
            }
            AuthMode::Oidc { org_id } => self.handle_oidc_auth(session, &host, org_id).await?,
        };
        if blocked { Ok(true) } else { Ok(false) }
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let host = extract_host(session);
        let req_path = session.req_header().uri.path().to_string();

        let routes = self.routes.load();
        match routes.get(&host).and_then(|f| match_folder(f, &req_path)) {
            Some((mount, Route::Direct(upstream), _)) => {
                ctx.mount_prefix = mount.clone();
                Ok(Box::new(HttpPeer::new(upstream.as_str(), false, String::new())))
            }
            Some((
                mount,
                Route::Relay {
                    upstream,
                    url,
                    relay_host,
                    tls,
                    sni,
                    proxy_token,
                },
                _,
            )) => {
                ctx.mount_prefix = mount.clone();
                // Store relay info in context for upstream_request_filter
                let path_prefix = url
                    .strip_prefix("https://")
                    .or_else(|| url.strip_prefix("http://"))
                    .and_then(|s| {
                        let after_host = s.find('/').map(|i| &s[i..]).unwrap_or("");
                        if after_host.is_empty() || after_host == "/" {
                            None
                        } else {
                            Some(after_host.trim_end_matches('/').to_string())
                        }
                    })
                    .unwrap_or_default();

                ctx.relay = Some(RelayCtx {
                    host: relay_host.clone(),
                    proxy_token: proxy_token.clone(),
                    path_prefix,
                });

                let peer = HttpPeer::new(upstream.as_str(), *tls, sni.clone());
                Ok(Box::new(peer))
            }
            Some((
                mount,
                Route::StaticOrigin {
                    upstream,
                    webspace_id,
                },
                _,
            )) => {
                ctx.mount_prefix = mount.clone();
                ctx.origin_static = Some(webspace_id.clone());
                Ok(Box::new(HttpPeer::new(upstream.as_str(), false, String::new())))
            }
            None => {
                tracing::debug!(host = %host, "no route found");
                Err(pingora::Error::because(
                    pingora::ErrorType::HTTPStatus(404),
                    "no route for this host",
                    pingora::Error::new(pingora::ErrorType::InternalError),
                ))
            }
        }
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut pingora::http::RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        // Strip the folder mount prefix so a folder mounted at /api sees "/".
        if !ctx.mount_prefix.is_empty() && ctx.mount_prefix != "/" {
            let stripped = strip_mount_prefix(upstream_request.uri.path(), &ctx.mount_prefix);
            let query = upstream_request
                .uri
                .query()
                .map(|q| format!("?{q}"))
                .unwrap_or_default();
            if let Ok(uri) = format!("{stripped}{query}").parse() {
                upstream_request.set_uri(uri);
            }
        }

        // Static folder served by the agency origin: keep the Host header
        // (passthrough) and tell the server which webspace to serve, trusted via
        // the internal token.
        if let Some(webspace_id) = &ctx.origin_static {
            let _ = upstream_request.insert_header("x-web-agency-webspace", webspace_id);
            let _ = upstream_request
                .insert_header("x-web-agency-internal-token", &self.internal_token);
        }

        if let Some(relay) = &ctx.relay {
            // Rewrite Host header to the relay hostname
            upstream_request
                .insert_header("host", &relay.host)
                .map_err(|e| {
                    pingora::Error::because(
                        pingora::ErrorType::InternalError,
                        "failed to set host header",
                        e,
                    )
                })?;

            // Inject proxy token (if present — relay type has one, tunnel type doesn't)
            if !relay.proxy_token.is_empty() {
                upstream_request
                    .insert_header("x-proxy-token", &relay.proxy_token)
                    .map_err(|e| {
                        pingora::Error::because(
                            pingora::ErrorType::InternalError,
                            "failed to set proxy token header",
                            e,
                        )
                    })?;
            }

            // Prepend path prefix if the relay URL had a path
            if !relay.path_prefix.is_empty() {
                let original_path = upstream_request.uri.path().to_string();
                let query = upstream_request
                    .uri
                    .query()
                    .map(|q| format!("?{q}"))
                    .unwrap_or_default();
                let new_uri = format!("{}{}{}", relay.path_prefix, original_path, query);
                if let Ok(uri) = new_uri.parse() {
                    upstream_request.set_uri(uri);
                }
            }
        }
        Ok(())
    }

    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut pingora::http::ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        // Redirects: upstream issues an absolute path in its own path space, but
        // the request path was rewritten on the way out (mount prefix stripped,
        // relay path prefix prepended). Reverse that transform so the browser
        // gets a Location that resolves under this folder.
        if !upstream_response.status.is_redirection() {
            return Ok(());
        }
        let Some(loc) = upstream_response
            .headers
            .get("location")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
        else {
            return Ok(());
        };
        // Only rewrite root-relative paths ("/x"); leave "//host", full URLs,
        // and relative paths alone.
        if !loc.starts_with('/') || loc.starts_with("//") {
            return Ok(());
        }

        let relay_prefix = ctx.relay.as_ref().map(|r| r.path_prefix.as_str()).unwrap_or("");
        let rewritten = rewrite_redirect_location(&loc, relay_prefix, &ctx.mount_prefix);

        if rewritten != loc {
            let _ = upstream_response.insert_header("location", &rewritten);
        }
        Ok(())
    }

    /// Render a localized HTML page for fatal errors (e.g. a 502 when the relay
    /// or upstream can't be reached, or a 404 for an unmatched path) instead of
    /// Pingora's plain default error body.
    async fn fail_to_proxy(
        &self,
        session: &mut Session,
        e: &pingora::Error,
        _ctx: &mut Self::CTX,
    ) -> pingora::proxy::FailToProxy
    where
        Self::CTX: Send + Sync,
    {
        use pingora::{ErrorSource, ErrorType};
        let code: u16 = match e.etype() {
            ErrorType::HTTPStatus(c) => *c,
            _ => match e.esource() {
                ErrorSource::Upstream => 502,
                ErrorSource::Downstream => match e.etype() {
                    ErrorType::WriteError
                    | ErrorType::ReadError
                    | ErrorType::ConnectionClosed => 0, // downstream already gone
                    _ => 400,
                },
                ErrorSource::Internal | ErrorSource::Unset => 500,
            },
        };

        if code > 0 {
            let lang = accept_lang(session);
            let (title_key, body_key) = match code {
                404 => ("not-found-title", "not-found-body"),
                502 | 503 | 504 => ("unreachable-title", "unreachable-body"),
                _ => ("error-title", "error-body"),
            };
            let html = plan_ai_html::error_page(lang, title_key, body_key);
            if write_html(session, code, html).await.is_err() {
                let _ = session.respond_error(code).await;
            }
        }

        pingora::proxy::FailToProxy {
            error_code: code,
            can_reuse_downstream: false,
        }
    }
}

/// Extract path+query from a URI, handling both origin-form (`/path?q=1`)
/// and absolute-form (`https://host/path?q=1`) that Pingora may produce.
fn safe_path_and_query(uri: &http::Uri) -> String {
    if let Some(pq) = uri.path_and_query() {
        return pq.to_string();
    }
    // Absolute-form URI — path_and_query() returns None, reconstruct manually
    let path = uri.path();
    match uri.query() {
        Some(q) => format!("{path}?{q}"),
        None => path.to_string(),
    }
}

/// Pick the UI language from the request's Accept-Language header.
fn accept_lang(session: &Session) -> plan_ai_html::Lang {
    plan_ai_html::Lang::from_accept_language(
        session
            .req_header()
            .headers
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    )
}

/// Write a self-contained HTML response and finish the request.
async fn write_html(session: &mut Session, code: u16, html: String) -> Result<()> {
    let bytes = bytes::Bytes::from(html);
    let mut resp = pingora::http::ResponseHeader::build(code, None).map_err(|e| {
        pingora::Error::because(pingora::ErrorType::InternalError, "build html response", e)
    })?;
    let _ = resp.insert_header("Content-Type", "text/html; charset=utf-8");
    let _ = resp.insert_header("Content-Length", &bytes.len().to_string());
    session.write_response_header(Box::new(resp), false).await?;
    session.write_response_body(Some(bytes), true).await?;
    Ok(())
}

/// Extract the hostname from a request, handling both HTTP/1.1 Host header
/// and HTTP/2 :authority pseudo-header. Strips port if present.
fn extract_host(session: &Session) -> String {
    let raw = session
        .req_header()
        .headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .or_else(|| session.req_header().uri.host())
        .unwrap_or("");
    raw.split(':').next().unwrap_or(raw).to_lowercase()
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

/// First value for a cookie name in a `Cookie` request header.
fn extract_cookie(header: &str, name: &str) -> Option<String> {
    cookie_values(header, name).into_iter().next()
}

/// All values for a cookie name. A browser can send several same-named cookies
/// (e.g. one left over at a different Path); the gate must try each. Parsed with
/// the `cookie` crate at the header boundary (Pingora exposes plain header strs).
fn cookie_values(header: &str, name: &str) -> Vec<String> {
    cookie::Cookie::split_parse(header)
        .filter_map(Result::ok)
        .filter(|c| c.name() == name)
        .map(|c| c.value().to_string())
        .collect()
}

/// Parse gate params from a URI query string.
/// Returns (sig, org_id, expiry_ts) if all three params are present.
fn parse_gate_params(uri: &str) -> Option<(String, String, i64)> {
    let query = uri.split('?').nth(1)?;
    let mut sig = None;
    let mut oid = None;
    let mut exp = None;
    for pair in query.split('&') {
        if let Some(val) = pair.strip_prefix("__pg_token=") {
            sig = Some(val.to_string());
        } else if let Some(val) = pair.strip_prefix("__pg_oid=") {
            oid = Some(val.to_string());
        } else if let Some(val) = pair.strip_prefix("__pg_exp=") {
            exp = val.parse::<i64>().ok();
        }
    }
    Some((sig?, oid?, exp?))
}

/// Strip gate params from a URI, preserving other query params.
fn strip_gate_params(uri: &str) -> String {
    let (path, query) = match uri.split_once('?') {
        Some((p, q)) => (p, q),
        None => return uri.to_string(),
    };
    let remaining: Vec<&str> = query
        .split('&')
        .filter(|p| {
            !p.starts_with("__pg_token=")
                && !p.starts_with("__pg_oid=")
                && !p.starts_with("__pg_exp=")
        })
        .collect();
    if remaining.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", remaining.join("&"))
    }
}

/// Build the Pingora HTTP proxy service with BoringSSL TLS and dynamic SNI.
pub fn build_service(
    server_conf: &Arc<pingora::server::configuration::ServerConf>,
    cfg: &crate::config::ProxyConfig,
    routes: Arc<ArcSwap<HashMap<String, Vec<(String, Route, AuthMode)>>>>,
    cert_store: Arc<crate::cert_store::CertStore>,
) -> pingora::services::listening::Service<pingora::proxy::HttpProxy<WebAgencyProxy>> {
    let internal_token = cfg.internal_token().trim().to_string();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("failed to build internal API client");
    let proxy = WebAgencyProxy {
        routes,
        agency_domain: cfg.agency_domain.clone(),
        internal_token,
        server_url: cfg.server_url.trim_end_matches('/').to_string(),
        client,
    };

    let mut svc = http_proxy_service(server_conf, proxy);

    svc.add_tcp(&cfg.http_addr);

    let callback = crate::cert_store::CertStoreCallback(cert_store);
    let mut tls_settings = pingora::listeners::tls::TlsSettings::with_callbacks(Box::new(callback))
        .expect("failed to create TLS settings");
    tls_settings.enable_h2();
    svc.add_tls_with_settings(&cfg.https_addr, None, tls_settings);

    svc
}

#[cfg(test)]
mod tests {
    use super::rewrite_redirect_location;

    #[test]
    fn redirect_location_rewrite() {
        // Mount at /customer, no relay path prefix: re-add the mount.
        assert_eq!(rewrite_redirect_location("/chat/login", "", "/customer"), "/customer/chat/login");
        // Relay URL has a /v1 path prefix: strip it, then add the mount.
        assert_eq!(rewrite_redirect_location("/v1/chat/login", "/v1", "/customer"), "/customer/chat/login");
        // Relay prefix maps to root.
        assert_eq!(rewrite_redirect_location("/v1", "/v1", "/customer"), "/customer/");
        // Root mount: unchanged.
        assert_eq!(rewrite_redirect_location("/chat/login", "", "/"), "/chat/login");
        // Redirect outside the relay prefix: best-effort, just add the mount.
        assert_eq!(rewrite_redirect_location("/other", "/v1", "/customer"), "/customer/other");
    }
}
