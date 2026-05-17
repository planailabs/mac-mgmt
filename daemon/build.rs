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
/// Requires `DX_WEB_PUBLIC` to be set (by patched dioxus-cli or the build script).
/// Waits in a loop for the sentinel file since the client build may still be running.
fn embed_dx_client_assets() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let generated = out_dir.join("web_assets_generated.rs");

    let Ok(dir) = std::env::var("DX_WEB_PUBLIC") else {
        // Not a dx build — generate an empty asset struct so the code compiles.
        // At runtime prepare_public_dir() will produce an empty directory.
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let empty = manifest_dir.join("src/memvault"); // exists, but has no web assets
        let empty_str = empty.display().to_string().replace('\\', "/");
        std::fs::write(
            &generated,
            format!(
                "#[derive(::rust_embed::Embed)]\n#[folder = \"{empty_str}\"]\n#[include = \"__nonexistent__\"]\nstruct WebAssets;\n"
            ),
        )
        .unwrap();
        return;
    };

    let p = PathBuf::from(&dir);
    // In release builds dx bundles WASM into assets/ and deletes wasm/.
    // Use index.html as sentinel — it's always present after a successful build.
    let sentinel = p.join("index.html");

    let timeout_secs: u64 = std::env::var("DX_CLIENT_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600);
    let timeout = Duration::from_secs(timeout_secs);
    let start = Instant::now();

    while !sentinel.exists() {
        if start.elapsed() > timeout {
            panic!(
                "build.rs: DX_WEB_PUBLIC={dir} — timed out after {}s waiting for {}",
                timeout.as_secs(),
                sentinel.display(),
            );
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    write_generated(&generated, &p);
}

fn write_generated(path: &PathBuf, folder: &PathBuf) {
    let abs = std::fs::canonicalize(folder).unwrap_or_else(|_| folder.clone());
    eprintln!("cargo:warning=build.rs: embedding assets from {}", abs.display());
    println!("cargo::rerun-if-changed={}", abs.display());

    let folder_str = abs.display().to_string().replace('\\', "/");
    let code = format!(
        r#"#[derive(::rust_embed::Embed)]
#[folder = "{folder_str}"]
struct WebAssets;
"#
    );
    std::fs::write(path, code).unwrap();
}
