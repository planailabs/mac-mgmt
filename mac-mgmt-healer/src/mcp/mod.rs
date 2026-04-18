pub mod relay_client;
pub mod types;

use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::tool::Parameters,
    model::{CallToolResult, Content, ServerInfo},
    tool, tool_handler, tool_router,
};

use self::relay_client::RelayClient;
use self::types::*;

/// MCP server that exposes relay tunnel tools to the AI agent.
/// Each instance is scoped to a specific target instance and cluster.
pub struct HealerMcpServer {
    relay: Arc<RelayClient>,
    /// The primary target instance's ID prefix (first 12 hex chars).
    target_instance: String,
    /// All instance ID prefixes in the same cluster.
    cluster_instances: Vec<String>,
    /// File tunnel names available on the target instance (from heartbeat data).
    file_tunnels: Vec<String>,
    /// Shell command names available on the target instance.
    shell_commands: Vec<String>,
}

impl HealerMcpServer {
    pub fn new(
        relay: Arc<RelayClient>,
        target_instance: String,
        cluster_instances: Vec<String>,
        file_tunnels: Vec<String>,
        shell_commands: Vec<String>,
    ) -> Self {
        Self {
            relay,
            target_instance,
            cluster_instances,
            file_tunnels,
            shell_commands,
        }
    }
}

#[tool_handler(router = Self::tool_router())]
impl ServerHandler for HealerMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: rmcp::model::Implementation {
                name: "mac-mgmt-healer".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            ..Default::default()
        }
    }
}

#[tool_router]
impl HealerMcpServer {
    #[tool(description = "List files in a file tunnel directory on the target instance")]
    async fn list_files(
        &self,
        Parameters(params): Parameters<ListFilesParams>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .relay
            .file_list(&self.target_instance, &params.tunnel_name, params.path.as_deref())
            .await
        {
            Ok(val) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&val).unwrap_or_default(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Read a configuration file from the target instance via a file tunnel")]
    async fn read_file(
        &self,
        Parameters(params): Parameters<ReadFileParams>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .relay
            .file_read(&self.target_instance, &params.tunnel_name, &params.path)
            .await
        {
            Ok(result) => {
                let text = String::from_utf8_lossy(&result.content);
                let mut output = text.into_owned();
                if let Some(mtime) = result.mtime {
                    output.push_str(&format!("\n\n[mtime: {mtime}]"));
                }
                Ok(CallToolResult::success(vec![Content::text(output)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(
        description = "Write a configuration file to the target instance via a file tunnel. Include expected_mtime from a previous read to detect concurrent modifications."
    )]
    async fn write_file(
        &self,
        Parameters(params): Parameters<WriteFileParams>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .relay
            .file_write(
                &self.target_instance,
                &params.tunnel_name,
                &params.path,
                params.content.as_bytes(),
                params.expected_mtime,
            )
            .await
        {
            Ok(val) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&val).unwrap_or_default(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(
        description = "Execute a predefined shell command on the target instance. Returns stdout, stderr, and exit code."
    )]
    async fn run_command(
        &self,
        Parameters(params): Parameters<RunCommandParams>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .relay
            .shell_exec(
                &self.target_instance,
                &params.command_name,
                params.user_arg.as_deref(),
            )
            .await
        {
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
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(
        description = "Fetch recent logs from the target instance, optionally filtered by service name"
    )]
    async fn fetch_logs(
        &self,
        Parameters(params): Parameters<FetchLogsParams>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .relay
            .log_fetch(
                &self.target_instance,
                params.n.or(Some(200)),
                params.service.as_deref(),
                None,
            )
            .await
        {
            Ok(val) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&val).unwrap_or_default(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "List all available file tunnels on the target instance")]
    async fn list_file_tunnels(
        &self,
    ) -> Result<CallToolResult, McpError> {
        let list = serde_json::to_string_pretty(&self.file_tunnels).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(list)]))
    }

    #[tool(description = "List all available shell commands on the target instance")]
    async fn list_shell_commands(
        &self,
    ) -> Result<CallToolResult, McpError> {
        let list = serde_json::to_string_pretty(&self.shell_commands).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(list)]))
    }

    #[tool(description = "Fetch logs from a different instance in the same cluster")]
    async fn fetch_cluster_logs(
        &self,
        Parameters(params): Parameters<ClusterInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        if !self.cluster_instances.contains(&params.instance_prefix) {
            return Ok(CallToolResult::error(vec![Content::text(format!(
                "Instance '{}' is not in this cluster. Available: {:?}",
                params.instance_prefix, self.cluster_instances
            ))]));
        }
        match self
            .relay
            .log_fetch(
                &params.instance_prefix,
                params.n.or(Some(200)),
                params.service.as_deref(),
                None,
            )
            .await
        {
            Ok(val) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&val).unwrap_or_default(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Run a shell command on a different instance in the same cluster")]
    async fn run_cluster_command(
        &self,
        Parameters(params): Parameters<ClusterCommandParams>,
    ) -> Result<CallToolResult, McpError> {
        if !self.cluster_instances.contains(&params.instance_prefix) {
            return Ok(CallToolResult::error(vec![Content::text(format!(
                "Instance '{}' is not in this cluster. Available: {:?}",
                params.instance_prefix, self.cluster_instances
            ))]));
        }
        match self
            .relay
            .shell_exec(
                &params.instance_prefix,
                &params.command_name,
                params.user_arg.as_deref(),
            )
            .await
        {
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
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }
}
