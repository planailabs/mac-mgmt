//! Host-level failure signals derived from the heartbeat's [`DynamicSample`].
//!
//! These are the resource-pressure alerts (disk, load, thermal). Drift signals
//! come from the service supervisor instead — see
//! [`crate::service_mgmt::ServiceManager::drift_signals`].
//!
//! Everything here is pure: given a sample and thresholds, produce signals.
//! The daemon calls [`evaluate_sample_signals`] each heartbeat and merges the
//! result with drift signals before sending.

use mac_mgmt_common::{DynamicSample, FailureSignal, SignalSeverity};

/// Thresholds for turning a raw sample into signals. Defaults are conservative
/// so we only alert on genuine trouble.
#[derive(Debug, Clone, Copy)]
pub struct SignalThresholds {
    /// Free-space fraction (0.0–1.0) at/below which a mount is CRITICAL.
    pub disk_free_critical_pct: f64,
    /// Free-space fraction at/below which a mount is a WARNING.
    pub disk_free_warning_pct: f64,
    /// `cpu_load_1m / cores` at/above which load is CRITICAL.
    pub load_per_core_critical: f64,
    /// `cpu_load_1m / cores` at/above which load is a WARNING.
    pub load_per_core_warning: f64,
}

impl Default for SignalThresholds {
    fn default() -> Self {
        // ponytail: fixed defaults; lift into daemon config if a site needs to tune them.
        Self {
            disk_free_critical_pct: 0.05,
            disk_free_warning_pct: 0.10,
            load_per_core_critical: 4.0,
            load_per_core_warning: 2.0,
        }
    }
}

/// Number of logical cores, for load-per-core scaling. Falls back to 1 so the
/// threshold never divides by zero on exotic platforms.
fn core_count() -> f64 {
    std::thread::available_parallelism()
        .map(|n| n.get() as f64)
        .unwrap_or(1.0)
}

/// Derive resource-pressure signals from a single dynamic sample.
pub fn evaluate_sample_signals(
    sample: &DynamicSample,
    thresholds: &SignalThresholds,
    now: i64,
) -> Vec<FailureSignal> {
    let mut signals = Vec::new();

    // ── Disk ────────────────────────────────────────────────────────────
    for d in &sample.disk_free {
        if d.total_bytes == 0 {
            continue;
        }
        let free_frac = d.free_bytes as f64 / d.total_bytes as f64;
        let severity = if free_frac <= thresholds.disk_free_critical_pct {
            Some(SignalSeverity::Critical)
        } else if free_frac <= thresholds.disk_free_warning_pct {
            Some(SignalSeverity::Warning)
        } else {
            None
        };
        if let Some(severity) = severity {
            signals.push(FailureSignal {
                kind: "disk_low".to_string(),
                severity,
                subject: d.mount.clone(),
                message: format!(
                    "{} has {:.1}% free ({} of {})",
                    d.mount,
                    free_frac * 100.0,
                    human_bytes(d.free_bytes),
                    human_bytes(d.total_bytes),
                ),
                since: now,
            });
        }
    }

    // ── Load ────────────────────────────────────────────────────────────
    let cores = core_count();
    let per_core = sample.cpu_load_1m as f64 / cores;
    let load_severity = if per_core >= thresholds.load_per_core_critical {
        Some(SignalSeverity::Critical)
    } else if per_core >= thresholds.load_per_core_warning {
        Some(SignalSeverity::Warning)
    } else {
        None
    };
    if let Some(severity) = load_severity {
        signals.push(FailureSignal {
            kind: "load_high".to_string(),
            severity,
            subject: "system".to_string(),
            message: format!(
                "1m load {:.2} over {:.0} core(s) = {:.2}/core",
                sample.cpu_load_1m, cores, per_core
            ),
            since: now,
        });
    }

    // ── Thermal ─────────────────────────────────────────────────────────
    if let Some(state) = &sample.thermal_state {
        let lower = state.to_ascii_lowercase();
        let severity = match lower.as_str() {
            "critical" => Some(SignalSeverity::Critical),
            "serious" => Some(SignalSeverity::Warning),
            _ => None,
        };
        if let Some(severity) = severity {
            signals.push(FailureSignal {
                kind: "thermal_critical".to_string(),
                severity,
                subject: "system".to_string(),
                message: format!("thermal pressure: {state}"),
                since: now,
            });
        }
    }

    signals
}

