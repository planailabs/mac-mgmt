//! GPU inventory + per-sample state.
//!
//! Shells out to vendor tools rather than linking to NVML/ROCm-SMI directly
//! so we stay dependency-free and work across all four vendors we care about
//! (NVIDIA, AMD, Apple Silicon, Intel/generic).
//!
//! Each collector runs in a best-effort pipeline — a missing binary or
//! unknown model leaves the corresponding entry empty, never failing the
//! surrounding assessment. Anything unexpected (non-zero exit, unparseable
//! output, missing required fields) is forwarded to Sentry with enough
//! context to debug without a matching test rig.

use std::io;
use std::process::{Command, Output};

use mac_mgmt_common::{GpuInfo, GpuSample};

use crate::sentry_ext;

/// Outcome of attempting to run a vendor CLI.
enum ToolRun {
    /// Binary wasn't installed. This is normal on hosts without the vendor —
    /// do not report.
    NotInstalled,
    /// Binary ran and exited 0. `stdout` is captured even if empty.
    Ok(String),
    /// Binary ran but exited non-zero, or failed to spawn for reasons other
    /// than ENOENT. Already reported to Sentry by the time we return.
    Failed,
}

/// Run a vendor CLI and classify the outcome. Captures non-ENOENT spawn
/// failures and non-zero exits to Sentry with stderr context.
fn run_tool(tool: &str, args: &[&str]) -> ToolRun {
    match Command::new(tool).args(args).output() {
        Ok(output) => classify_output(tool, args, output),
        Err(e) if e.kind() == io::ErrorKind::NotFound => ToolRun::NotInstalled,
        Err(e) => {
            sentry_ext::capture_error(
                &format!("gpu: failed to spawn {tool}"),
                &[
                    ("tool", tool),
                    ("args", &args.join(" ")),
                    ("error", &e.to_string()),
                ],
            );
            ToolRun::Failed
        }
    }
}

fn classify_output(tool: &str, args: &[&str], output: Output) -> ToolRun {
    if output.status.success() {
        return ToolRun::Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    sentry_ext::capture_cmd_failure(
        &format!("{tool} {}", args.join(" ")),
        output.status.code(),
        stderr.trim(),
    );
    ToolRun::Failed
}

/// Truncate a stdout sample so the Sentry payload doesn't blow past the
/// event size limit — 1 KB is plenty to diagnose a format drift.
fn stdout_sample(s: &str) -> String {
    const MAX: usize = 1024;
    if s.len() <= MAX {
        s.to_string()
    } else {
        format!("{}… [{} bytes truncated]", &s[..MAX], s.len() - MAX)
    }
}

/// Report a parse failure with a sample of the raw output so we can fix
/// the parser without needing the original hardware.
fn report_parse_failure(tool: &str, stage: &str, stdout: &str, detail: &str) {
    sentry_ext::capture_error(
        &format!("gpu: failed to parse {tool} output ({stage})"),
        &[
            ("tool", tool),
            ("stage", stage),
            ("detail", detail),
            ("stdout_sample", &stdout_sample(stdout)),
        ],
    );
}

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
    let args = [
        "--query-gpu=name,memory.total,driver_version,pci.bus_id",
        "--format=csv,noheader,nounits",
    ];
    let stdout = match run_tool("nvidia-smi", &args) {
        ToolRun::Ok(s) => s,
        ToolRun::NotInstalled | ToolRun::Failed => return Vec::new(),
    };
    let mut gpus = Vec::new();
    let mut skipped = 0usize;
    for (lineno, line) in stdout.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 4 {
            skipped += 1;
            report_parse_failure(
                "nvidia-smi",
                "inventory/field-count",
                &stdout,
                &format!("line {lineno}: expected 4 fields, got {}", parts.len()),
            );
            continue;
        }
        let vram_mib = match parts[1].parse::<u64>() {
            Ok(v) => v,
            Err(e) => {
                skipped += 1;
                report_parse_failure(
                    "nvidia-smi",
                    "inventory/memory-total",
                    &stdout,
                    &format!(
                        "line {lineno}: can't parse memory.total {:?}: {e}",
                        parts[1]
                    ),
                );
                continue;
            }
        };
        gpus.push(GpuInfo {
            index: 0, // reassigned by reindex
            vendor: "nvidia".into(),
            name: parts[0].to_string(),
            vram_total_bytes: vram_mib * 1024 * 1024,
            driver_version: Some(parts[2].to_string()),
            pci_bus_id: Some(parts[3].to_string()),
        });
    }
    if gpus.is_empty() && skipped == 0 {
        // nvidia-smi ran cleanly but returned zero rows — on a box with
        // nvidia-smi installed, that's surprising; could indicate driver
        // mismatch or NVIDIA_VISIBLE_DEVICES scoping.
        sentry_ext::breadcrumb(
            "assessment.gpu",
            "nvidia-smi returned no rows",
            &[("stdout_len", &stdout.len().to_string())],
        );
    }
    gpus
}

