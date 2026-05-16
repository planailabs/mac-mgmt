// When compiled for wasm32 (by dx @client), this is the WASM client entry point.
// Both this and the daemon's SSR compile the same App component from the same
// crate, ensuring hydration works correctly.
#[cfg(target_arch = "wasm32")]
fn main() {
    dioxus::launch(memvault_web::ui::app::App);
}

// Normal daemon entry point — delegated to a separate module to keep this
// file small and avoid gating every line.
#[cfg(not(target_arch = "wasm32"))]
mod daemon_main;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    daemon_main::main();
}
