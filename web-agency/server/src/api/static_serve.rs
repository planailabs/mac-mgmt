//! Webspace static router.
//!
//! The proxy proxies static-folder requests to the agency server (passing the
//! original Host through) and injects two headers:
//!   * `x-web-agency-webspace`        — the webspace (folder) id to serve
//!   * `x-web-agency-internal-token`  — the shared internal token (trust gate)
//!
//! This is a standalone router, kept fully separate from the agency app router
//! (no OIDC/Dioxus middleware). `main` steers requests carrying the webspace
//! header here; everything else goes to the agency router. Files are served
//! from the folder's webroot via `tower_http::services::ServeDir` (which handles
//! MIME types, index.html, range/conditional requests, and traversal safety).

use dioxus::fullstack::axum::{
    Router,
    body::Body,
    extract::Request,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use tower::ServiceExt; // oneshot
use tower_http::services::ServeDir;

pub const WEBSPACE_HEADER: &str = "x-web-agency-webspace";
const TOKEN_HEADER: &str = "x-web-agency-internal-token";

/// Router for proxy-forwarded static webspace requests. Every request is served
/// from the requested webspace's webroot (the proxy guarantees the header).
pub fn router() -> Router {
    Router::new().fallback(serve)
}

async fn serve(req: Request) -> Response {
    let Some(webspace_id) = req
        .headers()
        .get(WEBSPACE_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
    else {
        return (StatusCode::BAD_REQUEST, "missing webspace header").into_response();
    };

    if !internal_token_ok(req.headers()) {
        return (StatusCode::FORBIDDEN, "invalid internal token").into_response();
    }

    let dir = crate::local_hosting::webspace_dir(webspace_id);
    // ServeDir resolves the request path against `dir`, serves index.html for
    // directories, guesses the MIME type, and rejects `..` traversal. Its error
    // type is Infallible, so oneshot always succeeds.
    let resp = match ServeDir::new(&dir).oneshot(req).await {
        Ok(resp) => resp,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    // On a miss, serve the webspace's own 404.html (with a 404 status) if it has one.
    if resp.status() == StatusCode::NOT_FOUND {
        if let Ok(body) = tokio::fs::read(dir.join("404.html")).await {
            return (
                StatusCode::NOT_FOUND,
                [("content-type", "text/html; charset=utf-8")],
                body,
            )
                .into_response();
        }
    }

    resp.map(Body::new)
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
