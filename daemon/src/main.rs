// When compiled for wasm32 (by dx @client), this is the WASM client entry point.
// Both this and the daemon's SSR compile the same App component from the same
// crate, ensuring hydration works correctly.
#[cfg(target_arch = "wasm32")]
fn main() {
    memvault_web::launch_client();
}

// Normal daemon entry point — delegated to a separate module to keep this
// file small and avoid gating every line.
#[cfg(not(target_arch = "wasm32"))]
mod daemon_main;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let argv0 = std::env::args().next().unwrap_or_default();
    let basename = std::path::Path::new(&argv0)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    if basename == "systemctl" {
        daemon_main::systemctl_main();
    } else {
        daemon_main::main();
    }
}
