fn main() {
    println!("cargo::rerun-if-env-changed=ENVIRONMENT");
    if std::env::var("ENVIRONMENT").is_err() {
        println!("cargo::rustc-env=ENVIRONMENT=dev");
    }

    // Expose the build target triple
    let target = std::env::var("TARGET").unwrap();
    println!("cargo::rustc-env=TARGET={target}");

    // Re-embed scripts if any file in the scripts directory changes
    println!("cargo::rerun-if-changed=scripts");
}
