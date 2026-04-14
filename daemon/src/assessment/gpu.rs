//! GPU inventory + per-sample state.
//!
//! Shells out to vendor tools rather than linking to NVML/ROCm-SMI directly
//! so we stay dependency-free and work across all four vendors we care about
//! (NVIDIA, AMD, Apple Silicon, Intel/generic).
//!
//! Each collector runs in a best-effort pipeline — a missing binary or
//! unknown model leaves the corresponding entry empty, never failing the
//! surrounding assessment.

use std::process::Command;

use mac_mgmt_common::{GpuInfo, GpuSample};

/// Collect static GPU inventory. Called at the 6h inventory cadence.
pub fn inventory() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();
    gpus.extend(nvidia_inventory());
    gpus.extend(amd_inventory());
    #[cfg(target_os = "macos")]
    gpus.extend(apple_inventory());
    // If no vendor tool reported anything, try lspci as a last-resort catalog.
    if gpus.is_empty() {
        gpus.extend(lspci_fallback());
    }
    reindex(gpus)
}

/// Collect dynamic GPU state. Called every heartbeat (~60s) — should be cheap.
pub fn sample() -> Vec<GpuSample> {
    let mut samples = Vec::new();
    samples.extend(nvidia_sample());
    samples.extend(amd_sample());
    // macOS powermetrics requires root; we leave state None on macOS for now.
    reindex_samples(samples)
}

/// Ensure indices are 0-based and monotonic across the merged vendor list.
fn reindex(mut gpus: Vec<GpuInfo>) -> Vec<GpuInfo> {
    for (i, g) in gpus.iter_mut().enumerate() {
        g.index = i as u32;
    }
    gpus
}

fn reindex_samples(mut samples: Vec<GpuSample>) -> Vec<GpuSample> {
    for (i, s) in samples.iter_mut().enumerate() {
        s.index = i as u32;
    }
    samples
}

// ── NVIDIA ─────────────────────────────────────────────────────────────

fn nvidia_inventory() -> Vec<GpuInfo> {
    let out = match Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,driver_version,pci.bus_id",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() < 4 {
                return None;
            }
            let vram_mib: u64 = parts[1].parse().ok()?;
            Some(GpuInfo {
                index: 0, // reassigned by reindex
                vendor: "nvidia".into(),
                name: parts[0].to_string(),
                vram_total_bytes: vram_mib * 1024 * 1024,
                driver_version: Some(parts[2].to_string()),
                pci_bus_id: Some(parts[3].to_string()),
            })
        })
        .collect()
}