fn nvidia_sample() -> Vec<GpuSample> {
    let args = [
        "--query-gpu=utilization.gpu,memory.used,temperature.gpu,power.draw",
        "--format=csv,noheader,nounits",
    ];
    let stdout = match run_tool("nvidia-smi", &args) {
        ToolRun::Ok(s) => s,
        ToolRun::NotInstalled | ToolRun::Failed => return Vec::new(),
    };
    let mut samples = Vec::new();
    for (lineno, line) in stdout.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 4 {
            // Different field count than inventory — report once so we
            // learn about driver versions that trim columns.
            report_parse_failure(
                "nvidia-smi",
                "sample/field-count",
                &stdout,
                &format!("line {lineno}: expected 4 fields, got {}", parts.len()),
            );
        }
        // "[N/A]" is what nvidia-smi emits when a sensor is unavailable
        // (common for power on some consumer cards). Silent.
        let na = |s: &str| matches!(s, "[N/A]" | "[Not Supported]");
        let parse_u8 = |s: &str| if na(s) { None } else { s.parse::<u8>().ok() };
        let parse_u64 = |s: &str| if na(s) { None } else { s.parse::<u64>().ok() };
        let parse_i32 = |s: &str| if na(s) { None } else { s.parse::<i32>().ok() };
        let parse_f32 = |s: &str| if na(s) { None } else { s.parse::<f32>().ok() };
        samples.push(GpuSample {
            index: 0,
            utilization_pct: parts.first().copied().and_then(parse_u8),
            vram_used_bytes: parts
                .get(1)
                .copied()
                .and_then(parse_u64)
                .map(|m| m * 1024 * 1024),
            temperature_c: parts.get(2).copied().and_then(parse_i32),
            power_watts: parts.get(3).copied().and_then(parse_f32),
        });
    }
    samples
}

// ── AMD ROCm ───────────────────────────────────────────────────────────

fn amd_inventory() -> Vec<GpuInfo> {
    let args = [
        "--showproductname",
        "--showmeminfo",
        "vram",
        "--showdriverversion",
        "--showbus",
        "--json",
    ];
    let stdout = match run_tool("rocm-smi", &args) {
        ToolRun::Ok(s) => s,
        ToolRun::NotInstalled | ToolRun::Failed => return Vec::new(),
    };
    match parse_rocm_json_inventory_checked(&stdout) {
        Ok(gpus) => gpus,
        Err(e) => {
            report_parse_failure("rocm-smi", "inventory/json", &stdout, &e);
            Vec::new()
        }
    }
}

