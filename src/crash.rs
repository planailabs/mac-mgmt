use std::panic;

const SENTRY_DSN: &str = "https://5e5956eb9a8c1904c961efadc1e72444@o4511110586761216.ingest.de.sentry.io/4511110623133776"; // TODO: set your Sentry DSN

/// Initialize Sentry and install a panic hook that reports to Sentry
/// and attempts a self-update before aborting.
pub fn init() -> sentry::ClientInitGuard {
    let guard = sentry::init((
        SENTRY_DSN,
        sentry::ClientOptions {
            release: Some(env!("CARGO_PKG_VERSION").into()),
            ..Default::default()
        },
    ));

    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |info| {
        // Report to Sentry
        sentry::integrations::panic::panic_handler(info);
        sentry::Hub::current().client().map(|c| c.flush(Some(std::time::Duration::from_secs(5))));

        tracing::error!("daemon panicked, attempting self-update before exit");

        // Try to self-update so the next launch gets a (hopefully fixed) binary
        if let Err(e) = crate::daemon::do_update(false) {
            tracing::error!("self-update after panic failed: {e}");
        } else {
            tracing::info!("self-update after panic succeeded");
        }

        // Run the default hook (prints backtrace etc.)
        default_hook(info);
    }));

    guard
}
