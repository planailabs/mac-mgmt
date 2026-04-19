use serde::{Deserialize, Serialize};

/// Shared Sentry configuration — used by server, runner, and any other crate
/// that initialises Sentry from a TOML config file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SentryConfig {
    /// Sentry DSN. When unset, Sentry is disabled.
    #[serde(default)]
    pub dsn: Option<String>,
    /// Environment tag (e.g. "staging", "production").
    #[serde(default)]
    pub environment: Option<String>,
    /// Sample rate for traces [0.0, 1.0]. Default 0 (off).
    #[serde(default)]
    pub traces_sample_rate: f32,
}

/// Initialise Sentry from a [`SentryConfig`]. Returns `None` when no DSN is
/// configured (Sentry disabled). The returned guard must live for the lifetime
/// of the process.
pub fn init_sentry(cfg: &SentryConfig) -> Option<sentry::ClientInitGuard> {
    let dsn = cfg.dsn.as_deref()?;
    let guard = sentry::init((
        dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: cfg.environment.clone().map(Into::into),
            traces_sample_rate: cfg.traces_sample_rate,
            ..Default::default()
        },
    ));
    Some(guard)
}

// ── Sentry helper utilities ───────────────────────────────────────────

/// Add a breadcrumb with category, message, and optional key-value data.
pub fn breadcrumb(category: &str, message: &str, data: &[(&str, &str)]) {
    let mut map = sentry::protocol::Map::new();
    for (k, v) in data {
        map.insert(k.to_string(), serde_json::Value::String(v.to_string()));
    }
    sentry::add_breadcrumb(sentry::protocol::Breadcrumb {
        category: Some(category.to_string()),
        message: Some(message.to_string()),
        data: map,
        level: sentry::protocol::Level::Info,
        ..Default::default()
    });
}

/// Capture a command failure as a Sentry error event with context.
pub fn capture_cmd_failure(command: &str, exit_code: Option<i32>, stderr: &str) {
    sentry::with_scope(
        |scope| {
            scope.set_extra("command", serde_json::Value::String(command.to_string()));
            scope.set_extra(
                "exit_code",
                exit_code.map_or(serde_json::Value::Null, |c| {
                    serde_json::Value::Number(c.into())
                }),
            );
            if !stderr.is_empty() {
                scope.set_extra("stderr", serde_json::Value::String(stderr.to_string()));
            }
        },
        || {
            sentry::capture_message(&format!("command failed: {command}"), sentry::Level::Error);
        },
    );
}

/// Capture a general error with a message and extra context.
pub fn capture_error(message: &str, data: &[(&str, &str)]) {
    sentry::with_scope(
        |scope| {
            for (k, v) in data {
                scope.set_extra(k, serde_json::Value::String(v.to_string()));
            }
        },
        || {
            sentry::capture_message(message, sentry::Level::Error);
        },
    );
}

/// Set a tag on the current Sentry scope (persists until changed).
pub fn set_tag(key: &str, value: &str) {
    sentry::configure_scope(|scope| {
        scope.set_tag(key, value);
    });
}