fn nvidia_sample() -> Vec<GpuSample> {
    let out = match Command::new("nvidia-smi")
        .args([
            "--query-gpu=utilization.gpu,memory.used,temperature.gpu,power.draw",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|line| {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            let parse_u8 = |s: &&str| s.parse::<u8>().ok();
            let parse_u64 = |s: &&str| s.parse::<u64>().ok();
            let parse_i32 = |s: &&str| s.parse::<i32>().ok();
            let parse_f32 = |s: &&str| s.parse::<f32>().ok();
            GpuSample {
                index: 0,
                utilization_pct: parts.first().and_then(parse_u8),
                vram_used_bytes: parts.get(1).and_then(parse_u64).map(|m| m * 1024 * 1024),
                temperature_c: parts.get(2).and_then(parse_i32),
                power_watts: parts.get(3).and_then(parse_f32),
            }
        })
        .collect()
}

// ── AMD ROCm ───────────────────────────────────────────────────────────

fn amd_inventory() -> Vec<GpuInfo> {
    let out = match Command::new("rocm-smi")
        .args([
            "--showproductname",
            "--showmeminfo",
            "vram",
            "--showdriverversion",
            "--showbus",
            "--json",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    parse_rocm_json_inventory(&String::from_utf8_lossy(&out.stdout))
}

fn amd_sample() -> Vec<GpuSample> {
    let out = match Command::new("rocm-smi")
        .args([
            "--showuse",
            "--showmeminfo",
            "vram",
            "--showtemp",
            "--showpower",
            "--json",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    parse_rocm_json_sample(&String::from_utf8_lossy(&out.stdout))
}

/// Parse rocm-smi `--json` inventory output. Schema is `{"card0": {...}, ...}`.
fn parse_rocm_json_inventory(json: &str) -> Vec<GpuInfo> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let Some(obj) = v.as_object() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (_card, fields) in obj {
        let name = fields
            .get("Card series")
            .or_else(|| fields.get("Card Series"))
            .or_else(|| fields.get("Card model"))
            .and_then(|v| v.as_str())
            .unwrap_or("AMD GPU")
            .to_string();
        let vram_total = fields
            .get("VRAM Total Memory (B)")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let driver_version = fields
            .get("Driver version")
            .and_then(|v| v.as_str())
            .map(String::from);
        let pci_bus_id = fields.get("PCI Bus").and_then(|v| v.as_str()).map(String::from);
        out.push(GpuInfo {
            index: 0,
            vendor: "amd".into(),
            name,
            vram_total_bytes: vram_total,
            driver_version,
            pci_bus_id,
        });
    }
    out
}

fn parse_rocm_json_sample(json: &str) -> Vec<GpuSample> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let Some(obj) = v.as_object() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (_card, fields) in obj {
        let utilization_pct = fields
            .get("GPU use (%)")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u8>().ok());
        let vram_used_bytes = fields
            .get("VRAM Total Used Memory (B)")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok());
        // rocm-smi emits "Temperature (Sensor edge) (C)" etc; pick the first temp field.
        let temperature_c = fields
            .as_object()
            .into_iter()
            .flatten()
            .find_map(|(k, v): (&String, &serde_json::Value)| {
                if k.starts_with("Temperature (Sensor") && k.ends_with("(C)") {
                    v.as_str()
                        .and_then(|s| s.parse::<f32>().ok())
                        .map(|f| f as i32)
                } else {
                    None
                }
            });
        let power_watts = fields
            .get("Average Graphics Package Power (W)")
            .or_else(|| fields.get("Current Socket Graphics Package Power (W)"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f32>().ok());
        out.push(GpuSample {
            index: 0,
            utilization_pct,
            vram_used_bytes,
            temperature_c,
            power_watts,
        });
    }
    out
}

// ── macOS (Apple Silicon + discrete) ───────────────────────────────────

#[cfg(target_os = "macos")]
fn apple_inventory() -> Vec<GpuInfo> {
    let out = match Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let arr = match v.get("SPDisplaysDataType").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };
    let mut out_vec = Vec::new();
    for entry in arr {
        let name = entry
            .get("sppci_model")
            .and_then(|v| v.as_str())
            .unwrap_or("Apple GPU")
            .to_string();
        let vendor = if name.contains("Apple") {
            "apple"
        } else if name.to_lowercase().contains("amd") || name.to_lowercase().contains("radeon") {
            "amd"
        } else if name.to_lowercase().contains("intel") {
            "intel"
        } else {
            "unknown"
        };
        // VRAM is reported as a string like "8 GB" for discrete, omitted for unified
        // memory Apple Silicon (shares with system RAM). Parse best-effort.
        let vram_total_bytes = entry
            .get("spdisplays_vram")
            .or_else(|| entry.get("spdisplays_vram_shared"))
            .and_then(|v| v.as_str())
            .and_then(parse_macos_memory)
            .unwrap_or(0);
        let pci_bus_id = entry
            .get("sppci_bus")
            .and_then(|v| v.as_str())
            .map(String::from);
        out_vec.push(GpuInfo {
            index: 0,
            vendor: vendor.into(),
            name,
            vram_total_bytes,
            driver_version: None,
            pci_bus_id,
        });
    }
    out_vec
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn apple_inventory() -> Vec<GpuInfo> {
    Vec::new()
}

#[cfg(target_os = "macos")]
fn parse_macos_memory(s: &str) -> Option<u64> {
    // Examples: "8 GB", "1024 MB", "24576 MB"
    let s = s.trim();
    let (num, unit) = s.rsplit_once(' ')?;
    let n: u64 = num.parse().ok()?;
    match unit.to_uppercase().as_str() {
        "KB" => Some(n * 1024),
        "MB" => Some(n * 1024 * 1024),
        "GB" => Some(n * 1024 * 1024 * 1024),
        "TB" => Some(n * 1024 * 1024 * 1024 * 1024),
        _ => None,
    }
}

// ── Linux fallback ─────────────────────────────────────────────────────

fn lspci_fallback() -> Vec<GpuInfo> {
    // `lspci -mmn -d ::0300` lists display controllers in machine-readable form.
    // Fields: slot "vendor" "device" rev-progif "subsys_vendor" "subsys_device".
    let out = match Command::new("lspci")
        .args(["-mmn", "-d", "::0300"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut gpus = Vec::new();
    for line in text.lines() {
        let fields = split_lspci(line);
        if fields.len() < 3 {
            continue;
        }
        let slot = fields[0].clone();
        let vendor_id = fields[1].trim_matches('"').to_lowercase();
        let device_id = fields[2].trim_matches('"');
        let vendor = match vendor_id.as_str() {
            "10de" => "nvidia",
            "1002" | "1022" => "amd",
            "8086" => "intel",
            "106b" => "apple",
            _ => "unknown",
        };
        gpus.push(GpuInfo {
            index: 0,
            vendor: vendor.into(),
            name: format!("{vendor_id}:{device_id}"),
            vram_total_bytes: 0,
            driver_version: None,
            pci_bus_id: Some(slot),
        });
    }
    gpus
}

/// Split an lspci line that mixes bare tokens with quoted strings.
/// Example: `01:00.0 "0300" "10de" "2204" -r00 "1462" "394e"`.
fn split_lspci(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for c in line.chars() {
        match (c, in_quote) {
            ('"', _) => {
                in_quote = !in_quote;
                cur.push(c);
            }
            (' ', false) => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lspci_split_handles_quoted_fields() {
        let line = r#"01:00.0 "0300" "10de" "2204" -r00 "1462" "394e""#;
        let fields = split_lspci(line);
        assert_eq!(fields[0], "01:00.0");
        assert_eq!(fields[1], "\"0300\"");
        assert_eq!(fields[2], "\"10de\"");
    }

    #[test]
    fn rocm_json_inventory_smoke() {
        // Matches the documented field names rocm-smi emits.
        let json = r#"{"card0":{
            "Card series": "Radeon RX 6700 XT",
            "VRAM Total Memory (B)": "12884901888",
            "Driver version": "5.7.1",
            "PCI Bus": "0000:03:00.0"
        }}"#;
        let gpus = parse_rocm_json_inventory(json);
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].name, "Radeon RX 6700 XT");
        assert_eq!(gpus[0].vram_total_bytes, 12_884_901_888);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_memory_parse() {
        assert_eq!(parse_macos_memory("8 GB"), Some(8 * 1024 * 1024 * 1024));
        assert_eq!(parse_macos_memory("1024 MB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_macos_memory("nope"), None);
    }
}
