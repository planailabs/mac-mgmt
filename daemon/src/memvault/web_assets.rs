//! Embedded memvault-web frontend assets, served via rust-embed.
//!
//! The `memvault-web-dist/` directory is populated by `build-memvault.sh`
//! before compiling the daemon.

use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "memvault-web-dist/"]
struct WebAssets;

/// Try to serve an embedded static asset by path.
///
/// Returns `Some(response)` for exact file matches (wasm, js, css, etc.).
/// Returns `None` for index.html and unknown paths — those are handled by
/// the Dioxus SSR handler which provides hydration data.
pub fn try_serve(path: &str) -> Option<axum::response::Response> {
    if path.is_empty() || path == "index.html" {
        return None;
    }
    WebAssets::get(path).map(|file| serve_file(path, file))
}

fn serve_file(path: &str, file: rust_embed::EmbeddedFile) -> axum::response::Response {
    let mime = mime_for(path);
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, cache_policy(path)),
        ],
        file.data,
    )
        .into_response()
}

fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

fn cache_policy(path: &str) -> &'static str {
    // WASM and JS bundles are content-hashed by dx; cache aggressively.
    // index.html must always be revalidated.
    match path.rsplit('.').next() {
        Some("wasm") | Some("js") => "public, max-age=31536000, immutable",
        Some("css") => "public, max-age=3600",
        _ => "no-cache",
    }
}
