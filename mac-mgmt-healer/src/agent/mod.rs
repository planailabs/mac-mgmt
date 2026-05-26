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
/// ML-derived hints to inject into the system prompt.
/// These are optional predictions from trained models in mac-mgmt-trainer.
#[derive(Debug, Default, Clone)]
pub struct MlHints {
    /// Ranked tool recommendations: (tool_name, confidence 0.0-1.0).
    pub tool_recommendations: Vec<(String, f32)>,
    /// Predicted outcome: (label, probability).
    /// Labels: "success", "failure", "escalation".
    pub outcome_prediction: Option<(String, f32)>,
    /// Similar past sessions: (session_label, similarity).
    pub similar_sessions: Vec<(String, f32)>,
}

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
    auto_approve: bool,
    diagnosis_only: bool,
    metrics_summary: &str,
    ml_hints: Option<&MlHints>,
) -> String {
    let mut prompt = String::with_capacity(4096);

    if diagnosis_only {
        prompt.push_str("You are a server diagnosis agent for the mac-mgmt fleet management system.\n\
            Your job is to **diagnose only** — gather information, identify the root cause, and report findings.\n\
            Do NOT attempt fixes. A separate remediation step will handle that after your diagnosis is reviewed.\n\n\
            Always take action by calling tools rather than describing what you would do.\n\
            When your diagnosis is complete, pin your findings to the \"diagnosis\" slot and \
            call `set_phase(\"remediating\")` to hand off for approval.\n\n");
    } else {
        prompt.push_str("You are an autonomous server healing agent for the mac-mgmt fleet management system.\n\
            You run non-interactively over multiple rounds with no human in the loop.\n\
            Diagnose the issue, apply fixes, verify the result, and mark the session done — all on your own.\n\
            Do not ask for confirmation or wait for human input. If you get stuck after exhausting your options, \
            call `staff_ping` and set phase to `needs_human_attention`.\n\n\
            Always take action by calling tools rather than describing what you would do. \
            When you are finished, call `set_phase` with `done`.\n\n");
    }

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

    // Prometheus metrics snapshot
    if !metrics_summary.is_empty() {
        prompt.push_str("## Prometheus Metrics (snapshot)\n");
        prompt.push_str("These are the current Prometheus metrics from the instance. Use `get_metrics` to query fresh values during diagnosis.\n\n");
        prompt.push_str("```\n");
        prompt.push_str(metrics_summary);
        prompt.push_str("\n```\n\n");
    }

    // Available tunnels
    prompt.push_str("## Available Tools\n\n");
    prompt.push_str("**IMPORTANT: Only use tools that are provided in the tool definitions. Do not invent tool names or parameters that are not in the definitions.**\n\n");
    prompt.push_str("### Instance interaction\n");
    if !file_tunnels.is_empty() {
        prompt.push_str(&format!(
            "- File tunnels (use `read_file`/`write_file` with tunnel name): {}\n",
            file_tunnels.join(", ")
        ));
    }
    if !shell_commands.is_empty() {
        prompt.push_str(&format!(
            "- Shell commands (use `run_command` with command_name): {}\n",
            shell_commands.join(", ")
        ));
        prompt
            .push_str("  Use `list_shell_commands` to see descriptions and accepted arguments.\n");
    }
    prompt.push_str("- `fetch_logs` — fetch logs, optionally filtered by service\n");
    if !other_instances.is_empty() {
        prompt.push_str("- `fetch_cluster_logs` / `run_cluster_command` — for other instances\n");
    }
    prompt.push_str("\n### Session management\n");
    prompt.push_str("- `pin` — pin key information to the session (slots: `diagnosis`");
    if !diagnosis_only {
        prompt.push_str(", `remediation`, `final_report`");
    }
    prompt.push_str(")\n");
    prompt.push_str("- `set_phase` — transition between phases\n");
    prompt.push_str("- `name_session` — give this session a short descriptive name once you understand the issue (e.g. \"OOM crash in ollama\"). Call this early.\n");
    prompt.push_str("- `staff_ping` — notify admins when you need human help or encounter something unexpected\n");
    prompt.push_str("- `list_staff_pings` — list unresolved staff pings for this instance (check before creating a new one to avoid duplicates)\n");
    prompt.push_str("- `check_node_online` — check if the target node is connected to the relay\n");
    prompt.push_str("- `wait_for_node` — wait for the node to reconnect (e.g. after a reboot)\n");
    prompt.push_str("- `get_probe_status` — query fresh health probe results and system resources from the latest heartbeat\n");
    if !diagnosis_only {
        prompt.push_str(
            "- `wait` — pause for N seconds (1-300). Use after config changes or restarts.\n",
        );
        prompt.push_str(
            "- `request_assessment` — trigger an immediate health probe run on the instance\n\n",
        );
        prompt.push_str("### Cluster config management\n");
        prompt.push_str("- `get_config` — read the current cluster configuration\n");
        prompt.push_str(
            "- `patch_config` — merge a JSON patch into the config (only changed fields)\n",
        );
        prompt.push_str("- `set_config` — replace the entire cluster config\n");
        prompt.push_str("- `list_skills` / `add_skill` / `remove_skill` — manage cluster skills\n");
        prompt.push_str("- `list_mcp_servers` / `add_mcp_server` / `remove_mcp_server` — manage cluster MCP servers\n");
    } else {
        prompt.push_str("\n### Cluster config (read-only)\n");
        prompt.push_str("- `get_config` — read the current cluster configuration\n");
        prompt.push_str("- `list_skills` — list skills assigned to cluster\n");
        prompt.push_str("- `list_mcp_servers` — list MCP servers assigned to cluster\n");
    }
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
    if diagnosis_only {
        prompt.push_str("## Guidelines\n\n\
            You operate in multiple rounds. Each round you should call one or more tools, observe the \
            results, and decide the next action.\n\n\
            ### Workflow\n\
            1. **Round 1 — Gather**: fetch logs, check probe status, read config, check metrics. Do NOT skip this.\n\
            2. **Round 2+ — Diagnose**: analyse the data, identify the root cause\n\
            3. **Final round — Report**: pin your findings to the `diagnosis` slot (include affected services), \
               then call `set_phase(\"remediating\")` to request approval. Do NOT attempt to fix anything.\n\n\
            ### Rules\n\
            - Be thorough — check logs, probes, metrics, config, and system resources\n\
            - When a service's behavior is unclear, look up its documentation using `read_doc` or Context7\n\
            - If you need a tool or capability that is not available, use `staff_ping` with category `tool_needed`\n\
            - If you cannot determine the root cause, call `staff_ping` and set phase to `needs_human_attention`\n\n");
    } else {
        prompt.push_str("## Guidelines\n\n\
            You operate in multiple rounds. Each round you should call one or more tools, observe the \
            results, and decide the next action. Never try to diagnose and fix everything in a single \
            round — gather information first, then act, then verify.\n\n\
            ### Workflow\n\
            1. **Round 1 — Gather**: fetch logs, check probe status, read config. Do NOT skip this.\n\
            2. **Round 2+ — Diagnose**: analyse the data, pin your `diagnosis`, call `set_phase(\"diagnosing\")`\n\
            3. **Round 3+ — Remediate**: pin your `remediation` plan, apply fixes, call `set_phase(\"remediating\")`\n\
            4. **Round 4+ — Verify**: wait for changes to take effect, then call `request_assessment` \
               and `get_probe_status` to confirm recovery. Call `set_phase(\"verifying\")`\n\
            5. **Final round — Close**: pin `final_report`, call `set_phase(\"done\")` or `set_phase(\"needs_human_attention\")`\n\n\
            ### Rules\n\
            - NEVER make changes without understanding the root cause first\n\
            - NEVER skip the verification round — always confirm your fix worked before closing\n\
            - Make minimal, targeted fixes — prefer config changes over restarts\n\
            - Explain every change you make and why\n\
            - When a service's configuration format or behavior is unclear, look up its documentation using `read_doc` (for mac-mgmt docs) or Context7 (for third-party service docs) before guessing\n\
            - If you need a tool or capability that is not available, use `staff_ping` with category `tool_needed` describing what you need and why\n\
            - If you **cannot** fix the issue after multiple attempts, call `staff_ping`, pin `final_report` documenting your findings, and set phase to `needs_human_attention`\n\n");
    }

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

    // Remediation procedures (skip in diagnosis-only mode — the fix model gets them on resume)
    if !diagnosis_only {
        let error_classes: Vec<String> = services_extended
            .iter()
            .filter(|s| !s.healthy)
            .filter_map(|_| Some("error".to_string()))
            .collect();
        if !error_classes.is_empty() {
            prompt.push_str("## Remediation Procedures\n");
            prompt.push_str(&skills::format_skills_for_prompt(&error_classes));
        }
    }

    // Approval mode
    if !auto_approve {
        prompt.push_str(
            "\n## Approval Required\n\
            This session requires human approval before remediation. \
            You are in diagnosis-only mode — mutating tools (write_file, run_command, \
            config changes, etc.) are not available.\n\
            Complete your diagnosis using the available read-only tools. \
            Pin your findings with the `pin` tool (use the \"diagnosis\" slot). \
            When ready, call `set_phase(\"remediating\")` to request approval. \
            The session will pause for human review before remediation tools are unlocked.\n",
        );
    }

    // ML model hints
    if let Some(hints) = ml_hints {
        let has_content = !hints.tool_recommendations.is_empty()
            || hints.outcome_prediction.is_some()
            || !hints.similar_sessions.is_empty();
        if has_content {
            prompt.push_str(
                "\n## ML Model Hints\n\
                The following are predictions from trained models based on past session data. \
                Use these as soft guidance — they may be wrong.\n\n",
            );

            if !hints.tool_recommendations.is_empty() {
                prompt.push_str("**Suggested tools** (ranked by historical relevance):\n");
                for (tool, confidence) in hints.tool_recommendations.iter().take(5) {
                    prompt.push_str(&format!(
                        "- `{tool}` ({:.0}% confidence)\n",
                        confidence * 100.0
                    ));
                }
                prompt.push('\n');
            }

            if let Some((label, prob)) = &hints.outcome_prediction {
                if *prob > 0.6 {
                    prompt.push_str(&format!(
                        "**Outcome forecast**: {label} ({:.0}% probability). ",
                        prob * 100.0
                    ));
                    if label == "failure" || label == "escalation" {
                        prompt.push_str("Consider early escalation via `staff_ping` if you encounter obstacles.\n");
                    }
                    prompt.push('\n');
                }
            }

            if !hints.similar_sessions.is_empty() {
                prompt.push_str("**Similar past sessions**:\n");
                for (label, similarity) in hints.similar_sessions.iter().take(3) {
                    prompt.push_str(&format!("- {label} (similarity: {similarity:.2})\n"));
                }
                prompt.push('\n');
            }
        }
    }

    // Resume context
    if let Some(ctx) = resume_context {
        prompt.push_str("## Resume Context\n");
        prompt.push_str(ctx);
        prompt.push('\n');
    }

    prompt
}

