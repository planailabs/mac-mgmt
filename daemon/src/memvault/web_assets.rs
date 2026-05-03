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

/// Axum fallback handler that serves embedded web assets.
///
/// Tries an exact file match first; for unknown paths returns `index.html`
/// so client-side (SPA) routing works.
pub async fn serve_embedded(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Try exact match, then fall back to index.html for SPA routing.
    match WebAssets::get(path) {
        Some(file) => serve_file(path, file),
        None => match WebAssets::get("index.html") {
            Some(file) => serve_file("index.html", file),
            None => (StatusCode::NOT_FOUND, "index.html not found in embedded assets").into_response(),
        },
    }
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
