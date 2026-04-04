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

        #[cfg(feature = "self-update")]
        {
            tracing::error!("daemon panicked, attempting self-update before exit");

            // Run self-update on a separate thread to avoid creating a nested Tokio
            // runtime (reqwest::blocking internally creates one, which panics if
            // dropped inside an existing async context).
            let handle = std::thread::spawn(|| crate::self_update::apply(false));
            match handle.join() {
                Ok(Ok(())) => tracing::info!("self-update after panic succeeded"),
                Ok(Err(e)) => tracing::error!("self-update after panic failed: {e}"),
                Err(_) => tracing::error!("self-update thread panicked"),
            }
        }

        // Run the default hook (prints backtrace etc.)
        default_hook(info);
    }));

    guard
}
