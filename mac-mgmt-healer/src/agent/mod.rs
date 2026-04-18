pub mod skills;

use mac_mgmt_common::ServiceExtState;

/// Information about an instance in the cluster.
#[derive(Debug, Clone)]
pub struct InstanceInfo {
    pub instance_prefix: String,
    pub hostname: String,
    pub healthy: bool,
}

/// Build the system prompt for a healer session.
pub fn build_system_prompt(
    cluster_name: &str,
    cluster_id: &str,
    instance_id: &str,
    hostname: &str,
    other_instances: &[InstanceInfo],
    services_extended: &[ServiceExtState],
    sample_summary: &str,
    file_tunnels: &[String],
    shell_commands: &[String],
    resume_context: Option<&str>,
) -> String {
    let mut prompt = String::with_capacity(4096);

    prompt.push_str("You are a server healing agent for the mac-mgmt fleet management system.\n\n");

    // Target info
    prompt.push_str("## Target\n");
    prompt.push_str(&format!("- Cluster: {cluster_name} (ID: {cluster_id})\n"));
    prompt.push_str(&format!("- Instance: {instance_id} ({hostname})\n"));
    if !other_instances.is_empty() {
        prompt.push_str(&format!(
            "- Other instances in cluster: {}\n",
            other_instances
                .iter()
                .map(|i| format!(
                    "{} ({}{})",
                    i.instance_prefix,
                    i.hostname,
                    if i.healthy { "" } else { ", UNHEALTHY" }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    prompt.push('\n');

    // Detected issues
    let unhealthy: Vec<_> = services_extended
        .iter()
        .filter(|s| !s.healthy)
        .collect();
    if !unhealthy.is_empty() {
        prompt.push_str("## Detected Issues\n");
        for svc in &unhealthy {
            let probe_info = match (&svc.last_probe_kind, svc.last_probe_at) {
                (Some(kind), Some(at)) => {
                    let dt = chrono::DateTime::from_timestamp(at, 0)
                        .map(|d| d.to_rfc3339())
                        .unwrap_or_else(|| at.to_string());
                    format!("last {kind} probe failed at {dt}")
                }
                _ => "no recent probe data".to_string(),
            };
            prompt.push_str(&format!("- **{}**: UNHEALTHY — {}\n", svc.name, probe_info));
        }
        prompt.push('\n');
    }

    // Dynamic sample
    if !sample_summary.is_empty() {
        prompt.push_str("## System Resources\n");
        prompt.push_str(sample_summary);
        prompt.push_str("\n\n");
    }

    // Available tunnels
    prompt.push_str("## Available Tools\n");
    if !file_tunnels.is_empty() {
        prompt.push_str(&format!("- File tunnels: {}\n", file_tunnels.join(", ")));
    }
    if !shell_commands.is_empty() {
        prompt.push_str(&format!("- Shell commands: {}\n", shell_commands.join(", ")));
    }
    prompt.push_str("- Logs: available via fetch_logs (optionally filtered by service)\n");
    if !other_instances.is_empty() {
        prompt.push_str("- Cross-instance: fetch_cluster_logs and run_cluster_command for other instances\n");
    }
    prompt.push('\n');

    // Guidelines
    prompt.push_str("## Guidelines\n\
        1. Start by reading logs for the failing service(s)\n\
        2. Check current configuration files for obvious issues\n\
        3. Look for resource exhaustion (CPU, memory, disk, VRAM) in the system resources above\n\
        4. Make minimal, targeted fixes — prefer config changes over restarts\n\
        5. Explain every change you make and why\n\
        6. If you cannot fix the issue, document what you found and suggest manual steps\n\
        7. NEVER make changes without understanding the root cause first\n\
        8. Call report_phase when you transition between diagnosis, remediation, and verification\n\n");

    // Remediation procedures
    let error_classes: Vec<String> = services_extended
        .iter()
        .filter(|s| !s.healthy)
        .filter_map(|_| Some("error".to_string())) // TODO: integrate error_class from probes
        .collect();
    if !error_classes.is_empty() {
        prompt.push_str("## Remediation Procedures\n");
        prompt.push_str(&skills::format_skills_for_prompt(&error_classes));
    }

    // Resume context
    if let Some(ctx) = resume_context {
        prompt.push_str("## Resume Context\n");
        prompt.push_str(ctx);
        prompt.push('\n');
    }

    prompt
}

/// Format a DynamicSample into a human-readable summary for the system prompt.
pub fn format_sample_summary(sample: &serde_json::Value) -> String {
    let mut parts = Vec::new();

    if let Some(cpu) = sample.get("cpu_load_1m").and_then(|v| v.as_f64()) {
        parts.push(format!("CPU load: {cpu:.1}"));
    }
    if let (Some(mem_used), Some(mem_total)) = (
        sample.get("memory_used_mb").and_then(|v| v.as_u64()),
        sample.get("memory_total_mb").and_then(|v| v.as_u64()),
    ) {
        parts.push(format!("Memory: {mem_used}/{mem_total} MB"));
    }
    if let Some(disk_free) = sample.get("disk_free_mb").and_then(|v| v.as_u64()) {
        parts.push(format!("Disk free: {disk_free} MB"));
    }
    if let Some(gpu) = sample.get("gpu").and_then(|v| v.as_array()) {
        for (i, g) in gpu.iter().enumerate() {
            let util = g.get("utilization_pct").and_then(|v| v.as_u64()).unwrap_or(0);
            let vram_used = g.get("vram_used_mb").and_then(|v| v.as_u64()).unwrap_or(0);
            let vram_total = g.get("vram_total_mb").and_then(|v| v.as_u64()).unwrap_or(0);
            let temp = g.get("temperature_c").and_then(|v| v.as_u64()).unwrap_or(0);
            parts.push(format!(
                "GPU{i}: {util}% util, VRAM {vram_used}/{vram_total} MB, {temp}C"
            ));
        }
    }

    if parts.is_empty() {
        return String::new();
    }
    parts.join("\n")
}
