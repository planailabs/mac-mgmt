use anyhow::{Context, Result};
use std::process::Command;

/// Run `openclaw health --json` and return true if healthy.
pub fn check() -> Result<bool> {
    let output = Command::new("openclaw")
        .args(["health", "--json"])
        .output()
        .context("failed to run openclaw health")?;

    if !output.status.success() {
        tracing::warn!(
            "openclaw health exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    tracing::debug!("openclaw health output: {stdout}");

    // Any non-zero exit or presence of issues in JSON indicates unhealthy
    Ok(true)
}

/// Check if openclaw has active sessions (is currently busy).
pub fn is_busy() -> Result<bool> {
    let output = Command::new("openclaw")
        .args(["sessions", "--active", "1", "--json"])
        .output()
        .context("failed to run openclaw sessions")?;

    if !output.status.success() {
        tracing::warn!(
            "openclaw sessions exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        // Assume busy if we can't check, to be safe
        return Ok(true);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).context("failed to parse sessions json")?;

    // If the JSON array/object has entries, openclaw is busy
    let busy = match &json {
        serde_json::Value::Array(arr) => !arr.is_empty(),
        serde_json::Value::Object(obj) => !obj.is_empty(),
        _ => false,
    };

    if busy {
        tracing::info!("openclaw is currently busy");
    } else {
        tracing::debug!("openclaw is idle");
    }

    Ok(busy)
}

/// Run `openclaw doctor --fix` to attempt auto-repair.
pub fn doctor_fix() -> Result<()> {
    tracing::info!("running openclaw doctor --fix");

    let output = Command::new("openclaw")
        .args(["doctor", "--fix"])
        .output()
        .context("failed to run openclaw doctor --fix")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        tracing::info!("openclaw doctor --fix completed successfully");
    } else {
        tracing::warn!(
            "openclaw doctor --fix exited with status {}: {}",
            output.status,
            stderr.trim()
        );
    }

    if !stdout.is_empty() {
        tracing::info!("doctor output: {}", stdout.trim());
    }

    Ok(())
}
