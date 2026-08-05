//! The daemon's OpenTelemetry surface.
//!
//! Everything an instrumented path reports lands in [`State`], a single
//! in-memory snapshot; the instruments registered by [`register`] are
//! *observable*, so their callbacks read that snapshot whenever a reader
//! collects — the OTLP push reader and the `/metrics` scrape alike. Two
//! consequences worth knowing:
//!
//! - Replacing a snapshot replaces its label set, so a mount that disappears
//!   or a service that is removed stops being reported, without the explicit
//!   `reset()` calls a Prometheus gauge-vec needed.
//! - A scrape and an export always see the same values, because they read the
//!   same state rather than two separately-updated registries.
//!
//! Everything is a gauge unless it is genuinely monotonic: probes in
//! particular are sparse (~15min), so a rescrape has to show the latest
//! observation rather than a rate.
//!
//! `/metrics` renders this through `opentelemetry-prometheus`, so the
//! exposition keeps the names and labels the fleet's dashboards and the
//! relay's federation already expect.

use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock};

use mac_mgmt_common::otel::opentelemetry;
use opentelemetry::metrics::AsyncInstrument;
use opentelemetry::{KeyValue, global};

use mac_mgmt_common::{
    DynamicSample, GpuInfo, GpuSample, Inventory, SecurityFinding, ServiceInventory, ServiceSample,
    ServiceSecurity,
};

use crate::assessment::probes::{ProbeKind, ProbeResult};

/// Metric scope for everything the daemon reports.
const SCOPE: &str = "mac-mgmt-daemon";

/// Security posture checks exported as `mac_mgmt_security_enabled{check}`,
/// paired with the finding id they are derived from.
const SECURITY_BOOLS: &[(&str, &str)] = &[
    ("sip", "macos_sip"),
    ("filevault", "macos_filevault"),
    ("firewall", "macos_firewall"),
    ("gatekeeper", "macos_gatekeeper"),
    ("fde", "linux_fde"),
    ("ufw", "linux_ufw"),
];

/// Thermal states the one-hot gauge always reports, so a dashboard sees a 0
/// rather than a gap for the states the host isn't in.
const THERMAL_STATES: &[&str] = &["nominal", "fair", "serious", "critical"];

// ── Observed state ──────────────────────────────────────────────────────

/// The last observation of one probe run.
#[derive(Clone)]
pub struct ProbeState {
    pub kind: &'static str,
    pub ok: bool,
    pub duration_ms: u64,
    pub first_token_ms: Option<u64>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub last_run: i64,
}

/// A managed service's lifecycle state.
#[derive(Clone, Default)]
pub struct ServiceState {
    pub healthy: bool,
    pub upgrade_pending: bool,
    pub busy: bool,
    /// 0=stopped, 1=starting, 2=healthy, 3=unhealthy, 4=crash-backoff,
    /// 5=installing, 6=install-failed.
    pub phase: i64,
}

/// Everything the observable instruments read at collection time.
///
/// A `BTreeMap` rather than a `HashMap` so the exposition is stable between
/// scrapes — a diff of two scrapes should show value changes, not reordering.
#[derive(Default)]
pub struct State {
    pub services: std::collections::BTreeMap<String, ServiceState>,
    pub sample: Option<DynamicSample>,
    pub gpu_samples: Vec<GpuSample>,
    pub inventory: Option<Inventory>,
    pub security: Vec<SecurityFinding>,
    pub probes: std::collections::BTreeMap<String, ProbeState>,
    pub service_samples: Vec<ServiceSample>,
    pub service_inventories: Vec<ServiceInventory>,
    pub service_security: Vec<ServiceSecurity>,
    pub heartbeat_last_success: i64,
}

/// Process-wide observed state. Instruments are registered against this once;
/// [`Metrics`] is a handle onto it.
static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

/// A poisoned lock here means a panic while updating one gauge. Keep
/// reporting rather than cascading the panic into every later scrape.
fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

// ── Instrument helpers ──────────────────────────────────────────────────

