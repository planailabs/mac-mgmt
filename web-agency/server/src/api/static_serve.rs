//! Serve static webspace folders for the reverse proxy.
//!
//! The proxy proxies static-folder requests to the agency server (passing the
//! original Host through) and injects two headers:
//!   * `x-web-agency-webspace`        — the webspace (folder) id to serve
//!   * `x-web-agency-internal-token`  — the shared internal token (trust gate)
//!
//! This middleware intercepts those requests and serves files from the folder's
//! webroot directory via `tower_http::services::ServeDir` (which handles MIME
//! types, index.html, range/conditional requests, and path-traversal safety).
//! All other requests fall through to the rest of the app.

use dioxus::fullstack::axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tower::ServiceExt; // oneshot
use tower_http::services::ServeDir;

const WEBSPACE_HEADER: &str = "x-web-agency-webspace";
const TOKEN_HEADER: &str = "x-web-agency-internal-token";

pub async fn serve(req: Request, next: Next) -> Response {
    let Some(webspace_id) = req
        .headers()
        .get(WEBSPACE_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
    else {
        return next.run(req).await; // not a static-folder request
    };

    if !internal_token_ok(req.headers()) {
        return (StatusCode::FORBIDDEN, "invalid internal token").into_response();
    }

    let dir = crate::local_hosting::webspace_dir(webspace_id);
    // ServeDir resolves the request path against `dir`, serves index.html for
    // directories, guesses the MIME type, and rejects `..` traversal. Its error
    // type is Infallible, so oneshot always succeeds.
    match ServeDir::new(dir).oneshot(req).await {
        Ok(resp) => resp.map(Body::new),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Validate the injected internal token against the token file the proxy uses.
fn internal_token_ok(headers: &HeaderMap) -> bool {
    let Some(proxy_cfg) = crate::config::config().proxy.as_ref() else {
        return false;
    };
    let Ok(expected) = std::fs::read_to_string(&proxy_cfg.internal_token_path) else {
        return false;
    };
    let provided = headers
        .get(TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    !provided.is_empty() && provided == expected.trim()
}
