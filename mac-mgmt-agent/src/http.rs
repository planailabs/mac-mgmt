//! The process-wide HTTP client for management-server calls.
//!
//! Building a `reqwest::Client` is not cheap: it constructs a fresh TLS
//! configuration (parsing the whole CA bundle) and a new connection pool. Doing
//! that per call turns repetitive traffic — heartbeats every few hundred
//! milliseconds, one sync per server push — into a certificate-parsing loop.
//! sim-tests' `hammer_many_daemons` showed the effect directly: ten daemons in
//! one process, a push storm, and PEM/base64 decoding of the root store as the
//! top CPU consumer, which starved the very tasks that were supposed to send
//! heartbeats.
//!
//! Share one client instead: one TLS config per process and reused
//! connections. Callers keep their own per-request timeouts.

use std::sync::OnceLock;

/// The shared client. Initialised on first use and reused for the process'
/// lifetime, so it also survives a config reload or a server URL change — the
/// pool is keyed by origin, so pointing at a new server just opens new
/// connections.
pub fn shared_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}
