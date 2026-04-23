//! Native swiftide tools wrapping instance access traits.
//! Registered directly on the agent — no MCP transport needed.

use std::borrow::Cow;
use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use swiftide::chat_completion::{Tool, ToolCall, ToolOutput, ToolSpec, errors::ToolError};
use swiftide::traits::AgentContext;

/// Callback for sending SSE push events to daemons.
pub type PushFn = Arc<dyn Fn(uuid::Uuid, mac_mgmt_common::PushEvent) + Send + Sync>;

/// Shared context for all healer tools.
#[derive(Clone)]
pub struct ToolContext {
    pub instance: crate::instance_access::DynInstanceAccess,
    pub instance_data: crate::instance_data::DynInstanceData,
    pub cluster: Option<crate::instance_access::DynClusterAccess>,
    pub target_instance: String,
    pub cluster_instances: Vec<String>,
    pub file_tunnels: Vec<String>,
    pub file_tunnels_full: serde_json::Value,
    pub shell_commands: Vec<String>,
    pub shell_commands_full: serde_json::Value,
    pub store: crate::store::DynStore,
    pub session_id: uuid::Uuid,
    pub cluster_id: uuid::Uuid,
    pub instance_id: String,
    /// Send a push event to all daemons in a cluster.
    pub push_fn: Option<PushFn>,
    /// Broadcast healer events (status messages, etc.) to the SSE stream.
    pub events_tx: tokio::sync::broadcast::Sender<crate::session::HealerEvent>,
    /// Optional metrics URL for the get_metrics tool.
    pub metrics_url: Option<String>,
    /// Whether remediation is auto-approved (false = diagnosis-only until approved).
    pub auto_approve: bool,
    /// Notify handle to signal AwaitingApproval to the select! loop.
    pub approval_notify: Arc<tokio::sync::Notify>,
}

macro_rules! healer_tool {
    (
        name: $name:expr,
        struct_name: $struct_name:ident,
        description: $desc:expr,
        params: $params_ty:ty,
        handler: |$ctx_var:ident, $params_var:ident| $body:expr
    ) => {
        #[derive(Clone)]
        pub struct $struct_name {
            ctx: ToolContext,
        }

        impl $struct_name {
            pub fn new(ctx: ToolContext) -> Box<dyn Tool> {
                Box::new(Self { ctx })
            }
        }

        #[async_trait]
        impl Tool for $struct_name {
            fn tool_spec(&self) -> ToolSpec {
                let schema = schemars::schema_for!($params_ty);
                ToolSpec::builder()
                    .name($name)
                    .description($desc)
                    .parameters_schema(
                        serde_json::from_value::<schemars::Schema>(
                            serde_json::to_value(&schema).unwrap(),
                        )
                        .unwrap(),
                    )
                    .build()
                    .unwrap()
            }

            fn name(&self) -> Cow<'_, str> {
                Cow::Borrowed($name)
            }

            async fn invoke(
                &self,
                _agent_context: &dyn AgentContext,
                tool_call: &ToolCall,
            ) -> Result<ToolOutput, ToolError> {
                let args = tool_call
                    .args()
                    .ok_or_else(|| ToolError::MissingArguments("no arguments".into()))?;
                tracing::debug!(tool = $name, args = %args, "tool invoked");
                let $params_var: $params_ty = serde_json::from_str(&args)
                    .map_err(|e| ToolError::MissingArguments(e.to_string().into()))?;
                let $ctx_var = &self.ctx;
                $body
            }
        }
    };
    // No-params variant
    (
        name: $name:expr,
        struct_name: $struct_name:ident,
        description: $desc:expr,
        handler: |$ctx_var:ident| $body:expr
    ) => {
        #[derive(Clone)]
        pub struct $struct_name {
            ctx: ToolContext,
        }

        impl $struct_name {
            pub fn new(ctx: ToolContext) -> Box<dyn Tool> {
                Box::new(Self { ctx })
            }
        }

        #[async_trait]
        impl Tool for $struct_name {
            fn tool_spec(&self) -> ToolSpec {
                ToolSpec::builder()
                    .name($name)
                    .description($desc)
                    .build()
                    .unwrap()
            }

            fn name(&self) -> Cow<'_, str> {
                Cow::Borrowed($name)
            }

            async fn invoke(
                &self,
                _agent_context: &dyn AgentContext,
                _tool_call: &ToolCall,
            ) -> Result<ToolOutput, ToolError> {
                tracing::debug!(tool = $name, "tool invoked (no params)");
                let $ctx_var = &self.ctx;
                $body
            }
        }
    };
}

