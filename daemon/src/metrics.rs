use prometheus::{
    Encoder, Gauge, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry, TextEncoder,
};

use mac_mgmt_common::{
    DynamicSample, GpuInfo, GpuSample, Inventory, SecurityFinding, ServiceInventory, ServiceSample,
    ServiceSecurity,
};

use crate::assessment::probes::{ProbeKind, ProbeResult};

/// Prometheus surface for the system-assessment module.
///
/// Each tier (sample / inventory / security / probes) updates its own
/// gauges. Everything is a gauge (or gauge-vec) so that a rescrape always
/// shows the latest observation — probes in particular are sparse (~15min)
/// so a counter would be misleading.
pub struct AssessmentMetrics {
    // Dynamic sample (per-heartbeat ~60s).
    pub cpu_load_1m: Gauge,
    pub mem_used_bytes: IntGauge,
    pub mem_total_bytes: IntGauge,
    pub swap_used_bytes: IntGauge,
    pub disk_free_bytes: IntGaugeVec,
    pub disk_total_bytes: IntGaugeVec,
    pub net_rx_bytes: IntGauge,
    pub net_tx_bytes: IntGauge,
    pub process_count: IntGauge,
    /// One-hot gauge by state label ("nominal" / "fair" / "serious" / "critical").
    pub thermal_state: IntGaugeVec,

    // Inventory (every ~6h).
    pub uptime_secs: IntGauge,
    pub cpu_cores_logical: IntGauge,
    pub cpu_cores_physical: IntGauge,
    pub inventory_mem_total_bytes: IntGauge,
    /// Info-style gauge: always 1, all structural fields on labels.
    pub inventory_info: IntGaugeVec,

    // Security (every ~6h). Booleans are -1 = unknown, 0 = off, 1 = on.
    pub security_bool: IntGaugeVec,
    /// Info-style gauge: always 1; labels carry version strings.
    pub security_info: IntGaugeVec,
    /// Linux-only: number of rules in `nft list ruleset`. Unset when nft
    /// isn't readable (missing / permission denied / non-Linux host).
    pub security_nftables_rule_count: IntGauge,

    // GPUs — inventory refreshed on the inventory tick, state each heartbeat.
    /// Info-style gauge: always 1, per-GPU identity on labels.
    pub gpu_info: IntGaugeVec,
    pub gpu_vram_total_bytes: IntGaugeVec,
    pub gpu_vram_used_bytes: IntGaugeVec,
    pub gpu_utilization_pct: IntGaugeVec,
    pub gpu_temperature_celsius: IntGaugeVec,
    pub gpu_power_watts: Gauge,
    /// Only used as a placeholder — the real power gauge is a GaugeVec below.
    pub gpu_power_watts_vec: prometheus::GaugeVec,

    // Probes (every ~15min).
    pub probe_ok: IntGaugeVec,
    pub probe_duration_ms: IntGaugeVec,
    pub probe_first_token_ms: IntGaugeVec,
    pub probe_tokens_in: IntGaugeVec,
    pub probe_tokens_out: IntGaugeVec,
    pub probe_last_run_timestamp: IntGaugeVec,

    // Service-level assessment.
    pub service_sample: IntGaugeVec,
    pub service_inventory: IntGaugeVec,
    pub service_security: IntGaugeVec,
}

