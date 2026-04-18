//! Native swiftide tools wrapping the relay client.
//! Registered directly on the agent — no MCP transport needed.

use std::borrow::Cow;
use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use swiftide::chat_completion::{errors::ToolError, Tool, ToolCall, ToolOutput, ToolSpec};
use swiftide::traits::AgentContext;

use crate::mcp::relay_client::RelayClient;

/// Shared context for all healer tools.
#[derive(Clone)]
pub struct ToolContext {
    pub relay: Arc<RelayClient>,
    pub target_instance: String,
    pub cluster_instances: Vec<String>,
    pub file_tunnels: Vec<String>,
    pub shell_commands: Vec<String>,
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
        RunClusterCommandTool::new(ctx),
    ]
}
