use sentry::protocol::{Breadcrumb, Map};

/// Add a breadcrumb with category, message, and optional key-value data.
pub fn breadcrumb(category: &str, message: &str, data: &[(&str, &str)]) {
    let mut map = Map::new();
    for (k, v) in data {
        map.insert(k.to_string(), serde_json::Value::String(v.to_string()));
    }
    sentry::add_breadcrumb(Breadcrumb {
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
