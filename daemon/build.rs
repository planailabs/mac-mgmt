use std::path::PathBuf;
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
    println!("cargo::rerun-if-env-changed=DX_WEB_PUBLIC");
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

/// Generate a `WebAssets` struct via rust-embed pointing at the dx client output.
///
/// The folder is resolved in order:
/// 1. `DX_WEB_PUBLIC` env var — set by patched dioxus-cli before starting
///    builds, so the path is known even in concurrent @client/@server mode.
/// 2. Auto-detect `target/dx/mac-mgmt/{release,debug}/web/public` for local
///    dev with unpatched dx.
/// 3. Fallback to `memvault-web-dist/` for standalone `cargo build`.
fn embed_dx_client_assets() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let generated = out_dir.join("web_assets_generated.rs");

    let sentinel = "wasm/mac-mgmt.js";

    // ── 1. DX_WEB_PUBLIC env var (set by patched dx, or manually) ───────
    // In a concurrent @client/@server build, dx sets this to the client's
    // root_dir() before invoking cargo. The client build may still be running,
    // so we wait for the sentinel file.
    if let Ok(dir) = std::env::var("DX_WEB_PUBLIC") {
        let p = PathBuf::from(&dir);
        let timeout_secs: u64 = std::env::var("DX_CLIENT_TIMEOUT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);
        let timeout = Duration::from_secs(timeout_secs);
        let start = Instant::now();

        while !p.join(sentinel).exists() {
            if start.elapsed() > timeout {
                eprintln!(
                    "cargo:warning=build.rs: DX_WEB_PUBLIC={dir} — timed out after {}s waiting for sentinel",
                    timeout.as_secs()
                );
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        if p.join(sentinel).exists() {
            write_generated(&generated, &p);
            return;
        }
    }

    // ── 2. Auto-detect from target/dx/ (local dev without patched dx) ──
    let dx_base = manifest_dir.join("../target/dx/mac-mgmt");
    for profile in ["release", "debug"] {
        let c = dx_base.join(profile).join("web/public");
        if c.join(sentinel).exists() {
            write_generated(&generated, &c);
            return;
        }
    }

    // ── 3. Fallback: memvault-web-dist/ ─────────────────────────────────
    let fallback = manifest_dir.join("memvault-web-dist");
    if fallback.join(sentinel).exists() {
        eprintln!("cargo:warning=build.rs: using fallback memvault-web-dist/");
        println!("cargo::rerun-if-changed=memvault-web-dist");
        write_generated(&generated, &fallback);
        return;
    }

    eprintln!("cargo:warning=build.rs: no memvault-web assets found anywhere");
    std::fs::write(&generated, "// memvault-web assets not available\n").unwrap();
    println!("cargo::rerun-if-changed=memvault-web-dist");
}

fn write_generated(path: &PathBuf, folder: &PathBuf) {
    let abs = std::fs::canonicalize(folder).unwrap_or_else(|_| folder.clone());
    eprintln!("cargo:warning=build.rs: embedding assets from {}", abs.display());
    println!("cargo::rerun-if-changed={}", abs.display());

    // Compat symlinks so the web UI can reference the old "memvault-web" name.
    let wasm_dir = abs.join("wasm");
    let _ = std::os::unix::fs::symlink(wasm_dir.join("mac-mgmt.js"), wasm_dir.join("memvault-web.js"));
    let _ = std::os::unix::fs::symlink(
        wasm_dir.join("mac-mgmt_bg.wasm"),
        wasm_dir.join("memvault-web_bg.wasm"),
    );

    // Extra delay to ensure all files are flushed (snippets, wasm-opt).
    std::thread::sleep(Duration::from_secs(1));

    let folder_str = abs.display().to_string().replace('\\', "/");
    let code = format!(
        r#"#[derive(::rust_embed::Embed)]
#[folder = "{folder_str}"]
struct WebAssets;
"#
    );
    std::fs::write(path, code).unwrap();
}
