fn main() {
    // Match the daemon's GIT_SHA stamp (see daemon/build.rs). Prefer a
    // GIT_SHA env from CI / nix; fall back to `git rev-parse HEAD` with
    // a `-dirty` suffix when the tree has uncommitted changes.
    let sha = std::env::var("GIT_SHA")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            let output = std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let mut sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if sha.is_empty() {
                return None;
            }
            let dirty = std::process::Command::new("git")
                .args(["status", "--porcelain"])
                .output()
                .map(|o| !o.stdout.is_empty())
                .unwrap_or(false);
            if dirty {
                sha.push_str("-dirty");
            }
            Some(sha)
        })
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo::rustc-env=GIT_SHA={sha}");
    println!("cargo::rerun-if-changed=../.git/HEAD");
    println!("cargo::rerun-if-changed=../.git/index");
}