/// Build a system prompt adapted for external MCP clients (e.g. Claude Code).
///
/// Key differences from the autonomous agent prompt:
/// - Instructs the agent that it works interactively with a human operator
/// - Replaces `staff_ping` escalation with "ask the human on console"
/// - Removes staff_ping tool guidance section
pub fn build_system_prompt_external(
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
    metrics_summary: &str,
) -> String {
    let mut prompt = build_system_prompt(
        cluster_name,
        cluster_id,
        instance_id,
        hostname,
        other_instances,
        services_extended,
        sample_summary,
        file_tunnels,
        shell_commands,
        resume_context,
        /* auto_approve */ true,
        /* diagnosis_only */ false,
        metrics_summary,
        /* ml_hints */ None,
    );

    // Replace autonomous preamble with interactive one
    let autonomous_preamble = "You are an autonomous server healing agent for the mac-mgmt fleet management system.\n\
        You run non-interactively over multiple rounds with no human in the loop.\n\
        Diagnose the issue, apply fixes, verify the result, and mark the session done — all on your own.\n\
        Do not ask for confirmation or wait for human input. If you get stuck after exhausting your options, \
        call `staff_ping` and set phase to `needs_human_attention`.";
    let interactive_preamble = "You are a server healing agent for the mac-mgmt fleet management system.\n\
        You are working interactively with a human operator via an MCP client (e.g. Claude Code).\n\
        Diagnose the issue, apply fixes, verify the result, and mark the session done.\n\
        If you get stuck after exhausting your options, stop and ask the human operator for help directly \
        — do NOT create staff pings for escalation.";
    prompt = prompt.replace(autonomous_preamble, interactive_preamble);

    // Replace staff_ping guidance section
    let staff_ping_section_start = "## When to use staff_ping\n";
    if let Some(start) = prompt.find(staff_ping_section_start) {
        // Find the next section header (## )
        let rest = &prompt[start + staff_ping_section_start.len()..];
        let section_end = rest
            .find("\n## ")
            .map(|pos| start + staff_ping_section_start.len() + pos)
            .unwrap_or(prompt.len());
        let replacement = "## When to escalate to the human operator\n\
            When you encounter issues you cannot resolve after multiple attempts, \
            stop and report your findings to the human operator on the console. \
            Do NOT create staff pings — the human is watching and can intervene directly.\n\
            Situations that warrant escalation:\n\
            - Hardware failures, disk errors, GPU issues\n\
            - Network/DNS problems you cannot fix via config\n\
            - Missing credentials or permissions\n\
            - Repeated crash loops after remediation attempts\n\
            - Security concerns\n\n";
        prompt.replace_range(start..section_end, replacement);
    }

    prompt
}