// ── Parameter types ────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct ListFilesParams {
    /// File tunnel name (e.g. "ollama-config")
    tunnel_name: String,
    /// Optional subdirectory path within the tunnel
    #[serde(default)]
    path: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct ReadFileParams {
    /// File tunnel name
    tunnel_name: String,
    /// Path within the tunnel to read
    path: String,
}

#[derive(Deserialize, JsonSchema)]
struct WriteFileParams {
    /// File tunnel name
    tunnel_name: String,
    /// Path within the tunnel to write
    path: String,
    /// File content to write (text)
    content: String,
    /// Expected mtime from a previous read (for optimistic concurrency)
    #[serde(default)]
    expected_mtime: Option<i64>,
}

#[derive(Deserialize, JsonSchema)]
struct RunCommandParams {
    /// Shell command name (must be one of the predefined commands)
    command_name: String,
    /// Optional argument to pass to the command
    #[serde(default)]
    user_arg: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct FetchLogsParams {
    /// Number of log lines to fetch. Defaults to 200.
    #[serde(default)]
    n: Option<usize>,
    /// Filter logs by service name (e.g. "ollama", "openclaw")
    #[serde(default)]
    service: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct ClusterLogsParams {
    /// Instance ID prefix of another instance in the same cluster
    instance_prefix: String,
    /// Number of log lines to fetch
    #[serde(default)]
    n: Option<usize>,
    /// Filter logs by service name
    #[serde(default)]
    service: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct ClusterCommandParams {
    /// Instance ID prefix of another instance in the same cluster
    instance_prefix: String,
    /// Shell command name
    command_name: String,
    /// Optional argument
    #[serde(default)]
    user_arg: Option<String>,
}

// ── Tool implementations ───────────────────────────────────────────────

healer_tool! {
    name: "list_files",
    struct_name: ListFilesTool,
    description: "List files in a file tunnel directory on the target instance",
    params: ListFilesParams,
    handler: |ctx, params| {
        match ctx.instance.file_list(&params.tunnel_name, params.path.as_deref()).await {
            Ok(val) => Ok(ToolOutput::Text(serde_json::to_string_pretty(&val).unwrap_or_default())),
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

healer_tool! {
    name: "read_file",
    struct_name: ReadFileTool,
    description: "Read a configuration file from the target instance via a file tunnel",
    params: ReadFileParams,
    handler: |ctx, params| {
        match ctx.instance.file_read(&params.tunnel_name, &params.path).await {
            Ok(result) => {
                let text = String::from_utf8_lossy(&result.content);
                let mut output = text.into_owned();
                if let Some(mtime) = result.mtime {
                    output.push_str(&format!("\n\n[mtime: {mtime}]"));
                }
                Ok(ToolOutput::Text(output))
            }
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

healer_tool! {
    name: "write_file",
    struct_name: WriteFileTool,
    description: "Write a configuration file to the target instance via a file tunnel. Include expected_mtime from a previous read to detect concurrent modifications.",
    params: WriteFileParams,
    handler: |ctx, params| {
        match ctx.instance.file_write(&params.tunnel_name, &params.path, params.content.as_bytes(), params.expected_mtime).await {
            Ok(val) => Ok(ToolOutput::Text(serde_json::to_string_pretty(&val).unwrap_or_default())),
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

healer_tool! {
    name: "run_command",
    struct_name: RunCommandTool,
    description: "Execute a predefined shell command on the target instance. Returns stdout, stderr, and exit code.",
    params: RunCommandParams,
    handler: |ctx, params| {
        match ctx.instance.shell_exec(&params.command_name, params.user_arg.as_deref()).await {
            Ok(output) => {
                let mut text = String::new();
                for line in &output.lines {
                    text.push_str(&format!("[{}] {}\n", line.stream, line.data));
                }
                if let Some(code) = output.exit_code {
                    text.push_str(&format!("\n[exit code: {code}]"));
                }
                if let Some(err) = &output.error {
                    text.push_str(&format!("\n[error: {err}]"));
                }
                Ok(ToolOutput::Text(text))
            }
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

healer_tool! {
    name: "fetch_logs",
    struct_name: FetchLogsTool,
    description: "Fetch recent logs from the target instance, optionally filtered by service name",
    params: FetchLogsParams,
    handler: |ctx, params| {
        match ctx.instance.log_fetch(params.n.or(Some(200)), params.service.as_deref(), None).await {
            Ok(val) => Ok(ToolOutput::Text(serde_json::to_string_pretty(&val).unwrap_or_default())),
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

healer_tool! {
    name: "list_file_tunnels",
    struct_name: ListFileTunnelsTool,
    description: "List all available file tunnels on the target instance with descriptions",
    handler: |ctx| {
        let entries = ctx.file_tunnels_full.as_array();
        match entries {
            Some(arr) if !arr.is_empty() => {
                let mut out = String::new();
                for entry in arr {
                    let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let desc = entry.get("description").and_then(|v| v.as_str()).unwrap_or("");
                    let kind = entry.get("kind").and_then(|v| v.as_str()).unwrap_or("file");
                    let writable = entry.get("writable").and_then(|v| v.as_bool()).unwrap_or(false);
                    let mode = if writable { "read-write" } else { "read-only" };
                    out.push_str(&format!("- {name} ({kind}, {mode})"));
                    if !desc.is_empty() {
                        out.push_str(&format!(" — {desc}"));
                    }
                    out.push('\n');
                }
                Ok(ToolOutput::Text(out))
            }
            _ => Ok(ToolOutput::Text("No file tunnels available.".to_string())),
        }
    }
}

healer_tool! {
    name: "list_shell_commands",
    struct_name: ListShellCommandsTool,
    description: "List all available shell commands on the target instance with descriptions",
    handler: |ctx| {
        let entries = ctx.shell_commands_full.as_array();
        match entries {
            Some(arr) if !arr.is_empty() => {
                let mut out = String::new();
                for entry in arr {
                    let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let desc = entry.get("description").and_then(|v| v.as_str()).unwrap_or("");
                    let has_arg = entry.get("arg_template").is_some_and(|v| !v.is_null());
                    out.push_str(&format!("- {name}"));
                    if has_arg {
                        let label = entry.get("arg_template")
                            .and_then(|t| t.get("label"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("arg");
                        out.push_str(&format!(" <{label}>"));
                    }
                    if !desc.is_empty() {
                        out.push_str(&format!(" — {desc}"));
                    }
                    out.push('\n');
                }
                Ok(ToolOutput::Text(out))
            }
            _ => Ok(ToolOutput::Text("No shell commands available.".to_string())),
        }
    }
}

healer_tool! {
    name: "fetch_cluster_logs",
    struct_name: FetchClusterLogsTool,
    description: "Fetch logs from a different instance in the same cluster",
    params: ClusterLogsParams,
    handler: |ctx, params| {
        let Some(cluster) = &ctx.cluster else {
            return Ok(ToolOutput::Text("Cluster access not available in local mode.".to_string()));
        };
        if !ctx.cluster_instances.contains(&params.instance_prefix) {
            return Ok(ToolOutput::Text(format!(
                "Error: instance '{}' is not in this cluster. Available: {:?}",
                params.instance_prefix, ctx.cluster_instances
            )));
        }
        let peer = cluster.instance_access(&params.instance_prefix).await
            .map_err(|e| ToolError::execution_failed(anyhow::anyhow!("Failed to get peer access: {e}")))?;
        match peer.log_fetch(params.n.or(Some(200)), params.service.as_deref(), None).await {
            Ok(val) => Ok(ToolOutput::Text(serde_json::to_string_pretty(&val).unwrap_or_default())),
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

healer_tool! {
    name: "run_cluster_command",
    struct_name: RunClusterCommandTool,
    description: "Run a shell command on a different instance in the same cluster",
    params: ClusterCommandParams,
    handler: |ctx, params| {
        let Some(cluster) = &ctx.cluster else {
            return Ok(ToolOutput::Text("Cluster access not available in local mode.".to_string()));
        };
        if !ctx.cluster_instances.contains(&params.instance_prefix) {
            return Ok(ToolOutput::Text(format!(
                "Error: instance '{}' is not in this cluster. Available: {:?}",
                params.instance_prefix, ctx.cluster_instances
            )));
        }
        let peer = cluster.instance_access(&params.instance_prefix).await
            .map_err(|e| ToolError::execution_failed(anyhow::anyhow!("Failed to get peer access: {e}")))?;
        match peer.shell_exec(&params.command_name, params.user_arg.as_deref()).await {
            Ok(output) => {
                let mut text = String::new();
                for line in &output.lines {
                    text.push_str(&format!("[{}] {}\n", line.stream, line.data));
                }
                if let Some(code) = output.exit_code {
                    text.push_str(&format!("\n[exit code: {code}]"));
                }
                if let Some(err) = &output.error {
                    text.push_str(&format!("\n[error: {err}]"));
                }
                Ok(ToolOutput::Text(text))
            }
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

// ── Session management tools ───────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct PinParams {
    /// Which slot to pin: "diagnosis" or "final_report"
    slot: String,
    /// The content to pin
    summary: String,
    /// List of affected services (for diagnosis slot)
    #[serde(default)]
    affected_services: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
struct StaffPingParams {
    /// Category: hardware, network, disk_space, config_error, service_crash,
    /// model_issue, permission, dependency, security, performance, other
    category: String,
    /// Clear description of what needs human attention and why
    message: String,
}

#[derive(Deserialize, JsonSchema)]
struct NameSessionParams {
    /// Short descriptive name for this session (max ~120 chars)
    name: String,
}

#[derive(Deserialize, JsonSchema)]
struct SetPhaseParams {
    /// Phase to transition to: diagnosing, remediating, verifying, done, needs_human_attention
    phase: String,
    /// Brief explanation of why you are transitioning to this phase
    reason: String,
}

healer_tool! {
    name: "pin",
    struct_name: PinTool,
    description: "Pin important information to the session. Three slots available:\n- \"diagnosis\": Pin once you identify the root cause. Include affected_services.\n- \"remediation\": Pin your remediation plan before applying fixes.\n- \"final_report\": Pin at the end summarizing what was done, what worked, and any remaining issues.\nAll are displayed to staff and persisted across restarts.",
    params: PinParams,
    handler: |ctx, params| {
        let slot = match params.slot.as_str() {
            "diagnosis" | "remediation" | "final_report" => params.slot.as_str(),
            _ => return Ok(ToolOutput::Text("Invalid slot. Use 'diagnosis' or 'final_report'.".to_string())),
        };
        // Read current state_data, merge in the pin
        let key = format!("pin_{slot}");
        let pin_data = serde_json::json!({
            "summary": params.summary,
            "affected_services": params.affected_services,
        });
        // Store as a state_data update
        let data = serde_json::json!({ key: pin_data });
        let pin_content = serde_json::to_string(&serde_json::json!({
            "slot": slot,
            "summary": params.summary,
            "affected_services": params.affected_services,
        })).unwrap_or_default();
        ctx.store.append_message(
            ctx.session_id,
            "pin",
            &pin_content,
            Some(&data),
        ).await.ok();
        // Broadcast so SSE clients update pins live
        let _ = ctx.events_tx.send(crate::session::HealerEvent::Message {
            role: "pin".to_string(),
            content: pin_content,
            metadata: Some(data),
            created_at: chrono::Utc::now(),
        });
        Ok(ToolOutput::Text(format!("Pinned to '{slot}': {}", params.summary)))
    }
}

healer_tool! {
    name: "staff_ping",
    struct_name: StaffPingTool,
    description: "Send a notification to the admin staff. Use this when you encounter an issue that requires human intervention, when you find something unexpected that admins should know about, or when you cannot resolve an issue automatically. Categories: hardware, network, disk_space, config_error, service_crash, model_issue, permission, dependency, security, performance, other.",
    params: StaffPingParams,
    handler: |ctx, params| {
        let category = if crate::session::models::PING_CATEGORIES.contains(&params.category.as_str()) {
            params.category.clone()
        } else {
            "other".to_string()
        };
        match ctx.store.create_staff_ping(
            ctx.session_id,
            ctx.cluster_id,
            &ctx.instance_id,
            &category,
            &params.message,
        ).await {
            Ok(id) => Ok(ToolOutput::Text(format!("Staff ping created (id: {id}, category: {category})"))),
            Err(e) => Ok(ToolOutput::Text(format!("Error creating staff ping: {e}"))),
        }
    }
}

healer_tool! {
    name: "list_staff_pings",
    struct_name: ListStaffPingsTool,
    description: "List unresolved staff pings for this instance. Check this before calling staff_ping to avoid creating duplicates.",
    handler: |ctx| {
        match ctx.store.list_instance_pings(&ctx.instance_id).await {
            Ok(pings) if pings.is_empty() => Ok(ToolOutput::Text("No unresolved staff pings for this instance.".to_string())),
            Ok(pings) => {
                let lines: Vec<String> = pings.iter().map(|p| {
                    format!("- [{}] {} ({})", p.category, p.message, p.created_at.format("%Y-%m-%d %H:%M"))
                }).collect();
                Ok(ToolOutput::Text(format!("{} unresolved ping(s):\n{}", pings.len(), lines.join("\n"))))
            }
            Err(e) => Ok(ToolOutput::Text(format!("Error listing pings: {e}"))),
        }
    }
}

healer_tool! {
    name: "set_phase",
    struct_name: SetPhaseTool,
    description: "Transition the session to a new phase. Call this when you move between stages of your work. Valid phases: diagnosing (investigating), remediating (applying fixes), verifying (checking if fix worked), done (work complete — whether fixed or not), needs_human_attention (cannot be fixed automatically, requires human intervention).",
    params: SetPhaseParams,
    handler: |ctx, params| {
        let Some(new_state) = crate::session::SessionState::agent_allowed(&params.phase) else {
            return Ok(ToolOutput::Text(format!(
                "Invalid phase '{}'. Valid: diagnosing, remediating, verifying, done, needs_human_attention",
                params.phase
            )));
        };

        // When approval is required, intercept the transition to remediating.
        if !ctx.auto_approve && new_state == crate::session::SessionState::Remediating {
            let data = serde_json::json!({ "reason": params.reason });
            match ctx.store.transition_state(
                ctx.session_id,
                &crate::session::SessionState::AwaitingApproval,
                &data,
            ).await {
                Ok(()) => {
                    crate::session::emit_state_change(&ctx.events_tx, "awaiting_approval", &data);
                    // Signal the select! loop to stop the agent immediately.
                    ctx.approval_notify.notify_one();
                    return Ok(ToolOutput::Text(
                        "Diagnosis complete. Session paused — awaiting remediation approval.".to_string()
                    ));
                }
                Err(e) => return Ok(ToolOutput::Text(format!("Error requesting approval: {e}"))),
            }
        }

        let data = serde_json::json!({ "reason": params.reason });
        match ctx.store.transition_state(
            ctx.session_id,
            &new_state,
            &data,
        ).await {
            Ok(()) => {
                crate::session::emit_state_change(&ctx.events_tx, &params.phase, &data);
                Ok(ToolOutput::Text(format!("Phase set to: {}", params.phase)))
            }
            Err(e) => Ok(ToolOutput::Text(format!("Error setting phase: {e}"))),
        }
    }
}

healer_tool! {
    name: "name_session",
    struct_name: NameSessionTool,
    description: "Give this session a short, descriptive name summarizing what it is about. Call this early — once you understand the issue. Example: \"OOM crash in ollama\", \"GPU driver mismatch\", \"stale nix store\".",
    params: NameSessionParams,
    handler: |ctx, params| {
        let label = params.name.chars().take(120).collect::<String>();
        match ctx.store.set_label(ctx.session_id, &label).await {
            Ok(()) => Ok(ToolOutput::Text(format!("Session named: {label}"))),
            Err(e) => Ok(ToolOutput::Text(format!("Error naming session: {e}"))),
        }
    }
}

healer_tool! {
    name: "check_node_online",
    struct_name: CheckNodeOnlineTool,
    description: "Check if the target node is currently connected to the relay. Returns online/offline status.",
    handler: |ctx| {
        let online = ctx.instance.is_online().await;
        Ok(ToolOutput::Text(if online {
            "Node is online and reachable.".to_string()
        } else {
            "Node is OFFLINE — not reachable.".to_string()
        }))
    }
}

#[derive(Deserialize, JsonSchema)]
struct WaitForNodeParams {
    /// Maximum seconds to wait (1-600, default 300)
    #[serde(default)]
    max_seconds: Option<u64>,
}

healer_tool! {
    name: "wait_for_node",
    struct_name: WaitForNodeTool,
    description: "Wait for the target node to reconnect to the relay. Use this after a reboot or service restart that may cause the daemon to temporarily disconnect. Waits up to the specified time (default 5 minutes, max 10 minutes).",
    params: WaitForNodeParams,
    handler: |ctx, params| {
        if ctx.instance.is_online().await {
            return Ok(ToolOutput::Text("Node is already online.".to_string()));
        }
        let max = params.max_seconds.unwrap_or(300).min(600);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(max);
        let mut elapsed = 0u64;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            elapsed += 5;
            if ctx.instance.is_online().await {
                return Ok(ToolOutput::Text(format!("Node came back online after {elapsed}s.")));
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(ToolOutput::Text(format!(
                    "Node did not come back online within {max}s. It may need manual intervention."
                )));
            }
        }
    }
}

healer_tool! {
    name: "get_probe_status",
    struct_name: GetProbeStatusTool,
    description: "Query the current health probe status for all services on the target instance. Returns fresh data from the latest heartbeat — use this after applying a fix to verify whether services recovered.",
    handler: |ctx| {
        match ctx.instance_data.get_probe_status(&ctx.instance_id).await {
            Ok(Some(r)) => {
                let age_secs = (chrono::Utc::now() - r.reported_at).num_seconds();
                let services: Vec<serde_json::Value> = r
                    .services_extended
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();

                let mut out = format!("Heartbeat age: {}s\n\n", age_secs);

                if services.is_empty() {
                    out.push_str("No service probe data available.\n");
                } else {
                    out.push_str("Service probes:\n");
                    for svc in &services {
                        let name = svc.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                        let healthy = svc.get("healthy").and_then(|v| v.as_bool());
                        let probe_ok = svc.get("last_probe_ok").and_then(|v| v.as_bool());
                        let kind = svc.get("last_probe_kind").and_then(|v| v.as_str()).unwrap_or("?");
                        let status = match (healthy, probe_ok) {
                            (Some(true), _) => "HEALTHY",
                            (Some(false), _) => "UNHEALTHY",
                            (_, Some(true)) => "PROBE OK",
                            (_, Some(false)) => "PROBE FAILED",
                            _ => "UNKNOWN",
                        };
                        out.push_str(&format!("  - {name}: {status} (last probe: {kind})\n"));
                    }
                }

                if let Some(sample) = r.sample {
                    out.push_str(&format!("\nSystem resources:\n{}", crate::agent::format_sample_summary(&sample)));
                }

                Ok(ToolOutput::Text(out))
            }
            Ok(None) => Ok(ToolOutput::Text("No heartbeat data found for this instance.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error querying probe status: {e}"))),
        }
    }
}

// ── Documentation tools ───────────────────────────────────────────────

#[derive(rust_embed::RustEmbed)]
#[folder = "../server/docs/"]
#[include = "*.md"]
struct DocsAssets;

#[derive(Deserialize, JsonSchema)]
struct ReadDocParams {
    /// Document slug (filename without .md extension, e.g. "configuration-reference", "cluster-setup")
    slug: String,
}

healer_tool! {
    name: "read_doc",
    struct_name: ReadDocTool,
    description: "Read a mac-mgmt platform documentation page by slug. Use `list_docs` first to see available pages.",
    params: ReadDocParams,
    handler: |_ctx, params| {
        let filename = format!("{}.md", params.slug);
        match DocsAssets::get(&filename) {
            Some(file) => {
                let content = std::str::from_utf8(file.data.as_ref())
                    .unwrap_or("(binary content)");
                Ok(ToolOutput::Text(content.to_string()))
            }
            None => {
                let available: Vec<String> = DocsAssets::iter()
                    .map(|f| f.trim_end_matches(".md").to_string())
                    .collect();
                Ok(ToolOutput::Text(format!(
                    "Document '{}' not found. Available: {}",
                    params.slug,
                    available.join(", ")
                )))
            }
        }
    }
}

healer_tool! {
    name: "list_docs",
    struct_name: ListDocsTool,
    description: "List all available mac-mgmt documentation pages. Returns slugs that can be passed to `read_doc`.",
    handler: |_ctx| {
        let docs: Vec<String> = DocsAssets::iter()
            .map(|f| f.trim_end_matches(".md").to_string())
            .collect();
        Ok(ToolOutput::Text(docs.join("\n")))
    }
}

// ── Data query tools ──────────────────────────────────────────────────

healer_tool! {
    name: "get_inventory",
    struct_name: GetInventoryTool,
    description: "Query the latest hardware/software inventory for the target instance. Returns OS, CPU, memory, disks, GPUs, network interfaces, nix version, and security posture. Data is collected every ~6 hours.",
    handler: |ctx| {
        match ctx.instance_data.get_inventory(&ctx.instance_id).await {
            Ok(Some(r)) => {
                let age = chrono::Utc::now() - r.collected_at;
                let mut out = format!("Inventory (collected {}h ago):\n", age.num_hours());
                out.push_str(&serde_json::to_string_pretty(&r.inventory).unwrap_or_default());
                out.push_str("\n\nSecurity posture:\n");
                out.push_str(&serde_json::to_string_pretty(&r.security).unwrap_or_default());
                Ok(ToolOutput::Text(out))
            }
            Ok(None) => Ok(ToolOutput::Text("No inventory data found for this instance. The daemon may not have submitted an assessment yet.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error querying inventory: {e}"))),
        }
    }
}

healer_tool! {
    name: "get_system_sample",
    struct_name: GetSystemSampleTool,
    description: "Query the latest dynamic system sample (CPU load, memory, swap, disk free space, network I/O, process count, thermal state, GPU utilization). Updated with every heartbeat (~30s).",
    handler: |ctx| {
        match ctx.instance_data.get_system_sample(&ctx.instance_id).await {
            Ok(Some(r)) => {
                let age_secs = (chrono::Utc::now() - r.reported_at).num_seconds();
                match r.sample {
                    Some(sample) => {
                        let mut out = format!("System sample ({}s ago):\n", age_secs);
                        out.push_str(&serde_json::to_string_pretty(&sample).unwrap_or_default());
                        Ok(ToolOutput::Text(out))
                    }
                    None => Ok(ToolOutput::Text(format!(
                        "Heartbeat exists ({}s ago) but no sample data attached. Daemon may be an older version.",
                        age_secs
                    ))),
                }
            }
            Ok(None) => Ok(ToolOutput::Text("No heartbeat data found for this instance.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error querying sample: {e}"))),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
struct ProbeHistoryParams {
    /// Service name to query probes for (e.g. "ollama", "openclaw")
    #[serde(default)]
    service: Option<String>,
    /// Maximum number of probe results to return (default 20, max 100)
    #[serde(default)]
    limit: Option<i64>,
}

healer_tool! {
    name: "get_probe_history",
    struct_name: GetProbeHistoryTool,
    description: "Query recent probe results (health checks, functional tests) for the target instance. Optionally filter by service name. Returns timing, success/failure, error details, and LLM token counts.",
    params: ProbeHistoryParams,
    handler: |ctx, params| {
        let limit = params.limit.unwrap_or(20).min(100);

        match ctx.instance_data.get_probe_history(&ctx.instance_id, params.service.as_deref(), limit).await {
            Ok(rows) if rows.is_empty() => {
                Ok(ToolOutput::Text("No probe results found.".to_string()))
            }
            Ok(rows) => {
                let mut out = format!("{} probe result(s):\n\n", rows.len());
                for r in &rows {
                    let status = if r.ok { "OK" } else { "FAIL" };
                    let ts = r.collected_at.format("%Y-%m-%d %H:%M:%S");
                    out.push_str(&format!(
                        "[{ts}] {}: {} ({}, {}ms)",
                        r.service, status, r.kind, r.duration_ms
                    ));
                    if let Some(model) = &r.model {
                        out.push_str(&format!(" model={model}"));
                    }
                    if let (Some(tin), Some(tout)) = (r.tokens_in, r.tokens_out) {
                        out.push_str(&format!(" tokens={tin}/{tout}"));
                    }
                    if let Some(ttft) = r.first_token_ms {
                        out.push_str(&format!(" ttft={ttft}ms"));
                    }
                    if let Some(ec) = &r.error_class {
                        out.push_str(&format!(" error={ec}"));
                    }
                    if let Some(ed) = &r.error_detail {
                        let short = if ed.len() > 200 { &ed[..200] } else { ed };
                        out.push_str(&format!(": {short}"));
                    }
                    out.push('\n');
                }
                Ok(ToolOutput::Text(out))
            }
            Err(e) => Ok(ToolOutput::Text(format!("Error querying probes: {e}"))),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
struct MetricsQueryParams {
    /// Optional substring filter — only return metric lines containing this string.
    /// Useful for narrowing output (e.g. "ollama", "gpu", "http_request").
    #[serde(default)]
    filter: Option<String>,
}

healer_tool! {
    name: "get_metrics",
    struct_name: GetMetricsTool,
    description: "Fetch Prometheus metrics from the relay's federation endpoint. Returns metrics from all connected daemons (labelled with instance_id/hostname). Use the filter parameter to narrow output to specific metric names or labels.",
    params: MetricsQueryParams,
    handler: |ctx, params| {
        let Some(metrics_url) = &ctx.metrics_url else {
            return Ok(ToolOutput::Text("Metrics not available in this mode.".to_string()));
        };
        let client = reqwest::Client::new();
        let resp = client
            .get(metrics_url)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await;
        match resp {
            Ok(r) if r.status().is_success() => {
                let body = r.text().await.unwrap_or_default();
                match &params.filter {
                    Some(f) if !f.is_empty() => {
                        let filtered: String = body
                            .lines()
                            .filter(|l| l.contains(f.as_str()) || l.starts_with("# "))
                            .collect::<Vec<_>>()
                            .join("\n");
                        if filtered.lines().all(|l| l.starts_with('#')) {
                            Ok(ToolOutput::Text(format!("No metrics matching filter '{f}'.")))
                        } else {
                            Ok(ToolOutput::Text(filtered))
                        }
                    }
                    _ => {
                        // Truncate if too large
                        if body.len() > 50_000 {
                            Ok(ToolOutput::Text(format!(
                                "{}\n\n... (output truncated at 50KB, use filter parameter to narrow)",
                                &body[..50_000]
                            )))
                        } else {
                            Ok(ToolOutput::Text(body))
                        }
                    }
                }
            }
            Ok(r) => Ok(ToolOutput::Text(format!("Metrics endpoint returned {}", r.status()))),
            Err(e) => Ok(ToolOutput::Text(format!("Error fetching metrics: {e}"))),
        }
    }
}

// ── Skill tools ───────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct UseSkillParams {
    /// Skill slug (e.g. "ollama_model_swap", "service_crash_recovery")
    skill: String,
}

healer_tool! {
    name: "use_skill",
    struct_name: UseSkillTool,
    description: "Load a built-in healer skill by slug. Returns a detailed procedure with step-by-step instructions, tool usage patterns, and common pitfalls. Use `list_builtin_skills` to see available skills.",
    params: UseSkillParams,
    handler: |_ctx, params| {
        use crate::agent::skills;
        match skills::get_builtin_skill(&params.skill) {
            Some(skill) => Ok(ToolOutput::Text(skill.content)),
            None => {
                let available: Vec<_> = skills::list_builtin_skills()
                    .iter()
                    .map(|s| s.slug.clone())
                    .collect();
                Ok(ToolOutput::Text(format!(
                    "Unknown skill '{}'. Available skills: {}",
                    params.skill,
                    available.join(", ")
                )))
            }
        }
    }
}

healer_tool! {
    name: "list_builtin_skills",
    struct_name: ListBuiltinSkillsTool,
    description: "List all available built-in healer skills. Each skill provides a detailed procedure for a common remediation task.",
    handler: |_ctx| {
        use crate::agent::skills;
        let mut out = String::from("Available skills:\n\n");
        for skill in skills::list_builtin_skills() {
            if skill.desc.is_empty() {
                out.push_str(&format!("- **{}** (`{}`)\n", skill.name, skill.slug));
            } else {
                out.push_str(&format!("- **{}** (`{}`) — {}\n", skill.name, skill.slug, skill.desc));
            }
        }
        out.push_str("\nUse `use_skill` with the slug to load a skill's full procedure.");
        Ok(ToolOutput::Text(out))
    }
}

/// Create all healer tools for a session.
///
/// When `diagnosis_only` is true, mutating tools (write_file, run_command,
/// run_cluster_command) are omitted — the agent can only observe.
pub fn all_tools(ctx: ToolContext, diagnosis_only: bool) -> Vec<Box<dyn Tool>> {
    let has_cluster = ctx.cluster.is_some();
    let has_metrics = ctx.metrics_url.is_some();

    let mut tools: Vec<Box<dyn Tool>> = vec![
        ListFilesTool::new(ctx.clone()),
        ReadFileTool::new(ctx.clone()),
        FetchLogsTool::new(ctx.clone()),
        ListFileTunnelsTool::new(ctx.clone()),
        ListShellCommandsTool::new(ctx.clone()),
        PinTool::new(ctx.clone()),
        StaffPingTool::new(ctx.clone()),
        ListStaffPingsTool::new(ctx.clone()),
        SetPhaseTool::new(ctx.clone()),
        NameSessionTool::new(ctx.clone()),
        GetProbeStatusTool::new(ctx.clone()),
        GetInventoryTool::new(ctx.clone()),
        GetSystemSampleTool::new(ctx.clone()),
        GetProbeHistoryTool::new(ctx.clone()),
        ReadDocTool::new(ctx.clone()),
        ListDocsTool::new(ctx.clone()),
        UseSkillTool::new(ctx.clone()),
        ListBuiltinSkillsTool::new(ctx.clone()),
    ];

    if !diagnosis_only {
        tools.push(WriteFileTool::new(ctx.clone()));
        tools.push(RunCommandTool::new(ctx.clone()));
    }

    if has_cluster {
        tools.push(FetchClusterLogsTool::new(ctx.clone()));
        tools.push(CheckNodeOnlineTool::new(ctx.clone()));
        tools.push(WaitForNodeTool::new(ctx.clone()));
        if !diagnosis_only {
            tools.push(RunClusterCommandTool::new(ctx.clone()));
        }
    }

    if has_metrics {
        tools.push(GetMetricsTool::new(ctx));
    }

    tools
}