fn meter() -> opentelemetry::metrics::Meter {
    global::meter(SCOPE)
}

/// Register an observable gauge whose callback reads [`STATE`].
fn gauge_i64(
    name: &'static str,
    description: &'static str,
    observe: impl Fn(&State, &dyn AsyncInstrument<i64>) + Send + Sync + 'static,
) {
    meter()
        .i64_observable_gauge(name)
        .with_description(description)
        .with_callback(move |o| observe(&state(), o))
        .build();
}

fn gauge_f64(
    name: &'static str,
    description: &'static str,
    observe: impl Fn(&State, &dyn AsyncInstrument<f64>) + Send + Sync + 'static,
) {
    meter()
        .f64_observable_gauge(name)
        .with_description(description)
        .with_callback(move |o| observe(&state(), o))
        .build();
}

/// Attribute helper: `KeyValue::new` with a `'static` key.
fn kv(key: &'static str, value: impl Into<opentelemetry::Value>) -> KeyValue {
    KeyValue::new(key, value.into())
}

/// Heartbeat attempts by result. A real counter — this one is monotonic.
static HEARTBEATS: LazyLock<opentelemetry::metrics::Counter<u64>> = LazyLock::new(|| {
    meter()
        .u64_counter("mac_mgmt_heartbeat")
        .with_description("Total heartbeat attempts by result")
        .build()
});

// ── Registration ────────────────────────────────────────────────────────

