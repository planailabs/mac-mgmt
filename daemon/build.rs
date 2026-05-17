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
    println!("cargo::rerun-if-env-changed=DX_CLIENT_TIMEOUT");
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
/// When invoked via `dx build @client ... @server ...`, both targets build
/// concurrently. The client usually finishes first but we can't assume that.
/// This function blocks until the WASM output file appears (indicating the
/// client build is complete), then writes a generated source file with
/// `#[derive(Embed)] #[folder = "<absolute-path>"]` so the proc macro reads
/// assets directly from the dx output — no copy step needed.
///
/// For standalone `cargo build` (without dx), a pre-populated
/// `memvault-web-dist/` directory is used as fallback.
fn embed_dx_client_assets() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let generated = out_dir.join("web_assets_generated.rs");

    // ── Try to find dx client output ────────────────────────────────────
    let dx_dir = manifest_dir.join("../target/dx");

    let timeout_secs: u64 = std::env::var("DX_CLIENT_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let timeout = Duration::from_secs(timeout_secs);
    let start = Instant::now();

    // The JS loader is one of the last files dx writes for the client build.
    let sentinel_name = "wasm/mac-mgmt.js";

    // Scan target/dx/ for the sentinel file — handles any app name or profile.
    let find_dx_public = || -> Option<PathBuf> {
        let dx = std::fs::read_dir(&dx_dir).ok()?;
        for app_entry in dx.flatten() {
            let app_dir = app_entry.path();
            if !app_dir.is_dir() {
                continue;
            }
            let profiles = std::fs::read_dir(&app_dir).ok()?;
            for prof_entry in profiles.flatten() {
                let public = prof_entry.path().join("web/public");
                if public.join(sentinel_name).exists() {
                    return Some(public);
                }
            }
        }
        None
    };

    // Wait for the dx client output to appear (concurrent build).
    let dx_public = loop {
        if let Some(p) = find_dx_public() {
            break Some(p);
        }
        // If target/dx/ doesn't even exist, this isn't a dx build — fall through
        // immediately to the memvault-web-dist fallback.
        if !dx_dir.exists() {
            break None;
        }
        if start.elapsed() > timeout {
            eprintln!(
                "cargo:warning=build.rs: timed out after {}s waiting for dx client sentinel in {}",
                timeout.as_secs(),
                dx_dir.display(),
            );
            break None;
        }
        std::thread::sleep(Duration::from_millis(500));
    };

    // Resolve the folder path: dx output if found, otherwise memvault-web-dist/.
    let folder = if let Some(ref p) = dx_public {
        // Extra delay to ensure all files are flushed (snippets, wasm-opt).
        std::thread::sleep(Duration::from_secs(1));

        let abs = std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
        eprintln!("cargo:warning=build.rs: using dx client output at {}", abs.display());
        println!("cargo::rerun-if-changed={}", abs.display());

        // Compat symlinks so the web UI can reference the old "memvault-web" name.
        let wasm_dir = abs.join("wasm");
        let _ = std::os::unix::fs::symlink(wasm_dir.join("mac-mgmt.js"), wasm_dir.join("memvault-web.js"));
        let _ = std::os::unix::fs::symlink(wasm_dir.join("mac-mgmt_bg.wasm"), wasm_dir.join("memvault-web_bg.wasm"));

        abs
    } else {
        // Fallback: pre-populated memvault-web-dist/ (from a prior build or manual copy).
        let fallback = manifest_dir.join("memvault-web-dist");
        if !fallback.exists() {
            eprintln!(
                "cargo:warning=build.rs: no dx output found and memvault-web-dist/ does not exist"
            );
            // Write an empty generated file so compilation proceeds (the feature
            // may be gated at a higher level).
            std::fs::write(&generated, "// memvault-web assets not available\n").unwrap();
            println!("cargo::rerun-if-changed=memvault-web-dist");
            return;
        }
        eprintln!("cargo:warning=build.rs: using fallback memvault-web-dist/");
        println!("cargo::rerun-if-changed=memvault-web-dist");
        std::fs::canonicalize(&fallback).unwrap_or(fallback)
    };

    // Generate the rust-embed struct pointing at the resolved folder.
    let folder_str = folder.display().to_string().replace('\\', "/");
    let code = format!(
        r#"#[derive(::rust_embed::Embed)]
#[folder = "{folder_str}"]
struct WebAssets;
"#
    );
    std::fs::write(&generated, code).unwrap();
}
