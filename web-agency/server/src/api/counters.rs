//! Global atomic counters for Prometheus metrics.
//!
//! Per-cycle gauges are reset at the start of each cycle (sync, cert tick).
//! Cumulative counters are monotonically increasing and never reset.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use uuid::Uuid;

/// Global counters, accessed from instrumented code paths and read by the metrics endpoint.
pub static COUNTERS: LazyLock<GlobalCounters> = LazyLock::new(GlobalCounters::new);

/// Per-webspace relay token mint results, keyed by hostname.
pub static RELAY_MINT_RESULTS: LazyLock<Mutex<HashMap<String, MintResult>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Result of the last relay token mint attempt for a webspace hostname.
pub struct MintResult {
    pub org_id: Uuid,
    pub success: bool,
}

pub struct GlobalCounters {
    // ── Per-cycle gauges (reset at start of each cycle) ──
    pub sync_dns_errors: AtomicU64,
    pub sync_expiry_errors: AtomicU64,
    pub sync_ns_errors: AtomicU64,
    pub sync_bot_errors: AtomicU64,
    pub cert_renewal_success: AtomicU64,
    pub cert_renewal_failed: AtomicU64,
    pub cert_issuance_success: AtomicU64,
    pub cert_issuance_failed: AtomicU64,

    // ── Cumulative counters (never reset) ──
    pub deploy_success: AtomicU64,
    pub deploy_failed: AtomicU64,
    pub relay_mint_success: AtomicU64,
    pub relay_mint_failed: AtomicU64,
}

impl GlobalCounters {
    fn new() -> Self {
        Self {
            sync_dns_errors: AtomicU64::new(0),
            sync_expiry_errors: AtomicU64::new(0),
            sync_ns_errors: AtomicU64::new(0),
            sync_bot_errors: AtomicU64::new(0),
            cert_renewal_success: AtomicU64::new(0),
            cert_renewal_failed: AtomicU64::new(0),
            cert_issuance_success: AtomicU64::new(0),
            cert_issuance_failed: AtomicU64::new(0),
            deploy_success: AtomicU64::new(0),
            deploy_failed: AtomicU64::new(0),
            relay_mint_success: AtomicU64::new(0),
            relay_mint_failed: AtomicU64::new(0),
        }
    }

    /// Reset all per-cycle sync gauges to 0.
    pub fn reset_sync_gauges(&self) {
        self.sync_dns_errors.store(0, Ordering::Relaxed);
        self.sync_expiry_errors.store(0, Ordering::Relaxed);
        self.sync_ns_errors.store(0, Ordering::Relaxed);
        self.sync_bot_errors.store(0, Ordering::Relaxed);
    }

    /// Reset all per-cycle cert gauges to 0.
    pub fn reset_cert_gauges(&self) {
        self.cert_renewal_success.store(0, Ordering::Relaxed);
        self.cert_renewal_failed.store(0, Ordering::Relaxed);
        self.cert_issuance_success.store(0, Ordering::Relaxed);
        self.cert_issuance_failed.store(0, Ordering::Relaxed);
    }
}
