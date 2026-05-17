//! Embedded memvault-web frontend assets, served via rust-embed.
//!
//! The `memvault-web-dist/` directory is populated by `build-memvault.sh`
//! before compiling the daemon.

use rust_embed::Embed;
use std::path::PathBuf;

#[derive(Embed)]
#[folder = "memvault-web-dist/"]
struct WebAssets;

/// Extract all embedded assets to a temp directory and set `DIOXUS_PUBLIC_PATH`
/// so dioxus's `serve_dioxus_application` serves them.
/// Must be called before building the fullstack router.
pub fn prepare_public_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("memvault-web-public");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create memvault public dir");

    for file_path in WebAssets::iter() {
        if let Some(file) = WebAssets::get(&file_path) {
            let dest = dir.join(file_path.as_ref());
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&dest, &file.data);
        }
    }

    // SAFETY: called before the router is built, single-threaded init.
    unsafe { std::env::set_var("DIOXUS_PUBLIC_PATH", &dir) };
    dir
}