fn amd_sample() -> Vec<GpuSample> {
    let args = [
        "--showuse",
        "--showmeminfo",
        "vram",
        "--showtemp",
        "--showpower",
        "--json",
    ];
    let stdout = match run_tool("rocm-smi", &args) {
        ToolRun::Ok(s) => s,
        ToolRun::NotInstalled | ToolRun::Failed => return Vec::new(),
    };
    match parse_rocm_json_sample_checked(&stdout) {
        Ok(samples) => samples,
        Err(e) => {
            report_parse_failure("rocm-smi", "sample/json", &stdout, &e);
            Vec::new()
        }
    }
}

/// Parse rocm-smi `--json` inventory output. Schema is `{"card0": {...}, ...}`.
/// Infallible wrapper kept for unit tests.
fn parse_rocm_json_inventory(json: &str) -> Vec<GpuInfo> {
    parse_rocm_json_inventory_checked(json).unwrap_or_default()
}

fn parse_rocm_json_inventory_checked(json: &str) -> Result<Vec<GpuInfo>, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let obj = v
        .as_object()
        .ok_or_else(|| format!("expected JSON object, got {:?}", type_of(&v)))?;
    let mut out = Vec::new();
    let mut missing_name = 0usize;
    for (card, fields) in obj {
        let explicit_name = fields
            .get("Card series")
            .or_else(|| fields.get("Card Series"))
            .or_else(|| fields.get("Card model"))
            .and_then(|v| v.as_str());
        if explicit_name.is_none() {
            missing_name += 1;
        }
        let name = explicit_name.unwrap_or("AMD GPU").to_string();
        let vram_total = fields
            .get("VRAM Total Memory (B)")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let driver_version = fields
            .get("Driver version")
            .and_then(|v| v.as_str())
            .map(String::from);
        let pci_bus_id = fields
            .get("PCI Bus")
            .and_then(|v| v.as_str())
            .map(String::from);
        tracing::debug!("rocm-smi parsed {card}: vram={vram_total}B driver={driver_version:?}");
        out.push(GpuInfo {
            index: 0,
            vendor: "amd".into(),
            name,
            vram_total_bytes: vram_total,
            driver_version,
            pci_bus_id,
        });
    }
    if !out.is_empty() && missing_name == out.len() {
        // Every card came back without a recognised name field — the
        // schema has drifted. Tell us so we can update the key list.
        sentry_ext::breadcrumb(
            "assessment.gpu",
            "rocm-smi: no card-name field recognised",
            &[("card_count", &out.len().to_string())],
        );
    }
    Ok(out)
}

fn type_of(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn parse_rocm_json_sample(json: &str) -> Vec<GpuSample> {
    parse_rocm_json_sample_checked(json).unwrap_or_default()
}

fn parse_rocm_json_sample_checked(json: &str) -> Result<Vec<GpuSample>, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let obj = v
        .as_object()
        .ok_or_else(|| format!("expected JSON object, got {:?}", type_of(&v)))?;
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
        let temperature_c = fields.as_object().into_iter().flatten().find_map(
            |(k, v): (&String, &serde_json::Value)| {
                if k.starts_with("Temperature (Sensor") && k.ends_with("(C)") {
                    v.as_str()
                        .and_then(|s| s.parse::<f32>().ok())
                        .map(|f| f as i32)
                } else {
                    None
                }
            },
        );
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
    Ok(out)
}

// ── macOS (Apple Silicon + discrete) ───────────────────────────────────

