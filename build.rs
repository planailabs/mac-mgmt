fn main() {
    println!("cargo::rerun-if-env-changed=ENVIRONMENT");
    if std::env::var("ENVIRONMENT").is_err() {
        println!("cargo::rustc-env=ENVIRONMENT=dev");
    }
}
