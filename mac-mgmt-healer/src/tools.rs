//! Native swiftide tools wrapping the relay client.
//! Registered directly on the agent — no MCP transport needed.

use std::borrow::Cow;
use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use swiftide::chat_completion::{errors::ToolError, Tool, ToolCall, ToolOutput, ToolSpec};
use swiftide::traits::AgentContext;

use crate::relay_client::RelayClient;

/// Shared context for all healer tools.
#[derive(Clone)]
pub struct ToolContext {
    pub relay: Arc<RelayClient>,
    pub target_instance: String,
    pub cluster_instances: Vec<String>,
    pub file_tunnels: Vec<String>,
    pub shell_commands: Vec<String>,
    pub pool: sqlx::PgPool,
    pub session_id: uuid::Uuid,
    pub cluster_id: uuid::Uuid,
    pub instance_id: String,
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
                        serde_json::from_value::<schemars::Schema>(serde_json::to_value(&schema).unwrap()).unwrap(),
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
        match ctx.relay.file_list(&ctx.target_instance, &params.tunnel_name, params.path.as_deref()).await {
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
        match ctx.relay.file_read(&ctx.target_instance, &params.tunnel_name, &params.path).await {
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
        match ctx.relay.file_write(&ctx.target_instance, &params.tunnel_name, &params.path, params.content.as_bytes(), params.expected_mtime).await {
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
        match ctx.relay.shell_exec(&ctx.target_instance, &params.command_name, params.user_arg.as_deref()).await {
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
        match ctx.relay.log_fetch(&ctx.target_instance, params.n.or(Some(200)), params.service.as_deref(), None).await {
            Ok(val) => Ok(ToolOutput::Text(serde_json::to_string_pretty(&val).unwrap_or_default())),
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

healer_tool! {
    name: "list_file_tunnels",
    struct_name: ListFileTunnelsTool,
    description: "List all available file tunnels on the target instance",
    handler: |ctx| {
        Ok(ToolOutput::Text(serde_json::to_string_pretty(&ctx.file_tunnels).unwrap_or_default()))
    }
}

healer_tool! {
    name: "list_shell_commands",
    struct_name: ListShellCommandsTool,
    description: "List all available shell commands on the target instance",
    handler: |ctx| {
        Ok(ToolOutput::Text(serde_json::to_string_pretty(&ctx.shell_commands).unwrap_or_default()))
    }
}

healer_tool! {
    name: "fetch_cluster_logs",
    struct_name: FetchClusterLogsTool,
    description: "Fetch logs from a different instance in the same cluster",
    params: ClusterLogsParams,
    handler: |ctx, params| {
        if !ctx.cluster_instances.contains(&params.instance_prefix) {
            return Ok(ToolOutput::Text(format!(
                "Error: instance '{}' is not in this cluster. Available: {:?}",
                params.instance_prefix, ctx.cluster_instances
            )));
        }
        match ctx.relay.log_fetch(&params.instance_prefix, params.n.or(Some(200)), params.service.as_deref(), None).await {
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
        if !ctx.cluster_instances.contains(&params.instance_prefix) {
            return Ok(ToolOutput::Text(format!(
                "Error: instance '{}' is not in this cluster. Available: {:?}",
                params.instance_prefix, ctx.cluster_instances
            )));
        }
        match ctx.relay.shell_exec(&params.instance_prefix, &params.command_name, params.user_arg.as_deref()).await {
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
        crate::session::store::append_message(
            &ctx.pool,
            ctx.session_id,
            "pin",
            &serde_json::to_string(&serde_json::json!({
                "slot": slot,
                "summary": params.summary,
                "affected_services": params.affected_services,
            })).unwrap_or_default(),
            Some(&data),
        ).await.ok();
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
        match crate::session::store::create_staff_ping(
            &ctx.pool,
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
        let data = serde_json::json!({ "reason": params.reason });
        match crate::session::store::transition_state(
            &ctx.pool,
            ctx.session_id,
            &new_state,
            &data,
        ).await {
            Ok(()) => Ok(ToolOutput::Text(format!("Phase set to: {}", params.phase))),
            Err(e) => Ok(ToolOutput::Text(format!("Error setting phase: {e}"))),
        }
    }
}

healer_tool! {
    name: "check_node_online",
    struct_name: CheckNodeOnlineTool,
    description: "Check if the target node is currently connected to the relay. Returns online/offline status.",
    handler: |ctx| {
        let online = ctx.relay.is_daemon_online(&ctx.target_instance).await;
        Ok(ToolOutput::Text(if online {
            "Node is online and reachable through the relay.".to_string()
        } else {
            "Node is OFFLINE — not connected to the relay.".to_string()
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
        if ctx.relay.is_daemon_online(&ctx.target_instance).await {
            return Ok(ToolOutput::Text("Node is already online.".to_string()));
        }
        let max = params.max_seconds.unwrap_or(300).min(600);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(max);
        let mut elapsed = 0u64;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            elapsed += 5;
            if ctx.relay.is_daemon_online(&ctx.target_instance).await {
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
        #[derive(sqlx::FromRow)]
        struct Row {
            services_extended: Option<serde_json::Value>,
            sample: Option<serde_json::Value>,
            reported_at: chrono::DateTime<chrono::Utc>,
        }
        let row = sqlx::query_as::<_, Row>(
            "SELECT services_extended, sample, reported_at \
             FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
        )
        .bind(&ctx.instance_id)
        .fetch_optional(&ctx.pool)
        .await;

        match row {
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

/// Create all healer tools for a session.
pub fn all_tools(ctx: ToolContext) -> Vec<Box<dyn Tool>> {
    vec![
        ListFilesTool::new(ctx.clone()),
        ReadFileTool::new(ctx.clone()),
        WriteFileTool::new(ctx.clone()),
        RunCommandTool::new(ctx.clone()),
        FetchLogsTool::new(ctx.clone()),
        ListFileTunnelsTool::new(ctx.clone()),
        ListShellCommandsTool::new(ctx.clone()),
        FetchClusterLogsTool::new(ctx.clone()),
        RunClusterCommandTool::new(ctx.clone()),
        PinTool::new(ctx.clone()),
        StaffPingTool::new(ctx.clone()),
        SetPhaseTool::new(ctx.clone()),
        CheckNodeOnlineTool::new(ctx.clone()),
        WaitForNodeTool::new(ctx.clone()),
        GetProbeStatusTool::new(ctx),
    ]
}