impl AssessmentMetrics {
    fn new(registry: &Registry) -> Self {
        let cpu_load_1m = Gauge::with_opts(Opts::new(
            "mac_mgmt_cpu_load_1m",
            "1-minute CPU load average from the latest heartbeat sample",
        ))
        .unwrap();
        let mem_used_bytes = IntGauge::with_opts(Opts::new(
            "mac_mgmt_mem_used_bytes",
            "Resident memory in use, bytes",
        ))
        .unwrap();
        let mem_total_bytes = IntGauge::with_opts(Opts::new(
            "mac_mgmt_mem_total_bytes",
            "Total physical memory, bytes",
        ))
        .unwrap();
        let swap_used_bytes = IntGauge::with_opts(Opts::new(
            "mac_mgmt_swap_used_bytes",
            "Swap in use, bytes (0 when disabled)",
        ))
        .unwrap();
        let disk_free_bytes = IntGaugeVec::new(
            Opts::new("mac_mgmt_disk_free_bytes", "Free bytes per tracked mount"),
            &["mount"],
        )
        .unwrap();
        let disk_total_bytes = IntGaugeVec::new(
            Opts::new("mac_mgmt_disk_total_bytes", "Total bytes per tracked mount"),
            &["mount"],
        )
        .unwrap();
        let net_rx_bytes = IntGauge::with_opts(Opts::new(
            "mac_mgmt_net_rx_bytes",
            "Network bytes received since the sample collector was initialised",
        ))
        .unwrap();
        let net_tx_bytes = IntGauge::with_opts(Opts::new(
            "mac_mgmt_net_tx_bytes",
            "Network bytes transmitted since the sample collector was initialised",
        ))
        .unwrap();
        let process_count =
            IntGauge::with_opts(Opts::new("mac_mgmt_process_count", "Total process count"))
                .unwrap();
        let thermal_state = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_thermal_state",
                "One-hot thermal state; the label matching the current state is 1",
            ),
            &["state"],
        )
        .unwrap();

        let uptime_secs = IntGauge::with_opts(Opts::new(
            "mac_mgmt_host_uptime_secs",
            "Host uptime in seconds at last inventory",
        ))
        .unwrap();
        let cpu_cores_logical =
            IntGauge::with_opts(Opts::new("mac_mgmt_cpu_cores_logical", "Logical CPU cores"))
                .unwrap();
        let cpu_cores_physical = IntGauge::with_opts(Opts::new(
            "mac_mgmt_cpu_cores_physical",
            "Physical CPU cores",
        ))
        .unwrap();
        let inventory_mem_total_bytes = IntGauge::with_opts(Opts::new(
            "mac_mgmt_inventory_mem_total_bytes",
            "Total physical memory at last inventory",
        ))
        .unwrap();
        let inventory_info = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_inventory_info",
                "Static inventory facts, always 1; structural fields on labels",
            ),
            &[
                "os_name",
                "os_version",
                "kernel_version",
                "arch",
                "cpu_model",
                "supervisor",
                "nix_version",
                "nixpkgs_commit",
            ],
        )
        .unwrap();

        let security_bool = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_security_enabled",
                "Security posture toggle per check: -1 unknown, 0 off, 1 on",
            ),
            &["check"],
        )
        .unwrap();
        let security_info = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_security_info",
                "Security posture string fields, always 1; values on labels",
            ),
            &["xprotect_version", "selinux_mode", "apparmor_profiles"],
        )
        .unwrap();
        let security_nftables_rule_count = IntGauge::with_opts(Opts::new(
            "mac_mgmt_security_nftables_rule_count",
            "Linux: number of rules loaded in nftables. Absent when nft can't be read.",
        ))
        .unwrap();

        let gpu_info = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_gpu_info",
                "Per-GPU static inventory, always 1; identity on labels",
            ),
            &["index", "vendor", "name", "driver_version", "pci_bus_id"],
        )
        .unwrap();
        let gpu_vram_total_bytes = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_gpu_vram_total_bytes",
                "Total VRAM per GPU, bytes. 0 when the vendor tool doesn't report it.",
            ),
            &["index"],
        )
        .unwrap();
        let gpu_vram_used_bytes = IntGaugeVec::new(
            Opts::new("mac_mgmt_gpu_vram_used_bytes", "VRAM in use per GPU, bytes"),
            &["index"],
        )
        .unwrap();
        let gpu_utilization_pct = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_gpu_utilization_pct",
                "GPU utilization percentage (0-100)",
            ),
            &["index"],
        )
        .unwrap();
        let gpu_temperature_celsius = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_gpu_temperature_celsius",
                "GPU core temperature, °C",
            ),
            &["index"],
        )
        .unwrap();
        let gpu_power_watts_vec = prometheus::GaugeVec::new(
            Opts::new("mac_mgmt_gpu_power_watts", "GPU power draw, watts"),
            &["index"],
        )
        .unwrap();
        // Placeholder — unused but kept so struct init stays aligned with docs.
        let gpu_power_watts = Gauge::with_opts(Opts::new(
            "mac_mgmt_gpu_power_watts_aggregate",
            "Reserved — not exported",
        ))
        .unwrap();

        let probe_ok = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_probe_ok",
                "Last probe outcome: 1 ok, 0 failed. Absent until the first run.",
            ),
            &["service", "kind"],
        )
        .unwrap();
        let probe_duration_ms = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_probe_duration_ms",
                "Last probe wall-clock duration in milliseconds",
            ),
            &["service"],
        )
        .unwrap();
        let probe_first_token_ms = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_probe_first_token_ms",
                "Last probe first-token latency in milliseconds (LLM backends only)",
            ),
            &["service"],
        )
        .unwrap();
        let probe_tokens_in = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_probe_tokens_in",
                "Last probe prompt-token count (LLM backends only)",
            ),
            &["service"],
        )
        .unwrap();
        let probe_tokens_out = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_probe_tokens_out",
                "Last probe completion-token count (LLM backends only)",
            ),
            &["service"],
        )
        .unwrap();
        let probe_last_run_timestamp = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_probe_last_run_timestamp_seconds",
                "Unix timestamp of the last probe run",
            ),
            &["service"],
        )
        .unwrap();

        let service_sample = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_service_sample",
                "Per-service dynamic sample value",
            ),
            &["service", "entry"],
        )
        .unwrap();
        let service_inventory = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_service_inventory",
                "Per-service inventory entry (info-style: value on label, gauge=1)",
            ),
            &["service", "entry", "value"],
        )
        .unwrap();
        let service_security = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_service_security",
                "Per-service security finding (1=pass, 0=fail)",
            ),
            &["service", "check"],
        )
        .unwrap();

        for c in [
            Box::new(cpu_load_1m.clone()) as Box<dyn prometheus::core::Collector>,
            Box::new(mem_used_bytes.clone()),
            Box::new(mem_total_bytes.clone()),
            Box::new(swap_used_bytes.clone()),
            Box::new(disk_free_bytes.clone()),
            Box::new(disk_total_bytes.clone()),
            Box::new(net_rx_bytes.clone()),
            Box::new(net_tx_bytes.clone()),
            Box::new(process_count.clone()),
            Box::new(thermal_state.clone()),
            Box::new(uptime_secs.clone()),
            Box::new(cpu_cores_logical.clone()),
            Box::new(cpu_cores_physical.clone()),
            Box::new(inventory_mem_total_bytes.clone()),
            Box::new(inventory_info.clone()),
            Box::new(security_bool.clone()),
            Box::new(security_info.clone()),
            Box::new(security_nftables_rule_count.clone()),
            Box::new(gpu_info.clone()),
            Box::new(gpu_vram_total_bytes.clone()),
            Box::new(gpu_vram_used_bytes.clone()),
            Box::new(gpu_utilization_pct.clone()),
            Box::new(gpu_temperature_celsius.clone()),
            Box::new(gpu_power_watts_vec.clone()),
            Box::new(probe_ok.clone()),
            Box::new(probe_duration_ms.clone()),
            Box::new(probe_first_token_ms.clone()),
            Box::new(probe_tokens_in.clone()),
            Box::new(probe_tokens_out.clone()),
            Box::new(probe_last_run_timestamp.clone()),
            Box::new(service_sample.clone()),
            Box::new(service_inventory.clone()),
            Box::new(service_security.clone()),
        ] {
            registry.register(c).unwrap();
        }

        Self {
            cpu_load_1m,
            mem_used_bytes,
            mem_total_bytes,
            swap_used_bytes,
            disk_free_bytes,
            disk_total_bytes,
            net_rx_bytes,
            net_tx_bytes,
            process_count,
            thermal_state,
            uptime_secs,
            cpu_cores_logical,
            cpu_cores_physical,
            inventory_mem_total_bytes,
            inventory_info,
            security_bool,
            security_info,
            security_nftables_rule_count,
            gpu_info,
            gpu_vram_total_bytes,
            gpu_vram_used_bytes,
            gpu_utilization_pct,
            gpu_temperature_celsius,
            gpu_power_watts,
            gpu_power_watts_vec,
            probe_ok,
            probe_duration_ms,
            probe_first_token_ms,
            probe_tokens_in,
            probe_tokens_out,
            probe_last_run_timestamp,
            service_sample,
            service_inventory,
            service_security,
        }
    }

    pub fn update_gpu_inventory(&self, gpus: &[GpuInfo]) {
        self.gpu_info.reset();
        self.gpu_vram_total_bytes.reset();
        for g in gpus {
            let idx = g.index.to_string();
            self.gpu_info
                .with_label_values(&[
                    idx.as_str(),
                    g.vendor.as_str(),
                    g.name.as_str(),
                    g.driver_version.as_deref().unwrap_or(""),
                    g.pci_bus_id.as_deref().unwrap_or(""),
                ])
                .set(1);
            self.gpu_vram_total_bytes
                .with_label_values(&[idx.as_str()])
                .set(g.vram_total_bytes as i64);
        }
    }

    pub fn update_gpu_samples(&self, samples: &[GpuSample]) {
        self.gpu_vram_used_bytes.reset();
        self.gpu_utilization_pct.reset();
        self.gpu_temperature_celsius.reset();
        self.gpu_power_watts_vec.reset();
        for s in samples {
            let idx = s.index.to_string();
            if let Some(v) = s.vram_used_bytes {
                self.gpu_vram_used_bytes
                    .with_label_values(&[idx.as_str()])
                    .set(v as i64);
            }
            if let Some(v) = s.utilization_pct {
                self.gpu_utilization_pct
                    .with_label_values(&[idx.as_str()])
                    .set(v as i64);
            }
            if let Some(v) = s.temperature_c {
                self.gpu_temperature_celsius
                    .with_label_values(&[idx.as_str()])
                    .set(v as i64);
            }
            if let Some(v) = s.power_watts {
                self.gpu_power_watts_vec
                    .with_label_values(&[idx.as_str()])
                    .set(v as f64);
            }
        }
    }

    pub fn update_sample(&self, s: &DynamicSample) {
        self.cpu_load_1m.set(s.cpu_load_1m as f64);
        self.mem_used_bytes.set(s.mem_used_bytes as i64);
        self.mem_total_bytes.set(s.mem_total_bytes as i64);
        self.swap_used_bytes.set(s.swap_used_bytes as i64);
        for df in &s.disk_free {
            self.disk_free_bytes
                .with_label_values(&[&df.mount])
                .set(df.free_bytes as i64);
            self.disk_total_bytes
                .with_label_values(&[&df.mount])
                .set(df.total_bytes as i64);
        }
        self.net_rx_bytes.set(s.net_rx_bytes as i64);
        self.net_tx_bytes.set(s.net_tx_bytes as i64);
        self.process_count.set(s.process_count as i64);
        // One-hot thermal state: zero out the known labels, then set the current.
        for state in ["nominal", "fair", "serious", "critical"] {
            self.thermal_state.with_label_values(&[state]).set(0);
        }
        if let Some(state) = s.thermal_state.as_deref() {
            self.thermal_state.with_label_values(&[state]).set(1);
        }
        self.update_gpu_samples(&s.gpus);
    }

    pub fn update_inventory(&self, inv: &Inventory) {
        self.uptime_secs.set(inv.uptime_secs as i64);
        self.cpu_cores_logical.set(inv.cpu_cores_logical as i64);
        self.cpu_cores_physical.set(inv.cpu_cores_physical as i64);
        self.inventory_mem_total_bytes
            .set(inv.mem_total_bytes as i64);
        // Reset — an inventory update may change the label set (e.g. OS upgrade).
        self.inventory_info.reset();
        self.inventory_info
            .with_label_values(&[
                inv.os_name.as_str(),
                inv.os_version.as_str(),
                inv.kernel_version.as_str(),
                inv.arch.as_str(),
                inv.cpu_model.as_str(),
                inv.supervisor.as_deref().unwrap_or(""),
                inv.nix_version.as_deref().unwrap_or(""),
                inv.nixpkgs_commit.as_deref().unwrap_or(""),
            ])
            .set(1);
        self.update_gpu_inventory(&inv.gpus);
    }

    pub fn update_security(&self, findings: &[SecurityFinding]) {
        // Map findings back to the bool gauges by id.
        let bool_ids = [
            "macos_sip",
            "macos_filevault",
            "macos_firewall",
            "macos_gatekeeper",
            "linux_fde",
            "linux_ufw",
        ];
        let gauge_names = ["sip", "filevault", "firewall", "gatekeeper", "fde", "ufw"];
        for (id, gauge_name) in bool_ids.iter().zip(gauge_names.iter()) {
            let value = findings
                .iter()
                .find(|f| f.id == *id)
                .map(|f| if f.pass { 1i64 } else { 0 })
                .unwrap_or(-1);
            self.security_bool
                .with_label_values(&[gauge_name])
                .set(value);
        }

        self.security_info.reset();
        let xprotect = findings.iter().find(|f| f.id == "macos_xprotect");
        let selinux = findings.iter().find(|f| f.id == "linux_selinux");
        let apparmor = findings.iter().find(|f| f.id == "linux_apparmor");
        self.security_info
            .with_label_values(&[
                xprotect
                    .map(|f| {
                        f.message
                            .strip_prefix("XProtect definitions version ")
                            .unwrap_or(&f.message)
                    })
                    .unwrap_or(""),
                selinux
                    .map(|f| {
                        f.message
                            .strip_prefix("SELinux mode: ")
                            .unwrap_or(&f.message)
                    })
                    .unwrap_or(""),
                &apparmor.map(|f| f.message.clone()).unwrap_or_default(),
            ])
            .set(1);

        let n = findings
            .iter()
            .find(|f| f.id == "linux_nftables")
            .and_then(|f| f.message.split_whitespace().next()?.parse::<i64>().ok())
            .unwrap_or(-1);
        self.security_nftables_rule_count.set(n);
    }

    pub fn update_probe(&self, service: &str, kind: ProbeKind, r: &ProbeResult) {
        self.probe_ok
            .with_label_values(&[service, kind.as_str()])
            .set(if r.ok { 1 } else { 0 });
        self.probe_duration_ms
            .with_label_values(&[service])
            .set(r.duration_ms as i64);
        if let Some(v) = r.first_token_ms {
            self.probe_first_token_ms
                .with_label_values(&[service])
                .set(v as i64);
        }
        if let Some(v) = r.tokens_in {
            self.probe_tokens_in
                .with_label_values(&[service])
                .set(v as i64);
        }
        if let Some(v) = r.tokens_out {
            self.probe_tokens_out
                .with_label_values(&[service])
                .set(v as i64);
        }
        self.probe_last_run_timestamp
            .with_label_values(&[service])
            .set(chrono::Utc::now().timestamp());
    }

    pub fn update_service_samples(&self, samples: &[ServiceSample]) {
        self.service_sample.reset();
        for ss in samples {
            for entry in &ss.entries {
                if let Some(n) = entry
                    .value
                    .as_i64()
                    .or_else(|| entry.value.as_f64().map(|f| f as i64))
                {
                    self.service_sample
                        .with_label_values(&[&ss.service, &entry.id])
                        .set(n);
                }
            }
        }
    }

    pub fn update_service_inventories(&self, inventories: &[ServiceInventory]) {
        self.service_inventory.reset();
        for si in inventories {
            for entry in &si.entries {
                let val = match &entry.value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    other => other.to_string(),
                };
                self.service_inventory
                    .with_label_values(&[&si.service, &entry.id, &val])
                    .set(1);
            }
        }
    }

    pub fn update_service_security(&self, findings: &[ServiceSecurity]) {
        self.service_security.reset();
        for sf in findings {
            for finding in &sf.findings {
                self.service_security
                    .with_label_values(&[&sf.service, &finding.id])
                    .set(if finding.pass { 1 } else { 0 });
            }
        }
    }
}

