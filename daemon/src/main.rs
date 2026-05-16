// When built with --features web (by dx), this is the WASM client entry point.
// Both this and the daemon's SSR compile the same App component from the same
// crate, ensuring hydration works correctly.
#[cfg(feature = "web")]
fn main() {
    dioxus::launch(memvault_web::ui::app::App);
}

// Normal daemon entry point — delegated to a separate module to keep this
// file small and avoid gating every line with #[cfg(not(feature = "web"))].
#[cfg(not(feature = "web"))]
mod daemon_main;

#[cfg(not(feature = "web"))]
fn main() {
    daemon_main::main();
}
