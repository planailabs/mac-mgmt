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
    Basic { credentials: Vec<(String, String)> }, // Vec<(username, password_hash)>
}

/// Per-request context — carries relay info from upstream_peer to upstream_request_filter.
#[derive(Default)]
pub struct RequestCtx {
    relay: Option<RelayCtx>,
}

struct RelayCtx {
    host: String,
    proxy_token: String,
    path_prefix: String,
}

/// The Pingora HTTP proxy that routes requests by Host header.
pub struct WebAgencyProxy {
    pub routes: Arc<ArcSwap<HashMap<String, (Route, AuthMode)>>>,
    pub agency_domain: String,
    pub internal_token: String,
}

impl WebAgencyProxy {
    async fn handle_basic_auth(
        &self,
        session: &mut Session,
        credentials: &[(String, String)],
    ) -> Result<bool> {
        let auth_header = session
            .req_header()
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        if let Some(ref auth) = auth_header {
            if let Some(encoded) = auth.strip_prefix("Basic ") {
                use base64::Engine;
                if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded) {
                    if let Ok(cred_str) = std::str::from_utf8(&decoded) {
                        if let Some((user, pass)) = cred_str.split_once(':') {
                            let hash = sha256_hex(pass);
                            for (expected_user, expected_hash) in credentials {
                                if user == expected_user && constant_time_eq(&hash, expected_hash) {
                                    return Ok(false); // auth passed
                                }
                            }
                        }
                    }
                }
            }
        }

        // Return 401
        let mut resp = pingora::http::ResponseHeader::build(401, None).map_err(|e| {
            pingora::Error::because(pingora::ErrorType::InternalError, "build 401 response", e)
        })?;
        let _ = resp.insert_header("WWW-Authenticate", "Basic realm=\"Protected\"");
        let _ = resp.insert_header("Content-Type", "text/plain");
        session.write_response_header(Box::new(resp), false).await?;
        session
            .write_response_body(Some(bytes::Bytes::from_static(b"Unauthorized")), true)
            .await?;
        Ok(true) // short-circuit
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
                        let _ = resp.insert_header(
                            "Set-Cookie",
                            &format!(
                                "__proxy_gate={cookie_value}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=86400"
                            ),
                        );
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
        let routes = self.routes.load();
        let (_, auth) = match routes.get(&host) {
            Some(r) => r,
            None => return Ok(false), // will 404 in upstream_peer
        };

        match auth {
            AuthMode::None => Ok(false),
            AuthMode::Basic { credentials } => self.handle_basic_auth(session, credentials).await,
            AuthMode::Oidc { org_id } => self.handle_oidc_auth(session, &host, org_id).await,
        }
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let host = extract_host(session);

        let routes = self.routes.load();
        match routes.get(&host) {
            Some((Route::Direct(upstream), _)) => Ok(Box::new(HttpPeer::new(
                upstream.as_str(),
                false,
                String::new(),
            ))),
            Some((
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
        if let Some(val) = part.strip_prefix(name) {
            if let Some(val) = val.strip_prefix('=') {
                return Some(val);
            }
        }
    }
    None
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
    routes: Arc<ArcSwap<HashMap<String, (Route, AuthMode)>>>,
    cert_store: Arc<crate::cert_store::CertStore>,
) -> pingora::services::listening::Service<pingora::proxy::HttpProxy<WebAgencyProxy>> {
    let internal_token = cfg.internal_token().trim().to_string();
    let proxy = WebAgencyProxy {
        routes,
        agency_domain: cfg.agency_domain.clone(),
        internal_token,
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