/// Build a condensed system prompt for a fine-tuned model.
///
/// The fine-tuned model has internalized tool descriptions, workflow guidelines,
/// and staff_ping guidance. This prompt only includes dynamic per-session context.
pub fn build_system_prompt_finetuned(
    cluster_name: &str,
    instance_id: &str,
    hostname: &str,
    other_instances: &[InstanceInfo],
    services_extended: &[mac_mgmt_common::ServiceExtState],
    sample_summary: &str,
    resume_context: Option<&str>,
    auto_approve: bool,
    diagnosis_only: bool,
    metrics_summary: &str,
    ml_hints: Option<&MlHints>,
) -> String {
    let mut prompt = String::with_capacity(2048);

    if diagnosis_only {
        prompt.push_str("Mode: diagnosis-only (no fixes).\n\n");
    }

    prompt.push_str(&format!("## Target\n- Cluster: {cluster_name}\n- Instance: {instance_id}\n- Hostname: {hostname}\n\n"));

    // Other instances
    if !other_instances.is_empty() {
        prompt.push_str("## Cluster Instances\n");
        for inst in other_instances {
            let status = if inst.healthy { "healthy" } else { "UNHEALTHY" };
            prompt.push_str(&format!(
                "- {} ({}): {}\n",
                inst.instance_prefix, inst.hostname, status
            ));
        }
        prompt.push('\n');
    }

    // Unhealthy services
    let unhealthy: Vec<_> = services_extended.iter().filter(|s| !s.healthy).collect();
    if !unhealthy.is_empty() {
        prompt.push_str("## Detected Issues\n");
        for svc in &unhealthy {
            prompt.push_str(&format!("- {}: unhealthy\n", svc.name));
        }
        prompt.push('\n');
    }

    // System resources
    if !sample_summary.is_empty() {
        prompt.push_str(&format!("## System Resources\n{sample_summary}\n\n"));
    }

    // Metrics
    if !metrics_summary.is_empty() {
        prompt.push_str(&format!("## Metrics\n{metrics_summary}\n\n"));
    }

    if !auto_approve {
        prompt.push_str("## Approval Required\nMutating tools unavailable until human approval. Pin diagnosis, then call `set_phase(\"remediating\")` to request.\n\n");
    }

    // ML hints
    if let Some(hints) = ml_hints {
        if !hints.tool_recommendations.is_empty() || hints.outcome_prediction.is_some() {
            prompt.push_str("## ML Hints\n");
            if !hints.tool_recommendations.is_empty() {
                prompt.push_str("Suggested tools: ");
                let tools: Vec<_> = hints
                    .tool_recommendations
                    .iter()
                    .take(3)
                    .map(|(t, c)| format!("`{t}` ({:.0}%)", c * 100.0))
                    .collect();
                prompt.push_str(&tools.join(", "));
                prompt.push('\n');
            }
            if let Some((label, prob)) = &hints.outcome_prediction {
                if *prob > 0.6 {
                    prompt.push_str(&format!(
                        "Outcome forecast: {label} ({:.0}%)\n",
                        prob * 100.0
                    ));
                }
            }
            prompt.push('\n');
        }
    }

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