#[cfg(target_os = "macos")]
fn apple_inventory() -> Vec<GpuInfo> {
    let args = ["SPDisplaysDataType", "-json"];
    let stdout = match run_tool("system_profiler", &args) {
        ToolRun::Ok(s) => s,
        ToolRun::NotInstalled | ToolRun::Failed => return Vec::new(),
    };
    let v: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => {
            report_parse_failure(
                "system_profiler",
                "inventory/json",
                &stdout,
                &format!("invalid JSON: {e}"),
            );
            return Vec::new();
        }
    };
    let arr = match v.get("SPDisplaysDataType").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => {
            report_parse_failure(
                "system_profiler",
                "inventory/schema",
                &stdout,
                "missing or non-array SPDisplaysDataType key",
            );
            return Vec::new();
        }
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
        // memory Apple Silicon (shares with system RAM). Parse best-effort but
        // surface unknown formats to Sentry so we can extend the parser.
        let vram_raw = entry
            .get("spdisplays_vram")
            .or_else(|| entry.get("spdisplays_vram_shared"))
            .and_then(|v| v.as_str());
        let vram_total_bytes = match vram_raw {
            Some(raw) => match parse_macos_memory_checked(raw) {
                Ok(v) => v,
                Err(e) => {
                    sentry_ext::capture_error(
                        "gpu: unrecognised macOS VRAM format",
                        &[("raw", raw), ("detail", &e), ("gpu_name", name.as_str())],
                    );
                    0
                }
            },
            None => 0,
        };
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
    parse_macos_memory_checked(s).ok()
}

#[cfg(target_os = "macos")]
fn parse_macos_memory_checked(s: &str) -> Result<u64, String> {
    // Examples: "8 GB", "1024 MB", "24576 MB"
    let s = s.trim();
    let (num, unit) = s
        .rsplit_once(' ')
        .ok_or_else(|| format!("no space separator in {s:?}"))?;
    let n: u64 = num
        .parse()
        .map_err(|e| format!("can't parse numeric prefix {num:?}: {e}"))?;
    match unit.to_uppercase().as_str() {
        "KB" => Ok(n * 1024),
        "MB" => Ok(n * 1024 * 1024),
        "GB" => Ok(n * 1024 * 1024 * 1024),
        "TB" => Ok(n * 1024 * 1024 * 1024 * 1024),
        other => Err(format!("unknown unit {other:?}")),
    }
}

// ── Linux fallback ─────────────────────────────────────────────────────

fn lspci_fallback() -> Vec<GpuInfo> {
    // `lspci -mmn -d ::0300` lists display controllers in machine-readable form.
    // Fields: slot "vendor" "device" rev-progif "subsys_vendor" "subsys_device".
    let args = ["-mmn", "-d", "::0300"];
    let stdout = match run_tool("lspci", &args) {
        ToolRun::Ok(s) => s,
        ToolRun::NotInstalled | ToolRun::Failed => return Vec::new(),
    };
    let mut gpus = Vec::new();
    let mut short_rows = 0usize;
    for (lineno, line) in stdout.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields = split_lspci(line);
        if fields.len() < 3 {
            short_rows += 1;
            tracing::debug!(
                "lspci: short row at line {lineno}, got {} fields",
                fields.len()
            );
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
    if short_rows > 0 && gpus.is_empty() {
        report_parse_failure(
            "lspci",
            "fallback/field-count",
            &stdout,
            &format!("{short_rows} rows had < 3 fields; none parsed"),
        );
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

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_memory_parse_reports_error_detail() {
        let err = parse_macos_memory_checked("42 YB").unwrap_err();
        assert!(err.contains("unknown unit"), "err was: {err}");
        let err = parse_macos_memory_checked("gibberish").unwrap_err();
        assert!(err.contains("no space separator"), "err was: {err}");
    }

    #[test]
    fn rocm_invalid_json_returns_descriptive_error() {
        let err = parse_rocm_json_inventory_checked("{not json").unwrap_err();
        assert!(err.contains("invalid JSON"), "err was: {err}");
    }

    #[test]
    fn rocm_non_object_top_level_returns_error() {
        let err = parse_rocm_json_inventory_checked("[1,2,3]").unwrap_err();
        assert!(err.contains("expected JSON object"), "err was: {err}");
    }

    #[test]
    fn stdout_sample_truncates_big_output() {
        let big = "x".repeat(5_000);
        let sample = stdout_sample(&big);
        assert!(sample.len() < 1_200);
        assert!(sample.contains("truncated"));
    }
}
