//! Dynamic per-heartbeat sample. Cheap in the steady state: one collection per
//! `SAMPLE_TTL` is shared by every caller in the process (see `collect`).
//!
//! GDPR allowlist: counters only, no per-process info beyond the total count,
//! no per-peer network data, no IPs, no usernames.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use sysinfo::{Disks, Networks, System};

use mac_mgmt_common::{DiskFree, DynamicSample};

/// Shared `sysinfo::System` so we don't pay the full refresh cost each heartbeat —
/// CPU load averages need two samples to compute a rate, and the sysinfo API
/// expects the same instance across calls.
fn shared_system() -> &'static Mutex<System> {
    static SYS: OnceLock<Mutex<System>> = OnceLock::new();
    SYS.get_or_init(|| Mutex::new(System::new()))
}

/// How long a collected sample is reused before another collection runs.
///
/// The collectors are host-level (identical for every caller in the process),
/// and they are not as cheap as the old <50ms assumption: refreshing the sysinfo
/// process table plus the disk list and spawning the GPU vendor CLIs routinely
/// takes hundreds of milliseconds, all of it serialized behind `shared_system`.
/// A process that hosts many simulated daemons (or simply doubles up on ticks)
/// otherwise runs one full collection per daemon per tick, and the resulting
/// queue behind that lock starves every daemon's event loop — including the
/// heartbeat send. One fresh sample per second is plenty for a metric that is
/// piggybacked onto a heartbeat.
const SAMPLE_TTL: Duration = Duration::from_secs(1);

/// Wall-clock budget for one collection.
///
/// The collectors usually finish in tens of milliseconds, but they enumerate
/// processes/disks and shell out to GPU vendor CLIs — a slow or wedged vendor
/// tool must not hold a heartbeat, the health tick, or the cached sample
/// hostage, so the caller waits at most this long and then reuses the cache.
const COLLECT_DEADLINE: Duration = Duration::from_secs(5);

fn sample_cache() -> &'static Mutex<Option<(Instant, DynamicSample)>> {
    static CACHE: OnceLock<Mutex<Option<(Instant, DynamicSample)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Serializes collections process-wide so concurrent callers queue behind one
/// collection instead of piling their own onto the global sysinfo lock.
fn collect_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// Cached sample from within `SAMPLE_TTL`, if there is one.
fn cached_sample() -> Option<DynamicSample> {
    let cache = sample_cache().lock().unwrap();
    cache.as_ref().and_then(|(at, sample)| {
        if at.elapsed() < SAMPLE_TTL {
            Some(sample.clone())
        } else {
            None
        }
    })
}

pub async fn collect() -> Result<DynamicSample> {
    // Fast path: a sample younger than the TTL is good enough for a heartbeat.
    if let Some(sample) = cached_sample() {
        return Ok(sample);
    }

    let _guard = collect_lock().lock().await;
    // Another caller may have refreshed while we waited for the lock.
    if let Some(sample) = cached_sample() {
        return Ok(sample);
    }

    let sample = match tokio::time::timeout(
        COLLECT_DEADLINE,
        tokio::task::spawn_blocking(collect_blocking),
    )
    .await
    {
        Ok(joined) => joined??,
        Err(_) => {
            tracing::warn!(
                "sample collection exceeded {COLLECT_DEADLINE:?}; reusing the last sample"
            );
            return match sample_cache().lock().unwrap().as_ref() {
                Some((_, sample)) => Ok(sample.clone()),
                None => Err(anyhow::anyhow!(
                    "sample collection timed out and no cached sample is available"
                )),
            };
        }
    };
    *sample_cache().lock().unwrap() = Some((Instant::now(), sample.clone()));
    Ok(sample)
}

fn collect_blocking() -> Result<DynamicSample> {
    let started = Instant::now();

    let (mem_used, mem_total, swap_used, process_count) = {
        let mut sys = shared_system().lock().unwrap();
        sys.refresh_memory();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, false);
        (
            sys.used_memory(),
            sys.total_memory(),
            sys.used_swap(),
            sys.processes().len() as u32,
        )
    };

    let cpu_load_1m = System::load_average().one as f32;

    let disk_free: Vec<DiskFree> = Disks::new_with_refreshed_list()
        .iter()
        .filter_map(|d| {
            let mount = d.mount_point().to_string_lossy().to_string();
            if !is_tracked_mount(&mount) {
                return None;
            }
            Some(DiskFree {
                mount,
                free_bytes: d.available_space(),
                total_bytes: d.total_space(),
            })
        })
        .collect();

    let (net_rx_bytes, net_tx_bytes) = Networks::new_with_refreshed_list()
        .iter()
        .filter(|(name, _)| *name != "lo" && *name != "lo0")
        .fold((0u64, 0u64), |(rx, tx), (_, data)| {
            (rx + data.total_received(), tx + data.total_transmitted())
        });

    let gpus = crate::assessment::gpu::sample();

    let elapsed = started.elapsed();
    if elapsed > Duration::from_millis(200) {
        tracing::debug!("sample collection took {:?} (>200ms)", elapsed);
    }

    Ok(DynamicSample {
        cpu_load_1m,
        mem_used_bytes: mem_used,
        mem_total_bytes: mem_total,
        swap_used_bytes: swap_used,
        disk_free,
        net_rx_bytes,
        net_tx_bytes,
        process_count,
        thermal_state: thermal_state(),
        gpus,
    })
}

fn is_tracked_mount(mount: &str) -> bool {
    mount == "/" || mount == "/nix/store" || mount == "/nix" || mount == "/System/Volumes/Data"
}

#[cfg(target_os = "macos")]
fn thermal_state() -> Option<String> {
    // `pmset -g therm` prints lines like "CPU_Scheduler_Limit = 100" when nominal,
    // and drops that to <100 under thermal pressure. Cheap enough for 60s cadence.
    let out = std::process::Command::new("pmset")
        .args(["-g", "therm"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        if let Some((_, v)) = line.split_once('=') {
            if let Ok(limit) = v.trim().parse::<u32>() {
                return Some(match limit {
                    100 => "nominal".into(),
                    75..=99 => "fair".into(),
                    40..=74 => "serious".into(),
                    _ => "critical".into(),
                });
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn thermal_state() -> Option<String> {
    // Read /sys/class/thermal/thermal_zone*/temp (millidegrees C). Take the max.
    let dir = std::fs::read_dir("/sys/class/thermal").ok()?;
    let mut max_mc: i32 = 0;
    for entry in dir.flatten() {
        let path = entry.path().join("temp");
        if let Ok(s) = std::fs::read_to_string(&path) {
            if let Ok(v) = s.trim().parse::<i32>() {
                if v > max_mc {
                    max_mc = v;
                }
            }
        }
    }
    if max_mc == 0 {
        return None;
    }
    let c = max_mc / 1000;
    Some(
        match c {
            ..=65 => "nominal",
            66..=80 => "fair",
            81..=95 => "serious",
            _ => "critical",
        }
        .into(),
    )
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn thermal_state() -> Option<String> {
    None
}
