use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use crate::state::SharedState;
use crate::types::*;

const MAX_OUTPUT_BYTES: usize = 100 * 1024; // 100 KB

#[derive(Clone)]
pub struct ExecutorServer {
    state: SharedState,
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
}

impl ExecutorServer {
    pub fn new(state: SharedState) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler]
impl ServerHandler for ExecutorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "MCP server providing ephemeral Incus containers for code execution. \
                 Use os_list to discover available operating systems, system_create to \
                 spin up a container, and system_execute to run commands in it."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tool_router]
impl ExecutorServer {
    #[tool(
        name = "os_list",
        description = "List available OS images for container creation. Returns image aliases (like 'ubuntu/24.04', 'alpine/3.21') with descriptions. Use the 'filter' parameter to narrow results by distro name. Call this first if unsure which OS to use. Common choices: ubuntu/24.04 (general purpose, apt), alpine/3.21 (minimal, apk), debian/12 (stable, apt), fedora/42 (cutting edge, dnf)."
    )]
    async fn os_list(
        &self,
        Parameters(params): Parameters<OsListParams>,
    ) -> String {
        let images = match self.state.get_or_fetch_images().await {
            Ok(imgs) => imgs,
            Err(e) => return format!("Error listing images: {e}"),
        };

        let filtered: Vec<_> = match &params.filter {
            Some(f) => {
                let f = f.to_lowercase();
                images
                    .iter()
                    .filter(|img| {
                        img.alias.to_lowercase().contains(&f)
                            || img.os.to_lowercase().contains(&f)
                            || img.description.to_lowercase().contains(&f)
                    })
                    .collect()
            }
            None => images.iter().collect(),
        };

        if filtered.is_empty() {
            return match &params.filter {
                Some(f) => format!("No images matching '{f}'. Try a broader filter or omit the filter to see all images."),
                None => "No images found.".to_string(),
            };
        }

        let mut out = format!("Available images ({} results):\n\n", filtered.len());
        for img in &filtered {
            out.push_str(&format!(
                "  {:<30} {}\n",
                img.alias, img.description
            ));
        }
        out.push_str("\nUse the alias (first column) as the 'os' parameter in system_create.");
        out
    }

    #[tool(
        name = "system_create",
        description = "Create a new ephemeral Incus container with the specified OS. The container starts immediately and is ready for commands. Use os_list to find valid image aliases. Ephemeral containers are automatically cleaned up when stopped or when this session ends. Returns the container name for use with system_execute."
    )]
    async fn system_create(
        &self,
        Parameters(params): Parameters<SystemCreateParams>,
    ) -> String {
        let name = match params.name {
            Some(n) => {
                let prefix = self.state.session_prefix().await;
                if n.starts_with(&prefix) {
                    n
                } else {
                    format!("{prefix}{n}")
                }
            }
            None => self.state.generate_name().await,
        };

        tracing::info!("creating container {name} with image {}", params.os);

        if let Err(e) = self.state.backend.launch(&params.os, &name).await {
            return format!("Error creating container: {e}");
        }

        let info = InstanceInfo {
            name: name.clone(),
            image: params.os.clone(),
            created_at: chrono::Utc::now(),
        };
        self.state.add_instance(info).await;

        format!(
            "Container '{name}' created and running (image: {}).\n\
             You can now use system_execute to run commands in it.",
            params.os
        )
    }

    #[tool(
        name = "system_execute",
        description = "Execute a shell command inside a container. The command runs via 'sh -c' so pipes, redirects, and shell features work. Returns stdout, stderr, and exit code. If 'name' is omitted, uses the most recently created container. Default timeout is 120 seconds, max 600."
    )]
    async fn system_execute(
        &self,
        Parameters(params): Parameters<SystemExecuteParams>,
    ) -> String {
        let name = match self.state.resolve_name(params.name.as_deref()).await {
            Ok(n) => n,
            Err(e) => return format!("Error: {e}"),
        };

        let timeout = params.effective_timeout();
        tracing::info!("executing in {name}: {}", params.command);

        match self.state.backend.exec(&name, &params.command, timeout).await {
            Ok(output) => {
                let mut result = String::new();

                if !output.stdout.is_empty() {
                    let stdout = truncate_output(&output.stdout, MAX_OUTPUT_BYTES);
                    result.push_str(&stdout);
                }

                if !output.stderr.is_empty() {
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    let stderr = truncate_output(&output.stderr, MAX_OUTPUT_BYTES);
                    result.push_str(&format!("[stderr]\n{stderr}"));
                }

                if output.exit_code != 0 {
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    result.push_str(&format!("[exit code: {}]", output.exit_code));
                }

                if result.is_empty() {
                    "(no output)".to_string()
                } else {
                    result
                }
            }
            Err(e) => format!("Error executing command: {e}"),
        }
    }

    #[tool(
        name = "system_destroy",
        description = "Explicitly destroy a container. Useful for freeing resources before session end. All containers are also auto-cleaned on session shutdown."
    )]
    async fn system_destroy(
        &self,
        Parameters(params): Parameters<SystemDestroyParams>,
    ) -> String {
        let info = self.state.remove_instance(&params.name).await;
        if info.is_none() {
            return format!("No container named '{}' in this session.", params.name);
        }

        tracing::info!("destroying container {}", params.name);

        if let Err(e) = self.state.backend.delete(&params.name).await {
            return format!("Error destroying container: {e}");
        }

        format!("Container '{}' destroyed.", params.name)
    }

    #[tool(
        name = "system_list",
        description = "List all containers managed by this MCP session with their status, OS image, and creation time."
    )]
    async fn system_list(&self) -> String {
        let instances = self.state.list_instances().await;

        if instances.is_empty() {
            return "No containers in this session. Use system_create to create one.".to_string();
        }

        let mut out = format!("Active containers ({}):\n\n", instances.len());
        for inst in &instances {
            let status = self
                .state
                .backend
                .status(&inst.name)
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "Unknown".to_string());
            out.push_str(&format!(
                "  {:<30} image={:<20} status={:<10} created={}\n",
                inst.name,
                inst.image,
                status,
                inst.created_at.format("%H:%M:%S UTC"),
            ));
        }
        out
    }

    #[tool(
        name = "system_file_write",
        description = "Write content to a file inside a container. Creates parent directories as needed. Use this to place code files, configs, or scripts before executing them."
    )]
    async fn system_file_write(
        &self,
        Parameters(params): Parameters<SystemFileWriteParams>,
    ) -> String {
        let name = match self.state.resolve_name(params.name.as_deref()).await {
            Ok(n) => n,
            Err(e) => return format!("Error: {e}"),
        };

        tracing::info!("writing file {} in {name}", params.path);

        match self
            .state
            .backend
            .file_push(&name, &params.path, params.content.as_bytes())
            .await
        {
            Ok(()) => format!(
                "File written: {} ({} bytes)",
                params.path,
                params.content.len()
            ),
            Err(e) => format!("Error writing file: {e}"),
        }
    }

    #[tool(
        name = "system_file_read",
        description = "Read the contents of a file from inside a container. Returns the file content as text."
    )]
    async fn system_file_read(
        &self,
        Parameters(params): Parameters<SystemFileReadParams>,
    ) -> String {
        let name = match self.state.resolve_name(params.name.as_deref()).await {
            Ok(n) => n,
            Err(e) => return format!("Error: {e}"),
        };

        tracing::info!("reading file {} from {name}", params.path);

        match self.state.backend.file_pull(&name, &params.path).await {
            Ok(content) => truncate_output(&content, MAX_OUTPUT_BYTES),
            Err(e) => format!("Error reading file: {e}"),
        }
    }
}

fn truncate_output(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        s.to_string()
    } else {
        let truncated = &s[..max_bytes];
        // Find the last valid UTF-8 char boundary.
        let end = truncated
            .char_indices()
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        format!(
            "{}\n\n[output truncated: showing {end} of {} bytes]",
            &s[..end],
            s.len()
        )
    }
}