pub struct Metrics {
    registry: Registry,
    pub service_healthy: IntGaugeVec,
    pub service_upgrade_pending: IntGaugeVec,
    pub service_busy: IntGaugeVec,
    pub service_phase: IntGaugeVec,
    pub assessment: AssessmentMetrics,
    pub daemon_version: String,
    pub daemon_commit: String,
    pub started_at: std::time::Instant,
    pub heartbeat_last_success: IntGauge,
    pub heartbeat_total: IntCounterVec,
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let service_healthy = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_service_healthy",
                "Whether the service is healthy (1) or not (0)",
            ),
            &["service"],
        )
        .unwrap();

        let service_upgrade_pending = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_service_upgrade_pending",
                "Whether an upgrade is pending (1) or not (0)",
            ),
            &["service"],
        )
        .unwrap();

        let service_busy = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_service_busy",
                "Whether the service is busy (1) or not (0)",
            ),
            &["service"],
        )
        .unwrap();

        registry
            .register(Box::new(service_healthy.clone()))
            .unwrap();
        registry
            .register(Box::new(service_upgrade_pending.clone()))
            .unwrap();
        registry.register(Box::new(service_busy.clone())).unwrap();

        let service_phase = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_service_phase",
                "Service lifecycle phase (0=stopped, 1=starting, 2=healthy, 3=unhealthy)",
            ),
            &["service"],
        )
        .unwrap();
        registry.register(Box::new(service_phase.clone())).unwrap();

        let assessment = AssessmentMetrics::new(&registry);

        let heartbeat_last_success = IntGauge::new(
            "mac_mgmt_heartbeat_last_success_timestamp_seconds",
            "Unix timestamp of the last successful heartbeat",
        )
        .unwrap();
        let heartbeat_total = IntCounterVec::new(
            Opts::new(
                "mac_mgmt_heartbeat_total",
                "Total heartbeat attempts by result",
            ),
            &["result"],
        )
        .unwrap();
        registry
            .register(Box::new(heartbeat_last_success.clone()))
            .unwrap();
        registry
            .register(Box::new(heartbeat_total.clone()))
            .unwrap();

        let daemon_commit = env!("GIT_SHA").to_string();
        let build_info = IntGaugeVec::new(
            Opts::new(
                "mac_mgmt_build_info",
                "Daemon build metadata, always 1; version and commit on labels",
            ),
            &["version", "commit"],
        )
        .unwrap();
        build_info
            .with_label_values(&[env!("CARGO_PKG_VERSION"), &daemon_commit])
            .set(1);
        registry.register(Box::new(build_info)).unwrap();

        Metrics {
            registry,
            service_healthy,
            service_upgrade_pending,
            service_busy,
            service_phase,
            assessment,
            daemon_version: env!("CARGO_PKG_VERSION").to_string(),
            daemon_commit,
            started_at: std::time::Instant::now(),
            heartbeat_last_success,
            heartbeat_total,
        }
    }

    pub fn record_heartbeat_success(&self) {
        self.heartbeat_last_success
            .set(chrono::Utc::now().timestamp());
        self.heartbeat_total.with_label_values(&["success"]).inc();
    }

    pub fn record_heartbeat_failure(&self) {
        self.heartbeat_total.with_label_values(&["failure"]).inc();
    }

    pub fn register_collector(
        &self,
        collector: Box<dyn prometheus::core::Collector>,
    ) -> prometheus::Result<()> {
        self.registry.register(collector)
    }

    pub fn status(&self) -> (String, u64, Vec<(String, bool, bool, bool, &'static str)>) {
        let uptime = self.started_at.elapsed().as_secs();
        let families = self.registry.gather();
        let mut service_names: Vec<String> = Vec::new();

        for family in &families {
            if family.name() == "mac_mgmt_service_healthy" {
                for metric in family.get_metric() {
                    for label in metric.get_label() {
                        if label.name() == "service" {
                            service_names.push(label.value().to_string());
                        }
                    }
                }
            }
        }

        let services = service_names
            .iter()
            .map(|name| {
                let phase_int = self.service_phase.with_label_values(&[name]).get();
                let phase = match phase_int {
                    0 => "stopped",
                    1 => "starting",
                    2 => "healthy",
                    3 => "unhealthy",
                    _ => "unknown",
                };
                (
                    name.clone(),
                    self.service_healthy.with_label_values(&[name]).get() == 1,
                    self.service_upgrade_pending
                        .with_label_values(&[name])
                        .get()
                        == 1,
                    self.service_busy.with_label_values(&[name]).get() == 1,
                    phase,
                )
            })
            .collect();

        (self.daemon_version.clone(), uptime, services)
    }

    pub fn render(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}
