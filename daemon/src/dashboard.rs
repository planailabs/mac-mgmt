//! Local status dashboard rendered via Mustache (ramhorns).
//!
//! Serves a lightweight HTML page at `GET /dashboard` on the daemon's
//! metrics server. The page mirrors the design language of the management
//! server's fleet detail view (same color palette, badge styles, table
//! layout) but runs entirely locally with no JavaScript dependencies.

use std::sync::Arc;

use ramhorns::Content;

use crate::assessment::Assessor;
use crate::metrics::Metrics;

static TEMPLATE: &str = include_str!("dashboard.html");

// ── Mustache data structs ────────────────────────────────────────────

#[derive(Content)]
struct Dashboard {
    version: String,
    uptime: String,
    hostname: String,

    has_services: bool,
    services: Vec<ServiceRow>,

    has_probes: bool,
    probes: Vec<ProbeRow>,

    has_sample: bool,
    cpu_load: String,
    mem_used: String,
    mem_total: String,
    mem_pct: String,
    swap_used: String,
    process_count: String,
    thermal_state: String,
    disks: Vec<DiskRow>,
    has_disks: bool,
    gpus: Vec<GpuRow>,
    has_gpus: bool,
    net_rx: String,
    net_tx: String,
}

#[derive(Content)]
struct ServiceRow {
    name: String,
    healthy: bool,
    unhealthy: bool,
    upgrade_pending: bool,
    busy: bool,
}

#[derive(Content)]
struct ProbeRow {
    service: String,
    kind: String,
    ok: bool,
    failed: bool,
    duration: String,
    last_run: String,
}

#[derive(Content)]
struct DiskRow {
    mount: String,
    free: String,
    total: String,
    used_pct: String,
}

#[derive(Content)]
struct GpuRow {
    index: String,
    name: String,
    vram_used: String,
    vram_total: String,
    utilization: String,
    temperature: String,
    power: String,
}

// ── Rendering ────────────────────────────────────────────────────────

pub fn render(metrics: &Arc<Metrics>, assessor: &Arc<Assessor>) -> String {
    let tpl = ramhorns::Template::new(TEMPLATE).expect("dashboard template parse");

    let (version, uptime_secs, svc_list) = metrics.status();
    let uptime = humantime::format_duration(std::time::Duration::from_secs(uptime_secs)).to_string();
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".into());

    let services: Vec<ServiceRow> = svc_list
        .into_iter()
        .map(|(name, healthy, upgrade_pending, busy, _phase)| ServiceRow {
            name,
            healthy,
            unhealthy: !healthy,
            upgrade_pending,
            busy,
        })
        .collect();

    let probe_states = assessor.latest_probes_snapshot();
    let probes: Vec<ProbeRow> = probe_states
        .into_iter()
        .map(|p| {
            let last_run = p
                .last_probe_at
                .map(|ts| {
                    chrono::DateTime::from_timestamp(ts, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                        .unwrap_or_else(|| "—".into())
                })
                .unwrap_or_else(|| "never".into());
            let duration = p
                .last_probe_duration_ms
                .map(|ms| format!("{ms} ms"))
                .unwrap_or_else(|| "—".into());
            let ok = p.last_probe_ok.unwrap_or(false);
            ProbeRow {
                service: p.name,
                kind: p.last_probe_kind.unwrap_or_else(|| "—".into()),
                ok,
                failed: !ok,
                duration,
                last_run,
            }
        })
        .collect();

    let sample = assessor.latest_sample_snapshot();
    let (has_sample, cpu_load, mem_used, mem_total, mem_pct, swap_used, process_count, thermal_state, disks, gpus, net_rx, net_tx) =
        if let Some(s) = sample {
            let mem_pct = if s.mem_total_bytes > 0 {
                format!("{}%", s.mem_used_bytes * 100 / s.mem_total_bytes)
            } else {
                "—".into()
            };
            let disk_rows: Vec<DiskRow> = s
                .disk_free
                .iter()
                .map(|d| {
                    let used_pct = if d.total_bytes > 0 {
                        format!(
                            "{}%",
                            ((d.total_bytes - d.free_bytes) * 100 / d.total_bytes).min(100)
                        )
                    } else {
                        "—".into()
                    };
                    DiskRow {
                        mount: d.mount.clone(),
                        free: human_bytes(d.free_bytes),
                        total: human_bytes(d.total_bytes),
                        used_pct,
                    }
                })
                .collect();
            let gpu_rows: Vec<GpuRow> = s
                .gpus
                .iter()
                .map(|g| GpuRow {
                    index: g.index.to_string(),
                    name: String::new(), // name comes from inventory, not sample
                    vram_used: g
                        .vram_used_bytes
                        .map(human_bytes)
                        .unwrap_or_else(|| "—".into()),
                    vram_total: String::new(),
                    utilization: g
                        .utilization_pct
                        .map(|v| format!("{v}%"))
                        .unwrap_or_else(|| "—".into()),
                    temperature: g
                        .temperature_c
                        .map(|v| format!("{v} °C"))
                        .unwrap_or_else(|| "—".into()),
                    power: g
                        .power_watts
                        .map(|v| format!("{v:.0} W"))
                        .unwrap_or_else(|| "—".into()),
                })
                .collect();
            (
                true,
                format!("{:.2}", s.cpu_load_1m),
                human_bytes(s.mem_used_bytes),
                human_bytes(s.mem_total_bytes),
                mem_pct,
                human_bytes(s.swap_used_bytes),
                s.process_count.to_string(),
                s.thermal_state
                    .clone()
                    .unwrap_or_else(|| "nominal".into()),
                disk_rows,
                gpu_rows,
                human_bytes(s.net_rx_bytes),
                human_bytes(s.net_tx_bytes),
            )
        } else {
            (
                false,
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                Vec::new(),
                Vec::new(),
                String::new(),
                String::new(),
            )
        };

    let data = Dashboard {
        version,
        uptime,
        hostname,
        has_services: !services.is_empty(),
        services,
        has_probes: !probes.is_empty(),
        probes,
        has_sample,
        cpu_load,
        mem_used,
        mem_total,
        mem_pct,
        swap_used,
        process_count,
        thermal_state,
        has_disks: !disks.is_empty(),
        disks,
        has_gpus: !gpus.is_empty(),
        gpus,
        net_rx,
        net_tx,
    };

    tpl.render(&data)
}

fn human_bytes(b: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;
    if b >= TIB {
        format!("{:.1} TiB", b as f64 / TIB as f64)
    } else if b >= GIB {
        format!("{:.1} GiB", b as f64 / GIB as f64)
    } else if b >= MIB {
        format!("{:.1} MiB", b as f64 / MIB as f64)
    } else if b >= KIB {
        format!("{:.1} KiB", b as f64 / KIB as f64)
    } else {
        format!("{b} B")
    }
}