/// Build every observable instrument. Called once per process by
/// [`Metrics::new`], after `otel::init` has installed a meter provider —
/// instruments built before that would bind to a no-op provider.
fn register(daemon_commit: &str) {
    // ── Dynamic sample (per-heartbeat, ~60s) ──

    gauge_f64(
        "mac_mgmt_cpu_load_1m",
        "1-minute CPU load average from the latest heartbeat sample",
        |s, o| {
            if let Some(sample) = &s.sample {
                o.observe(sample.cpu_load_1m as f64, &[]);
            }
        },
    );

    for (name, description, pick) in [
        (
            "mac_mgmt_mem_used_bytes",
            "Resident memory in use, bytes",
            (|s: &DynamicSample| s.mem_used_bytes) as fn(&DynamicSample) -> u64,
        ),
        (
            "mac_mgmt_mem_total_bytes",
            "Total physical memory, bytes",
            |s| s.mem_total_bytes,
        ),
        (
            "mac_mgmt_swap_used_bytes",
            "Swap in use, bytes (0 when disabled)",
            |s| s.swap_used_bytes,
        ),
        (
            "mac_mgmt_net_rx_bytes",
            "Network bytes received since the sample collector was initialised",
            |s| s.net_rx_bytes,
        ),
        (
            "mac_mgmt_net_tx_bytes",
            "Network bytes transmitted since the sample collector was initialised",
            |s| s.net_tx_bytes,
        ),
        (
            "mac_mgmt_process_count",
            "Total process count",
            |s| s.process_count as u64,
        ),
    ] {
        gauge_i64(name, description, move |s, o| {
            if let Some(sample) = &s.sample {
                o.observe(pick(sample) as i64, &[]);
            }
        });
    }

    for (name, description, pick) in [
        (
            "mac_mgmt_disk_free_bytes",
            "Free bytes per tracked mount",
            (|d: &mac_mgmt_common::DiskFree| d.free_bytes) as fn(&mac_mgmt_common::DiskFree) -> u64,
        ),
        (
            "mac_mgmt_disk_total_bytes",
            "Total bytes per tracked mount",
            |d| d.total_bytes,
        ),
    ] {
        gauge_i64(name, description, move |s, o| {
            let Some(sample) = &s.sample else { return };
            for d in &sample.disk_free {
                o.observe(pick(d) as i64, &[kv("mount", d.mount.clone())]);
            }
        });
    }

    gauge_i64(
        "mac_mgmt_thermal_state",
        "One-hot thermal state; the label matching the current state is 1",
        |s, o| {
            let Some(sample) = &s.sample else { return };
            let current = sample.thermal_state.as_deref();
            for state in THERMAL_STATES {
                o.observe(i64::from(current == Some(state)), &[kv("state", *state)]);
            }
        },
    );

    // ── GPUs — inventory on the inventory tick, state each heartbeat ──

    gauge_i64(
        "mac_mgmt_gpu_info",
        "Per-GPU static inventory, always 1; identity on labels",
        |s, o| {
            let Some(inv) = &s.inventory else { return };
            for g in &inv.gpus {
                o.observe(1, &gpu_info_attrs(g));
            }
        },
    );
    gauge_i64(
        "mac_mgmt_gpu_vram_total_bytes",
        "Total VRAM per GPU, bytes. 0 when the vendor tool doesn't report it.",
        |s, o| {
            let Some(inv) = &s.inventory else { return };
            for g in &inv.gpus {
                o.observe(g.vram_total_bytes as i64, &[kv("index", g.index as i64)]);
            }
        },
    );

    for (name, description, pick) in [
        (
            "mac_mgmt_gpu_vram_used_bytes",
            "VRAM in use per GPU, bytes",
            (|g: &GpuSample| g.vram_used_bytes.map(|v| v as i64))
                as fn(&GpuSample) -> Option<i64>,
        ),
        (
            "mac_mgmt_gpu_utilization_pct",
            "GPU utilization percentage (0-100)",
            |g| g.utilization_pct.map(|v| v as i64),
        ),
        (
            "mac_mgmt_gpu_temperature_celsius",
            "GPU core temperature, °C",
            |g| g.temperature_c.map(|v| v as i64),
        ),
    ] {
        gauge_i64(name, description, move |s, o| {
            for g in &s.gpu_samples {
                if let Some(v) = pick(g) {
                    o.observe(v, &[kv("index", g.index as i64)]);
                }
            }
        });
    }

    gauge_f64("mac_mgmt_gpu_power_watts", "GPU power draw, watts", |s, o| {
        for g in &s.gpu_samples {
            if let Some(v) = g.power_watts {
                o.observe(v as f64, &[kv("index", g.index as i64)]);
            }
        }
    });

    // ── Inventory (every ~6h) ──

    for (name, description, pick) in [
        (
            "mac_mgmt_host_uptime_secs",
            "Host uptime in seconds at last inventory",
            (|i: &Inventory| i.uptime_secs as i64) as fn(&Inventory) -> i64,
        ),
        (
            "mac_mgmt_cpu_cores_logical",
            "Logical CPU cores",
            |i| i.cpu_cores_logical as i64,
        ),
        (
            "mac_mgmt_cpu_cores_physical",
            "Physical CPU cores",
            |i| i.cpu_cores_physical as i64,
        ),
        (
            "mac_mgmt_inventory_mem_total_bytes",
            "Total physical memory at last inventory",
            |i| i.mem_total_bytes as i64,
        ),
    ] {
        gauge_i64(name, description, move |s, o| {
            if let Some(inv) = &s.inventory {
                o.observe(pick(inv), &[]);
            }
        });
    }

    gauge_i64(
        "mac_mgmt_inventory_info",
        "Static inventory facts, always 1; structural fields on labels",
        |s, o| {
            let Some(i) = &s.inventory else { return };
            o.observe(
                1,
                &[
                    kv("os_name", i.os_name.clone()),
                    kv("os_version", i.os_version.clone()),
                    kv("kernel_version", i.kernel_version.clone()),
                    kv("arch", i.arch.clone()),
                    kv("cpu_model", i.cpu_model.clone()),
                    kv("supervisor", i.supervisor.clone().unwrap_or_default()),
                    kv("nix_version", i.nix_version.clone().unwrap_or_default()),
                    kv("nixpkgs_commit", i.nixpkgs_commit.clone().unwrap_or_default()),
                ],
            );
        },
    );

    // ── Security (every ~6h) ──

    gauge_i64(
        "mac_mgmt_security_enabled",
        "Security posture toggle per check: -1 unknown, 0 off, 1 on",
        |s, o| {
            for (check, id) in SECURITY_BOOLS {
                let value = s
                    .security
                    .iter()
                    .find(|f| f.id == *id)
                    .map(|f| i64::from(f.pass))
                    // Absent is not the same as off: the check may not apply
                    // to this platform, or may not have run yet.
                    .unwrap_or(-1);
                o.observe(value, &[kv("check", *check)]);
            }
        },
    );

    gauge_i64(
        "mac_mgmt_security_info",
        "Security posture string fields, always 1; values on labels",
        |s, o| {
            let find = |id: &str| s.security.iter().find(|f| f.id == id);
            let strip = |id: &str, prefix: &str| {
                find(id)
                    .map(|f| {
                        f.message
                            .strip_prefix(prefix)
                            .unwrap_or(&f.message)
                            .to_string()
                    })
                    .unwrap_or_default()
            };
            o.observe(
                1,
                &[
                    kv(
                        "xprotect_version",
                        strip("macos_xprotect", "XProtect definitions version "),
                    ),
                    kv("selinux_mode", strip("linux_selinux", "SELinux mode: ")),
                    kv(
                        "apparmor_profiles",
                        find("linux_apparmor")
                            .map(|f| f.message.clone())
                            .unwrap_or_default(),
                    ),
                ],
            );
        },
    );

    gauge_i64(
        "mac_mgmt_security_nftables_rule_count",
        "Linux: number of rules loaded in nftables. -1 when nft can't be read.",
        |s, o| {
            let n = s
                .security
                .iter()
                .find(|f| f.id == "linux_nftables")
                .and_then(|f| f.message.split_whitespace().next()?.parse::<i64>().ok())
                .unwrap_or(-1);
            o.observe(n, &[]);
        },
    );

    // ── Probes (every ~15min) ──

    gauge_i64(
        "mac_mgmt_probe_ok",
        "Last probe outcome: 1 ok, 0 failed. Absent until the first run.",
        |s, o| {
            for (service, p) in &s.probes {
                o.observe(
                    i64::from(p.ok),
                    &[kv("service", service.clone()), kv("kind", p.kind)],
                );
            }
        },
    );

    for (name, description, pick) in [
        (
            "mac_mgmt_probe_duration_ms",
            "Last probe wall-clock duration in milliseconds",
            (|p: &ProbeState| Some(p.duration_ms as i64)) as fn(&ProbeState) -> Option<i64>,
        ),
        (
            "mac_mgmt_probe_first_token_ms",
            "Last probe first-token latency in milliseconds (LLM backends only)",
            |p| p.first_token_ms.map(|v| v as i64),
        ),
        (
            "mac_mgmt_probe_tokens_in",
            "Last probe prompt-token count (LLM backends only)",
            |p| p.tokens_in.map(|v| v as i64),
        ),
        (
            "mac_mgmt_probe_tokens_out",
            "Last probe completion-token count (LLM backends only)",
            |p| p.tokens_out.map(|v| v as i64),
        ),
        (
            "mac_mgmt_probe_last_run_timestamp_seconds",
            "Unix timestamp of the last probe run",
            |p| Some(p.last_run),
        ),
    ] {
        gauge_i64(name, description, move |s, o| {
            for (service, p) in &s.probes {
                if let Some(v) = pick(p) {
                    o.observe(v, &[kv("service", service.clone())]);
                }
            }
        });
    }

    // ── Service-level assessment ──

    gauge_i64(
        "mac_mgmt_service_sample",
        "Per-service dynamic sample value",
        |s, o| {
            for ss in &s.service_samples {
                for entry in &ss.entries {
                    if let Some(n) = entry
                        .value
                        .as_i64()
                        .or_else(|| entry.value.as_f64().map(|f| f as i64))
                    {
                        o.observe(
                            n,
                            &[
                                kv("service", ss.service.clone()),
                                kv("entry", entry.id.clone()),
                            ],
                        );
                    }
                }
            }
        },
    );

    gauge_i64(
        "mac_mgmt_service_inventory",
        "Per-service inventory entry (info-style: value on label, gauge=1)",
        |s, o| {
            for si in &s.service_inventories {
                for entry in &si.entries {
                    let value = match &entry.value {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    o.observe(
                        1,
                        &[
                            kv("service", si.service.clone()),
                            kv("entry", entry.id.clone()),
                            kv("value", value),
                        ],
                    );
                }
            }
        },
    );

    gauge_i64(
        "mac_mgmt_service_security",
        "Per-service security finding (1=pass, 0=fail)",
        |s, o| {
            for sf in &s.service_security {
                for finding in &sf.findings {
                    o.observe(
                        i64::from(finding.pass),
                        &[
                            kv("service", sf.service.clone()),
                            kv("check", finding.id.clone()),
                        ],
                    );
                }
            }
        },
    );

    // ── Managed service lifecycle ──

    for (name, description, pick) in [
        (
            "mac_mgmt_service_healthy",
            "Whether the service is healthy (1) or not (0)",
            (|s: &ServiceState| i64::from(s.healthy)) as fn(&ServiceState) -> i64,
        ),
        (
            "mac_mgmt_service_upgrade_pending",
            "Whether an upgrade is pending (1) or not (0)",
            |s| i64::from(s.upgrade_pending),
        ),
        (
            "mac_mgmt_service_busy",
            "Whether the service is busy (1) or not (0)",
            |s| i64::from(s.busy),
        ),
        (
            "mac_mgmt_service_phase",
            "Service lifecycle phase (0=stopped, 1=starting, 2=healthy, 3=unhealthy, \
             4=crash-backoff, 5=installing, 6=install-failed)",
            |s| s.phase,
        ),
    ] {
        gauge_i64(name, description, move |s, o| {
            for (service, svc) in &s.services {
                o.observe(pick(svc), &[kv("service", service.clone())]);
            }
        });
    }

    // ── Heartbeat and build info ──

    gauge_i64(
        "mac_mgmt_heartbeat_last_success_timestamp_seconds",
        "Unix timestamp of the last successful heartbeat",
        |s, o| o.observe(s.heartbeat_last_success, &[]),
    );

    let build = [
        kv("version", env!("CARGO_PKG_VERSION")),
        kv("commit", daemon_commit.to_string()),
    ];
    meter()
        .i64_observable_gauge("mac_mgmt_build_info")
        .with_description("Daemon build metadata, always 1; version and commit on labels")
        .with_callback(move |o| o.observe(1, &build))
        .build();

    LazyLock::force(&HEARTBEATS);
}

fn gpu_info_attrs(g: &GpuInfo) -> [KeyValue; 5] {
    [
        kv("index", g.index as i64),
        kv("vendor", g.vendor.clone()),
        kv("name", g.name.clone()),
        kv("driver_version", g.driver_version.clone().unwrap_or_default()),
        kv("pci_bus_id", g.pci_bus_id.clone().unwrap_or_default()),
    ]
}

// ── Update API ──────────────────────────────────────────────────────────

/// Writes the assessment tiers into the observed state. Kept as a separate
/// type so call sites read the same as they did against the gauge structs.
pub struct AssessmentMetrics;

impl AssessmentMetrics {
    pub fn update_gpu_inventory(&self, gpus: &[GpuInfo]) {
        // GPU inventory travels with the host inventory; a caller that has
        // only the GPU list patches it in.
        if let Some(inv) = state().inventory.as_mut() {
            inv.gpus = gpus.to_vec();
        }
    }

    pub fn update_gpu_samples(&self, samples: &[GpuSample]) {
        state().gpu_samples = samples.to_vec();
    }

    pub fn update_sample(&self, s: &DynamicSample) {
        let mut state = state();
        state.gpu_samples = s.gpus.clone();
        state.sample = Some(s.clone());
    }

    pub fn update_inventory(&self, inv: &Inventory) {
        state().inventory = Some(inv.clone());
    }

    pub fn update_security(&self, findings: &[SecurityFinding]) {
        state().security = findings.to_vec();
    }

    pub fn update_probe(&self, service: &str, kind: ProbeKind, r: &ProbeResult) {
        state().probes.insert(
            service.to_string(),
            ProbeState {
                kind: kind.as_str(),
                ok: r.ok,
                duration_ms: r.duration_ms,
                first_token_ms: r.first_token_ms,
                tokens_in: r.tokens_in,
                tokens_out: r.tokens_out,
                last_run: chrono::Utc::now().timestamp(),
            },
        );
    }

    pub fn update_service_samples(&self, samples: &[ServiceSample]) {
        state().service_samples = samples.to_vec();
    }

    pub fn update_service_inventories(&self, inventories: &[ServiceInventory]) {
        state().service_inventories = inventories.to_vec();
    }

    pub fn update_service_security(&self, findings: &[ServiceSecurity]) {
        state().service_security = findings.to_vec();
    }
}

/// Handle onto the process's metric state.
pub struct Metrics {
    pub assessment: AssessmentMetrics,
    pub daemon_version: String,
    pub daemon_commit: String,
    pub started_at: std::time::Instant,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    pub fn new() -> Self {
        let daemon_commit = env!("GIT_SHA").to_string();

        // Instruments are process-wide: registering a second set would double
        // every series. A second handle just shares the first's — and waits
        // for it, so nothing can scrape a half-registered set.
        static REGISTERED: OnceLock<()> = OnceLock::new();
        REGISTERED.get_or_init(|| register(&daemon_commit));

        Metrics {
            assessment: AssessmentMetrics,
            daemon_version: env!("CARGO_PKG_VERSION").to_string(),
            daemon_commit,
            started_at: std::time::Instant::now(),
        }
    }

    pub fn set_service_state(
        &self,
        name: &str,
        healthy: bool,
        upgrade_pending: bool,
        busy: bool,
        phase: i64,
    ) {
        state().services.insert(
            name.to_string(),
            ServiceState {
                healthy,
                upgrade_pending,
                busy,
                phase,
            },
        );
    }

    pub fn record_heartbeat_success(&self) {
        state().heartbeat_last_success = chrono::Utc::now().timestamp();
        HEARTBEATS.add(1, &[kv("result", "success")]);
    }

    pub fn record_heartbeat_failure(&self) {
        HEARTBEATS.add(1, &[kv("result", "failure")]);
    }

    pub fn status(&self) -> (String, u64, Vec<(String, bool, bool, bool, &'static str)>) {
        let services = state()
            .services
            .iter()
            .map(|(name, s)| {
                let phase = match s.phase {
                    0 => "stopped",
                    1 => "starting",
                    2 => "healthy",
                    3 => "unhealthy",
                    4 => "crash-backoff",
                    5 => "installing",
                    6 => "install-failed",
                    _ => "unknown",
                };
                (name.clone(), s.healthy, s.upgrade_pending, s.busy, phase)
            })
            .collect();

        (
            self.daemon_version.clone(),
            self.started_at.elapsed().as_secs(),
            services,
        )
    }

    /// The Prometheus exposition, converted from the OTel pipeline. Collecting
    /// runs the observable callbacks, so a scrape reads state at scrape time.
    pub fn render(&self) -> String {
        match mac_mgmt_common::metrics::prom::encode() {
            Ok(text) => text,
            Err(e) => {
                tracing::warn!("metrics encode failed: {e}");
                String::new()
            }
        }
    }
}

/// Register a service-owned observable gauge over a value the service keeps
/// up to date in `collect_metrics`.
///
/// ```ignore
/// let loaded = Arc::new(AtomicI64::new(0));
/// observable_gauge("mac_mgmt_ollama_loaded_models", "...", Arc::clone(&loaded));
/// ```
pub fn observable_gauge(
    name: &'static str,
    description: &'static str,
    value: Arc<std::sync::atomic::AtomicI64>,
) {
    meter()
        .i64_observable_gauge(name)
        .with_description(description)
        .with_callback(move |o| o.observe(value.load(std::sync::atomic::Ordering::Relaxed), &[]))
        .build();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reflects_the_last_service_update() {
        // Instruments bind to whatever meter provider is installed when they
        // are built, so the pipeline has to exist before the first handle.
        mac_mgmt_common::otel::init("mac-mgmt-daemon", "info");
        let m = Metrics::new();
        m.set_service_state("ollama", true, false, false, 2);
        m.set_service_state("hermes", false, true, true, 3);

        let (_version, _uptime, services) = m.status();
        let by_name: std::collections::HashMap<_, _> =
            services.into_iter().map(|s| (s.0.clone(), s)).collect();

        assert_eq!(by_name["ollama"].4, "healthy");
        assert!(by_name["ollama"].1);
        assert_eq!(by_name["hermes"].4, "unhealthy");
        assert!(by_name["hermes"].2 && by_name["hermes"].3);

        // A later update replaces the earlier one rather than accumulating.
        m.set_service_state("hermes", true, false, false, 2);
        let (_, _, services) = m.status();
        let hermes = services.iter().find(|s| s.0 == "hermes").expect("hermes");
        assert_eq!(hermes.4, "healthy");
    }

    /// The exposition is what the fleet's dashboards and the relay's
    /// federation consume, so the converter's naming has to keep matching the
    /// names the Prometheus registry used to emit.
    #[test]
    fn exposition_keeps_the_names_the_fleet_scrapes() {
        mac_mgmt_common::otel::init("mac-mgmt-daemon", "info");
        let m = Metrics::new();

        m.set_service_state("exposition-probe", true, false, false, 2);
        m.record_heartbeat_success();
        m.assessment.update_sample(&DynamicSample {
            cpu_load_1m: 1.5,
            mem_used_bytes: 2048,
            mem_total_bytes: 8192,
            swap_used_bytes: 0,
            disk_free: vec![mac_mgmt_common::DiskFree {
                mount: "/nix/store".into(),
                free_bytes: 10,
                total_bytes: 100,
            }],
            net_rx_bytes: 7,
            net_tx_bytes: 9,
            process_count: 42,
            thermal_state: Some("fair".into()),
            gpus: Vec::new(),
        });

        let out = m.render();
        for name in [
            "mac_mgmt_cpu_load_1m",
            "mac_mgmt_mem_used_bytes",
            "mac_mgmt_process_count",
            "mac_mgmt_service_healthy",
            "mac_mgmt_build_info",
            // A counter picks up the _total suffix on the way through the
            // converter, exactly as the IntCounterVec did.
            "mac_mgmt_heartbeat_total",
        ] {
            assert!(out.contains(name), "{name} missing from:\n{out}");
        }

        assert!(
            out.contains(r#"mac_mgmt_service_healthy{service="exposition-probe"} 1"#),
            "{out}"
        );
        assert!(
            out.contains(r#"mac_mgmt_disk_free_bytes{mount="/nix/store"} 10"#),
            "{out}"
        );
        // One-hot: the state the host is in reads 1, the others 0.
        assert!(out.contains(r#"mac_mgmt_thermal_state{state="fair"} 1"#), "{out}");
        assert!(
            out.contains(r#"mac_mgmt_thermal_state{state="critical"} 0"#),
            "{out}"
        );
    }

    #[test]
    fn security_bools_report_unknown_when_the_check_did_not_run() {
        mac_mgmt_common::otel::init("mac-mgmt-daemon", "info");
        let m = Metrics::new();
        m.assessment.update_security(&[SecurityFinding {
            id: "macos_sip".into(),
            severity: mac_mgmt_common::FindingSeverity::Info,
            pass: true,
            message: "enabled".into(),
        }]);

        let s = state();
        let value = |id: &str| {
            s.security
                .iter()
                .find(|f| f.id == id)
                .map(|f| i64::from(f.pass))
                .unwrap_or(-1)
        };
        assert_eq!(value("macos_sip"), 1);
        assert_eq!(value("macos_filevault"), -1);
    }
}
