//! Eventual-consistency assertion helpers, wired to the Antithesis SDK when the
//! `antithesis` feature is on.
//!
//! The SDK's `assert_always!`/`assert_sometimes!` macros require a *literal*
//! message (they build a static assertion catalog), so the dynamic `what`
//! description is carried in the JSON details and every call site here uses a
//! fixed literal. Locally (feature off) these still do the real poll and
//! return an error on timeout, which is what fails the emulator round.

use std::future::Future;
use std::time::Duration;

use serde_json::json;

/// Initialise the SDK. Defaults `ANTITHESIS_SDK_LOCAL_OUTPUT` to `local_output`
/// (if given and the env var is unset) so assertion/event JSONL lands there
/// during local runs.
pub fn init(local_output: Option<&std::path::Path>) {
    if let Some(path) = local_output {
        if std::env::var_os("ANTITHESIS_SDK_LOCAL_OUTPUT").is_none() {
            // SAFETY: called once at startup before other threads read env.
            unsafe {
                std::env::set_var("ANTITHESIS_SDK_LOCAL_OUTPUT", path);
            }
        }
    }
    #[cfg(feature = "antithesis")]
    antithesis_sdk::antithesis_init();
}

/// Signal that cluster/node setup is finished and fault injection may begin.
pub fn setup_complete(details: &serde_json::Value) {
    tracing::info!(?details, "setup complete");
    #[cfg(feature = "antithesis")]
    antithesis_sdk::lifecycle::setup_complete(details);
}

/// Mark that a code location was reached at least once.
pub fn reached(what: &str) {
    tracing::debug!(what, "reachable");
    #[cfg(feature = "antithesis")]
    antithesis_sdk::assert_reachable!("workload reachable", &json!({ "what": what }));
}

/// Record an always-property outcome (the property must always hold).
fn always(cond: bool, what: &str, details: &serde_json::Value) {
    #[cfg(feature = "antithesis")]
    antithesis_sdk::assert_always!(cond, "workload eventual-consistency property", &json!({
        "what": what,
        "details": details,
    }));
    #[cfg(not(feature = "antithesis"))]
    {
        let _ = (cond, what, details);
    }
}

/// Record a sometimes-property outcome (the property must hold at least once
/// across the whole test — good for "this state is reachable").
fn sometimes(cond: bool, what: &str, details: &serde_json::Value) {
    #[cfg(feature = "antithesis")]
    antithesis_sdk::assert_sometimes!(cond, "workload sometimes property", &json!({
        "what": what,
        "details": details,
    }));
    #[cfg(not(feature = "antithesis"))]
    {
        let _ = (cond, what, details);
    }
}

/// Poll `check` until it returns `Ok(())` or `timeout` elapses.
///
/// On resolution emits an SDK always+sometimes assertion describing whether the
/// property held, and returns `Err` with the last failure on timeout so the
/// caller (emulator/CLI) fails the round.
pub async fn eventually<F, Fut>(
    what: &str,
    timeout: Duration,
    interval: Duration,
    mut check: F,
) -> anyhow::Result<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match check().await {
            Ok(()) => {
                always(true, what, &json!({ "resolved": true }));
                sometimes(true, what, &json!({ "resolved": true }));
                tracing::info!(what, "EC property holds");
                return Ok(());
            }
            Err(e) => {
                if tokio::time::Instant::now() >= deadline {
                    let msg = e.to_string();
                    always(false, what, &json!({ "resolved": false, "error": msg }));
                    return Err(anyhow::anyhow!("EC property '{what}' not reached: {msg}"));
                }
                tracing::debug!(what, error = %e, "EC not yet satisfied");
            }
        }
        tokio::time::sleep(interval).await;
    }
}
