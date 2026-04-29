//! Shared HTTP proxy helpers for building requests to local tunnel targets.
//!
//! Extracted from relay_client.rs for reuse across WS and libp2p transports.

use std::borrow::Cow;

/// A tunnel definition mapping a name to a local host:port.
#[derive(Debug, Clone)]
pub struct TunnelTarget {
    pub host: String,
    pub port: u16,
}

/// Headers stripped when `fake_origin_local` is true to prevent
/// local services from rejecting requests with non-local origins.
const FAKE_ORIGIN_DROP_HEADERS: &[&str] = &[
    "host",
    "origin",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-proto",
    "x-forwarded-port",
    "x-real-ip",
    "forwarded",
    "via",
];

/// Rewrite the origin (scheme + host + port) of a URL while preserving the
/// path, query, and fragment.
fn rewrite_url_origin(value: &str, target: &TunnelTarget) -> Option<String> {
    let mut parsed = reqwest::Url::parse(value).ok()?;
    let _ = parsed.set_host(Some(&target.host));
    let _ = parsed.set_port(Some(target.port));
    let _ = parsed.set_scheme("http");
    Some(parsed.to_string())
}

/// Build a reqwest request for the given HTTP method against a tunnel target.
pub fn build_proxy_request(
    client: &reqwest::Client,
    target: &TunnelTarget,
    method: &str,
    path: &str,
    fake_origin_local: bool,
) -> reqwest::RequestBuilder {
    let url = format!("http://{}:{}{path}", target.host, target.port);
    let mut req = match method {
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        "HEAD" => client.head(&url),
        _ => client.get(&url),
    };
    if fake_origin_local {
        req = req.header("host", format!("{}:{}", target.host, target.port));
    }
    req
}

/// Rewrite or drop a single header value when `fake_origin_local` is true.
fn fake_origin_header<'a>(
    key: &str,
    value: &'a str,
    target: &TunnelTarget,
) -> Option<Cow<'a, str>> {
    if key == "referer" {
        return Some(
            rewrite_url_origin(value, target)
                .map(Cow::Owned)
                .unwrap_or(Cow::Borrowed(value)),
        );
    }
    None
}

/// Apply request headers from a Vec, filtering hop-by-hop headers.
pub fn apply_headers_vec(
    mut req: reqwest::RequestBuilder,
    headers: &[(String, String)],
    fake_origin_local: bool,
    target: &TunnelTarget,
) -> reqwest::RequestBuilder {
    for (k, v) in headers {
        let lk = k.to_lowercase();
        if lk == "connection" || lk == "transfer-encoding" {
            continue;
        }
        if fake_origin_local && FAKE_ORIGIN_DROP_HEADERS.contains(&lk.as_str()) {
            if let Some(rewritten) = fake_origin_header(&lk, v, target) {
                req = req.header(k.as_str(), rewritten.as_ref());
            }
            continue;
        }
        req = req.header(k.as_str(), v.as_str());
    }
    req
}

/// Apply request headers from a JSON object, filtering hop-by-hop headers.
pub fn apply_headers_json(
    mut req: reqwest::RequestBuilder,
    headers: &serde_json::Value,
    fake_origin_local: bool,
    target: &TunnelTarget,
) -> reqwest::RequestBuilder {
    if let Some(hdrs) = headers.as_object() {
        for (k, v) in hdrs {
            let lk = k.to_lowercase();
            if lk == "connection" || lk == "transfer-encoding" {
                continue;
            }
            if fake_origin_local && FAKE_ORIGIN_DROP_HEADERS.contains(&lk.as_str()) {
                if let Some(val) = v.as_str() {
                    if let Some(rewritten) = fake_origin_header(&lk, val, target) {
                        req = req.header(k.as_str(), rewritten.as_ref());
                    }
                }
                continue;
            }
            if let Some(val) = v.as_str() {
                req = req.header(k.as_str(), val);
            }
        }
    }
    req
}

/// Apply fake_origin_local header rewrites to a WebSocket upgrade request.
/// Strips origin-revealing headers and sets Host to the local target.
pub fn apply_fake_origin_ws(
    headers: &mut reqwest::header::HeaderMap,
    target: &TunnelTarget,
) {
    // Set Host to local target.
    let host_val = format!("{}:{}", target.host, target.port);
    if let Ok(v) = host_val.parse() {
        headers.insert("Host", v);
    }
    // Strip origin-revealing headers.
    for &name in FAKE_ORIGIN_DROP_HEADERS {
        if name != "host" {
            headers.remove(name);
        }
    }
}

/// Decode a base64-encoded body and attach it to the request.
pub fn apply_body_b64(
    req: reqwest::RequestBuilder,
    body_b64: Option<String>,
) -> reqwest::RequestBuilder {
    if let Some(b64) = body_b64 {
        use base64::Engine;
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&b64) {
            return req.body(bytes);
        }
    }
    req
}
