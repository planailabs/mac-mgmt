/// Initialise a basic `tracing_subscriber` with env-filter support.
///
/// Uses `RUST_LOG` if set, otherwise falls back to `default_filter`
/// (e.g. `"info"` or `"info,my_crate=debug"`).
pub fn init_tracing(default_filter: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
        )
        .init();
}

/// Initialise tracing with Sentry integration (tracing layer + fmt layer).
///
/// This mirrors what the server uses: events are printed to stdout *and*
/// forwarded to Sentry as breadcrumbs/errors.
#[cfg(feature = "sentry")]
pub fn init_tracing_with_sentry(default_filter: &str) {
    use tracing_subscriber::prelude::*;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(sentry::integrations::tracing::layer())
        .try_init();
}