/// Compact human-readable byte size for signal messages.
fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut val = bytes as f64;
    let mut unit = 0;
    while val >= 1024.0 && unit < UNITS.len() - 1 {
        val /= 1024.0;
        unit += 1;
    }
    format!("{val:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;
    use mac_mgmt_common::DiskFree;

    fn sample() -> DynamicSample {
        DynamicSample::default()
    }

    fn kinds(sigs: &[FailureSignal]) -> Vec<(&str, SignalSeverity)> {
        sigs.iter().map(|s| (s.kind.as_str(), s.severity)).collect()
    }

    #[test]
    fn disk_thresholds() {
        let t = SignalThresholds::default();
        let mut s = sample();
        // 3% free → critical
        s.disk_free = vec![DiskFree {
            mount: "/".into(),
            free_bytes: 3,
            total_bytes: 100,
        }];
        assert_eq!(
            kinds(&evaluate_sample_signals(&s, &t, 0)),
            vec![("disk_low", SignalSeverity::Critical)]
        );

        // 8% free → warning
        s.disk_free = vec![DiskFree {
            mount: "/".into(),
            free_bytes: 8,
            total_bytes: 100,
        }];
        assert_eq!(
            kinds(&evaluate_sample_signals(&s, &t, 0)),
            vec![("disk_low", SignalSeverity::Warning)]
        );

        // 50% free → nothing
        s.disk_free = vec![DiskFree {
            mount: "/".into(),
            free_bytes: 50,
            total_bytes: 100,
        }];
        assert!(evaluate_sample_signals(&s, &t, 0).is_empty());

        // zero-total mount is skipped, not a divide-by-zero
        s.disk_free = vec![DiskFree {
            mount: "/x".into(),
            free_bytes: 0,
            total_bytes: 0,
        }];
        assert!(evaluate_sample_signals(&s, &t, 0).is_empty());
    }

    #[test]
    fn load_scales_with_cores() {
        let cores = core_count() as f32;
        let t = SignalThresholds::default();
        let mut s = sample();

        // Below warning: 1x/core.
        s.cpu_load_1m = cores * 1.0;
        assert!(evaluate_sample_signals(&s, &t, 0).is_empty());

        // Warning band: 2x/core.
        s.cpu_load_1m = cores * 2.0;
        assert_eq!(
            kinds(&evaluate_sample_signals(&s, &t, 0)),
            vec![("load_high", SignalSeverity::Warning)]
        );

        // Critical: 4x/core.
        s.cpu_load_1m = cores * 4.0;
        assert_eq!(
            kinds(&evaluate_sample_signals(&s, &t, 0)),
            vec![("load_high", SignalSeverity::Critical)]
        );
    }

    #[test]
    fn thermal_maps_state_to_severity() {
        let t = SignalThresholds::default();
        let mut s = sample();

        s.thermal_state = Some("nominal".into());
        assert!(evaluate_sample_signals(&s, &t, 0).is_empty());

        s.thermal_state = Some("Serious".into());
        assert_eq!(
            kinds(&evaluate_sample_signals(&s, &t, 0)),
            vec![("thermal_critical", SignalSeverity::Warning)]
        );

        s.thermal_state = Some("CRITICAL".into());
        assert_eq!(
            kinds(&evaluate_sample_signals(&s, &t, 0)),
            vec![("thermal_critical", SignalSeverity::Critical)]
        );
    }

    #[test]
    fn since_is_stamped() {
        let t = SignalThresholds::default();
        let mut s = sample();
        s.cpu_load_1m = core_count() as f32 * 5.0;
        let sigs = evaluate_sample_signals(&s, &t, 12345);
        assert_eq!(sigs[0].since, 12345);
    }
}
