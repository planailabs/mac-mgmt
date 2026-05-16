use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn main() {
    // ── Embed memvault-web assets from dx client output ────────────────
    // Only relevant for the native server build — skip entirely for wasm32
    // (which IS the client producing those assets).
    let target = std::env::var("TARGET").unwrap();
    if !target.contains("wasm32") {
        embed_dx_client_assets();
    }

    println!("cargo::rerun-if-env-changed=ENVIRONMENT");
    if std::env::var("ENVIRONMENT").is_err() {
        println!("cargo::rustc-env=ENVIRONMENT=dev");
    }

    // Expose the build target triple
    println!("cargo::rustc-env=TARGET={target}");

    // Git commit the binary was built from. Prefer GIT_SHA from the
    // environment (CI / nix), fall back to `git rev-parse HEAD` when
    // building from a checkout. "unknown" when neither works (e.g.
    // building from a source tarball). "-dirty" suffix when the tree
    // has uncommitted changes.
    let sha = std::env::var("GIT_SHA")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().trim_end_matches("-dirty").to_string())
        .or_else(|| {
            let output = std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if sha.is_empty() {
                return None;
            }
            Some(sha)
        })
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo::rustc-env=GIT_SHA={sha}");
    println!("cargo::rerun-if-changed=../.git/HEAD");
    println!("cargo::rerun-if-changed=../.git/index");

    // Re-embed scripts if any file in the scripts directory changes
    println!("cargo::rerun-if-changed=scripts");
}

/// Copy dx client output to memvault-web-dist/ for rust-embed.
///
/// When invoked via `dx build @client ... @server ...`, both targets build
/// concurrently. The client usually finishes first but we can't assume that.
/// This function blocks until the WASM output file appears (indicating the
/// client build is complete), then copies everything to the embed directory.
///
/// For standalone `cargo build` (without dx), the directory must already be
/// populated by a prior build — this function is a no-op if the dx output
/// doesn't exist within the timeout.
fn embed_dx_client_assets() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let dist_dir = manifest_dir.join("memvault-web-dist");

    // Determine dx output path based on build profile.
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let dx_public = manifest_dir
        .join("../target/dx/mac-mgmt")
        .join(&profile)
        .join("web/public");

    // The JS loader is one of the last files dx writes for the client build.
    // Its presence means the client output is complete.
    let wasm_sentinel = dx_public.join("wasm/mac-mgmt.js");

    // Check if we're in a dx build by looking for the dx output directory.
    // If the directory doesn't exist at all, this isn't a dx build.
    if !dx_public.exists() {
        println!("cargo::rerun-if-changed=memvault-web-dist");
        return;
    }

    // Wait for the client WASM output to appear (dx may still be building it).
    let timeout = Duration::from_secs(300); // 5 minutes max
    let start = Instant::now();
    while !wasm_sentinel.exists() {
        if start.elapsed() > timeout {
            eprintln!(
                "cargo:warning=Timed out waiting for dx client output at {}",
                wasm_sentinel.display()
            );
            println!("cargo::rerun-if-changed=memvault-web-dist");
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // Extra delay to ensure all files are flushed (snippets, wasm-opt)
    std::thread::sleep(Duration::from_secs(1));

    // Remove stale dist and copy fresh client output
    let _ = std::fs::remove_dir_all(&dist_dir);
    copy_dir_recursive(&dx_public, &dist_dir);

    // Compat aliases: the web UI may reference the old package name "memvault-web"
    let wasm_dir = dist_dir.join("wasm");
    let _ = std::os::unix::fs::symlink(wasm_dir.join("mac-mgmt.js"), wasm_dir.join("memvault-web.js"));
    let _ = std::os::unix::fs::symlink(wasm_dir.join("mac-mgmt_bg.wasm"), wasm_dir.join("memvault-web_bg.wasm"));

    // Rerun when the dx client output changes
    println!("cargo::rerun-if-changed={}", dx_public.display());
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            std::fs::copy(&src_path, &dst_path).unwrap();
        }
    }
}
