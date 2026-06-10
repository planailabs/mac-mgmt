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
    let args: Vec<String> = std::env::args().collect();
    let argv0 = args.first().cloned().unwrap_or_default();
    let basename = std::path::Path::new(&argv0)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    if basename == "systemctl" {
        daemon_main::systemctl_main();
    }

    // USB-run mode: when built with `--features usb`, running with no
    // subcommand (the default-command flip) or an explicit `usb` subcommand
    // boots the sovereign-AI stack. This is intercepted *before* the tokio
    // runtime so the desktop event loop can own the main thread (a hard
    // requirement on macOS). `usb-prefetch` and all other subcommands fall
    // through to the normal async entry below.
    #[cfg(feature = "usb")]
    {
        let first = args.get(1).map(String::as_str);
        let usb_run = first.is_none() || first == Some("usb");
        if usb_run {
            mac_mgmt_daemon::usb::main(args);
        }
    }

    // USB-daemon mode: `mac-mgmt usbd …` boots the reduced phone-home control
    // plane for the plan-ai-usb-minimal stack. Intercepted before the tokio
    // runtime so HOME can be pinned single-threaded (and, on macOS, so any
    // future event loop can own the main thread), matching `usb` above.
    #[cfg(feature = "usbd")]
    {
        if args.get(1).map(String::as_str) == Some("usbd") {
            mac_mgmt_daemon::usb_daemon::main(args);
        }
    }

    daemon_main::main();
}
