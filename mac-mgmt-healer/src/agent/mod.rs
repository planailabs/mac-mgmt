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
    let unhealthy: Vec<_> = services_extended.iter().filter(|s| !s.healthy).collect();
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
    prompt.push_str("## Available Tools\n\n");
    prompt.push_str("### Instance interaction\n");
    if !file_tunnels.is_empty() {
        prompt.push_str(&format!("- File tunnels: {}\n", file_tunnels.join(", ")));
    }
    if !shell_commands.is_empty() {
        prompt.push_str(&format!(
            "- Shell commands: {}\n",
            shell_commands.join(", ")
        ));
    }
    prompt.push_str("- `fetch_logs` — fetch logs, optionally filtered by service\n");
    if !other_instances.is_empty() {
        prompt.push_str("- `fetch_cluster_logs` / `run_cluster_command` — for other instances\n");
    }
    prompt.push_str("\n### Session management\n");
    prompt.push_str("- `pin` — pin key information to the session (three slots):\n");
    prompt.push_str(
        "  - `diagnosis` slot: pin once you identify the root cause (include affected services)\n",
    );
    prompt.push_str("  - `remediation` slot: pin your remediation plan before applying fixes\n");
    prompt.push_str("  - `final_report` slot: pin at the end summarizing what was done and any remaining issues\n");
    prompt.push_str("- `set_phase` — transition between phases: `diagnosing`, `remediating`, `verifying`, `done`, `needs_human_attention`\n");
    prompt.push_str("- `staff_ping` — notify admins when you need human help or encounter something unexpected\n");
    prompt.push_str("- `check_node_online` — check if the target node is connected to the relay\n");
    prompt.push_str("- `wait_for_node` — wait for the node to reconnect (e.g. after a reboot)\n");
    prompt.push_str("- `get_probe_status` — query fresh health probe results and system resources from the latest heartbeat\n");
    prompt.push_str(
        "- `wait` — pause for N seconds (1-300). Use after config changes or restarts.\n",
    );
    prompt.push_str(
        "- `request_assessment` — trigger an immediate health probe run on the instance\n\n",
    );
    prompt.push_str("### Cluster config management\n");
    prompt.push_str("- `get_config` — read the current cluster configuration\n");
    prompt
        .push_str("- `patch_config` — merge a JSON patch into the config (only changed fields)\n");
    prompt.push_str("- `set_config` — replace the entire cluster config\n");
    prompt.push_str("- `list_skills` / `add_skill` / `remove_skill` — manage cluster skills\n");
    prompt.push_str("- `list_mcp_servers` / `add_mcp_server` / `remove_mcp_server` — manage cluster MCP servers\n");
    prompt.push('\n');

    prompt.push_str("### Documentation\n");
    prompt.push_str("- `read_doc` — read mac-mgmt platform documentation by slug (e.g. \"configuration-reference\", \"cluster-setup\")\n");
    prompt.push_str("- `list_docs` — list all available documentation pages\n");
    prompt.push_str("- If Context7 tools are available (`context7-resolve-library-id`, `context7-query-docs`), use them to look up current documentation for third-party services (Ollama, LM Studio, nix, systemd, etc.) when the service's behavior or configuration is unclear.\n\n");

    prompt.push_str("### Skills\n");
    prompt.push_str("- `list_builtin_skills` — list available healer skills (step-by-step procedures for common tasks)\n");
    prompt.push_str("- `use_skill` — load a skill by slug to get detailed instructions for a specific remediation task\n");
    prompt.push_str("- Use skills when you encounter a matching situation — they encode proven procedures from past sessions\n\n");

    // Guidelines
    prompt.push_str("## Guidelines\n\
        1. Start by reading logs for the failing service(s)\n\
        2. Check current configuration files for obvious issues\n\
        3. Look for resource exhaustion (CPU, memory, disk, VRAM) in the system resources above\n\
        4. Once you identify the root cause, call `pin` with slot `diagnosis`\n\
        5. Call `set_phase` when transitioning between stages of your work\n\
        6. Before applying fixes, call `pin` with slot `remediation` describing your plan\n\
        7. Make minimal, targeted fixes — prefer config changes over restarts\n\
        8. Explain every change you make and why\n\
        9. After applying a fix, call `set_phase` with `verifying`, then use `get_probe_status` to check if services recovered\n\
        10. If the fix worked, call `pin` with slot `final_report` summarizing what was done, \
            then call `set_phase` with `done`\n\
        11. If you **cannot** fix the issue automatically, call `staff_ping` to notify admins, \
            call `pin` with slot `final_report` documenting your findings, \
            then call `set_phase` with `needs_human_attention`\n\
        12. NEVER make changes without understanding the root cause first\n\
        13. When a service's configuration format or behavior is unclear, look up its documentation using `read_doc` (for mac-mgmt docs) or Context7 (for third-party service docs) before guessing\n\
        14. If you need a tool or capability that is not available, use `staff_ping` with category `tool_needed` describing what you need and why — staff can add tools for future sessions\n\n");

    // Staff pings guidance
    prompt.push_str(
        "## When to use staff_ping\n\
        Use `staff_ping` to create actionable notifications for admin staff:\n\
        - **hardware**: GPU failures, bad RAM, disk errors\n\
        - **network**: DNS issues, firewall blocks, connectivity problems\n\
        - **disk_space**: filesystem full, needs cleanup\n\
        - **config_error**: configuration you can't fix (e.g. needs credential rotation)\n\
        - **service_crash**: repeated crash loops you can't resolve\n\
        - **model_issue**: model corruption, incompatible model format\n\
        - **permission**: file permission issues, access denied\n\
        - **dependency**: missing system packages, library version conflicts\n\
        - **security**: suspicious activity, certificate expiry\n\
        - **performance**: severe degradation that needs investigation\n\
        - **tool_needed**: a tool you need is not available (describe what tool/capability is missing and why you need it)\n\
        - **other**: anything that doesn't fit the above\n\n\
        Also use `staff_ping` when you encounter unexpected errors during tool calls \
        that might indicate a deeper infrastructure issue.\n\n\
        Common tool errors to watch for:\n\
        - **\"validation command failed to run: No such file or directory\"**: The file tunnel \
          has a validator configured but the binary is missing on the daemon. The write was \
          rolled back. Send a `staff_ping` with category `dependency` and do NOT retry the \
          write — it will fail again until the binary is installed.\n\
        - **502/503/504 errors**: The daemon disconnected from the relay. The tool will \
          automatically wait up to 10 minutes for it to reconnect. If it times out, use \
          `check_node_online` and consider using `staff_ping` with category `network`.\n\n",
    );

    // Remediation procedures
    let error_classes: Vec<String> = services_extended
        .iter()
        .filter(|s| !s.healthy)
        .filter_map(|_| Some("error".to_string()))
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
            let util = g
                .get("utilization_pct")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
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
