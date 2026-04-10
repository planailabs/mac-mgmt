use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // Embed build timestamp for CSS cache busting.
    // The timestamp changes on every build, forcing browsers to fetch fresh CSS.
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    println!("cargo:rustc-env=BUILD_TIMESTAMP={ts}");
}
