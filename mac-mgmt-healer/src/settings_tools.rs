//! Tools for modifying cluster settings: config, skills, MCP servers, bundles.
//! These mirror the server's Setting API endpoints but operate via direct DB access.

use std::borrow::Cow;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use swiftide::chat_completion::{Tool, ToolCall, ToolOutput, ToolSpec, errors::ToolError};
use swiftide::traits::AgentContext;

use crate::tools::ToolContext;

// Re-use the same macro from tools.rs
macro_rules! settings_tool {
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
                let $params_var: $params_ty = serde_json::from_str(&args)
                    .map_err(|e| ToolError::MissingArguments(e.to_string().into()))?;
                let $ctx_var = &self.ctx;
                $body
            }
        }
    };
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
struct WaitParams {
    /// Seconds to wait (1-300)
    seconds: u64,
}

#[derive(Deserialize, JsonSchema)]
struct PatchConfigParams {
    /// JSON patch to merge into the cluster config. Use JSON Merge Patch format.
    /// Example: {"ollama": {"port": 11435}} to change the ollama port.
    patch: serde_json::Value,
}

#[derive(Deserialize, JsonSchema)]
struct SetConfigParams {
    /// Full cluster config as JSON. Replaces the entire config.
    config: serde_json::Value,
}

#[derive(Deserialize, JsonSchema)]
struct AddSkillParams {
    /// Skill channel ID (UUID) to add to the cluster
    skill_channel_id: String,
}

#[derive(Deserialize, JsonSchema)]
struct RemoveSkillParams {
    /// Skill channel ID (UUID) to remove from the cluster
    skill_channel_id: String,
}

#[derive(Deserialize, JsonSchema)]
struct AddMcpServerParams {
    /// MCP server ID (UUID) to add to the cluster
    mcp_server_id: String,
}

#[derive(Deserialize, JsonSchema)]
struct RemoveMcpServerParams {
    /// MCP server ID (UUID) to remove from the cluster
    mcp_server_id: String,
}

// ── Tool implementations ───────────────────────────────────────────────

settings_tool! {
    name: "wait",
    struct_name: WaitTool,
    description: "Wait for a specified number of seconds (1-300). Use this when you need to wait for a service to restart, a config reload to take effect, or probes to run.",
    params: WaitParams,
    handler: |ctx, params| {
        let secs = params.seconds.min(300).max(1);
        let _ = ctx.events_tx.send(crate::session::HealerEvent::Status {
            message: format!("Waiting {secs}s..."),
        });
        let mut elapsed = 0u64;
        while elapsed < secs {
            let chunk = (secs - elapsed).min(10);
            tokio::time::sleep(std::time::Duration::from_secs(chunk)).await;
            elapsed += chunk;
            if elapsed < secs {
                let remaining = secs - elapsed;
                let _ = ctx.events_tx.send(crate::session::HealerEvent::Status {
                    message: format!("Waiting... {remaining}s remaining"),
                });
            }
        }
        let _ = ctx.events_tx.send(crate::session::HealerEvent::Status {
            message: format!("Wait complete ({secs}s)."),
        });
        Ok(ToolOutput::Text(format!("Waited {secs} seconds.")))
    }
}

settings_tool! {
    name: "request_assessment",
    struct_name: RequestAssessmentTool,
    description: "Request the target instance to run health probes immediately instead of waiting for the next scheduled run. Sends a push event via SSE to the daemon. Use wait (30-60s) then get_probe_status to check results.",
    handler: |ctx| {
        if let Some(push_fn) = &ctx.push_fn {
            push_fn(ctx.cluster_id, mac_mgmt_common::PushEvent::RequestAssessment);
            Ok(ToolOutput::Text(
                "Assessment requested. The instance will run probes shortly. \
                 Use `wait` (30-60s) then `get_probe_status` to check results.".to_string()
            ))
        } else {
            Ok(ToolOutput::Text("Push not available — assessment cannot be triggered remotely.".to_string()))
        }
    }
}

settings_tool! {
    name: "get_config",
    struct_name: GetConfigTool,
    description: "Get the current cluster configuration as JSON.",
    handler: |ctx| {
        match ctx.store.get_config(ctx.cluster_id).await {
            Ok(Some(config)) => Ok(ToolOutput::Text(
                serde_json::to_string_pretty(&config).unwrap_or_default()
            )),
            Ok(None) => Ok(ToolOutput::Text("No config found for this cluster.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

settings_tool! {
    name: "patch_config",
    struct_name: PatchConfigTool,
    description: "Merge a JSON patch into the cluster config. Use JSON Merge Patch format — only include fields you want to change. Example: {\"ollama\": {\"port\": 11435}}",
    params: PatchConfigParams,
    handler: |ctx, params| {
        // Get current config
        let mut config = match ctx.store.get_config(ctx.cluster_id).await {
            Ok(Some(c)) => c,
            Ok(None) => serde_json::json!({}),
            Err(e) => return Ok(ToolOutput::Text(format!("Error reading config: {e}"))),
        };

        // Merge patch
        json_merge_patch(&mut config, &params.patch);
        mac_mgmt_common::config_migrate::migrate(&mut config);

        // Validate merged config before saving
        if let Err(e) = serde_json::from_value::<mac_mgmt_common::ClusterConfig>(config.clone()) {
            return Ok(ToolOutput::Text(format!("Invalid config after merge: {e}")));
        }

        // Save new config
        match ctx.store.save_config(ctx.cluster_id, &config).await {
            Ok(_) => Ok(ToolOutput::Text("Config patched successfully. The daemon will pick up changes on next sync.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error saving config: {e}"))),
        }
    }
}

settings_tool! {
    name: "set_config",
    struct_name: SetConfigTool,
    description: "Replace the entire cluster configuration with new JSON. Use get_config first to see the current config, then modify and set.",
    params: SetConfigParams,
    handler: |ctx, params| {
        let mut config = params.config;
        mac_mgmt_common::config_migrate::migrate(&mut config);

        // Validate before saving
        if let Err(e) = serde_json::from_value::<mac_mgmt_common::ClusterConfig>(config.clone()) {
            return Ok(ToolOutput::Text(format!("Invalid config: {e}")));
        }

        match ctx.store.save_config(ctx.cluster_id, &config).await {
            Ok(_) => Ok(ToolOutput::Text("Config replaced. The daemon will pick up changes on next sync.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error saving config: {e}"))),
        }
    }
}

settings_tool! {
    name: "list_skills",
    struct_name: ListSkillsTool,
    description: "List skills currently assigned to this cluster.",
    handler: |ctx| {
        match ctx.store.list_skills(ctx.cluster_id).await {
            Ok(rows) => {
                if rows.is_empty() {
                    return Ok(ToolOutput::Text("No skills assigned to this cluster.".to_string()));
                }
                let mut out = String::from("Assigned skills:\n");
                for entry in &rows {
                    out.push_str(&format!("  - {}/{} (channel_id: {})\n", entry.slug, entry.channel, entry.id));
                }
                Ok(ToolOutput::Text(out))
            }
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

settings_tool! {
    name: "list_mcp_servers",
    struct_name: ListMcpServersTool,
    description: "List MCP servers currently assigned to this cluster.",
    handler: |ctx| {
        match ctx.store.list_mcp_servers(ctx.cluster_id).await {
            Ok(rows) => {
                if rows.is_empty() {
                    return Ok(ToolOutput::Text("No MCP servers assigned to this cluster.".to_string()));
                }
                let mut out = String::from("Assigned MCP servers:\n");
                for entry in &rows {
                    out.push_str(&format!("  - {} ({}) [id: {}]\n", entry.slug, entry.name, entry.id));
                }
                Ok(ToolOutput::Text(out))
            }
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

settings_tool! {
    name: "add_skill",
    struct_name: AddSkillTool,
    description: "Add a skill channel to this cluster by its channel ID (UUID). Use list_skills to see currently assigned skills.",
    params: AddSkillParams,
    handler: |ctx, params| {
        let channel_id: uuid::Uuid = match params.skill_channel_id.parse() {
            Ok(id) => id,
            Err(_) => return Ok(ToolOutput::Text("Invalid UUID for skill_channel_id.".to_string())),
        };
        match ctx.store.add_skill(ctx.cluster_id, channel_id).await {
            Ok(_) => Ok(ToolOutput::Text("Skill added to cluster.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

settings_tool! {
    name: "remove_skill",
    struct_name: RemoveSkillTool,
    description: "Remove a skill channel from this cluster by its channel ID (UUID).",
    params: RemoveSkillParams,
    handler: |ctx, params| {
        let channel_id: uuid::Uuid = match params.skill_channel_id.parse() {
            Ok(id) => id,
            Err(_) => return Ok(ToolOutput::Text("Invalid UUID.".to_string())),
        };
        match ctx.store.remove_skill(ctx.cluster_id, channel_id).await {
            Ok(true) => Ok(ToolOutput::Text("Skill removed from cluster.".to_string())),
            Ok(false) => Ok(ToolOutput::Text("Skill was not assigned to this cluster.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

settings_tool! {
    name: "add_mcp_server",
    struct_name: AddMcpServerTool,
    description: "Add an MCP server to this cluster by its ID (UUID). Use list_mcp_servers to see currently assigned servers.",
    params: AddMcpServerParams,
    handler: |ctx, params| {
        let server_id: uuid::Uuid = match params.mcp_server_id.parse() {
            Ok(id) => id,
            Err(_) => return Ok(ToolOutput::Text("Invalid UUID.".to_string())),
        };
        match ctx.store.add_mcp_server(ctx.cluster_id, server_id).await {
            Ok(_) => Ok(ToolOutput::Text("MCP server added to cluster.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

settings_tool! {
    name: "remove_mcp_server",
    struct_name: RemoveMcpServerTool,
    description: "Remove an MCP server from this cluster by its ID (UUID).",
    params: RemoveMcpServerParams,
    handler: |ctx, params| {
        let server_id: uuid::Uuid = match params.mcp_server_id.parse() {
            Ok(id) => id,
            Err(_) => return Ok(ToolOutput::Text("Invalid UUID.".to_string())),
        };
        match ctx.store.remove_mcp_server(ctx.cluster_id, server_id).await {
            Ok(true) => Ok(ToolOutput::Text("MCP server removed from cluster.".to_string())),
            Ok(false) => Ok(ToolOutput::Text("MCP server was not assigned to this cluster.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

/// Register all settings tools.
// ── Push / sync tools ─────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct SendPushParams {
    /// Push event type: sync_config, sync_skills, sync_mcp_servers, sync_ssh_keys,
    /// self_update, sync_nixpkgs, sync_packages, request_assessment
    event: String,
}

settings_tool! {
    name: "send_push",
    struct_name: SendPushTool,
    description: "Send a push event to all daemons in the cluster via SSE. Available events: sync_config (reload config), sync_skills (re-sync skills), sync_mcp_servers (re-sync MCP servers), sync_ssh_keys (re-sync SSH keys), self_update (trigger self-update check), sync_nixpkgs (re-sync nixpkgs pin), sync_packages (re-sync unified nix packages), request_assessment (trigger immediate probe run).",
    params: SendPushParams,
    handler: |ctx, params| {
        let event = match params.event.as_str() {
            "sync_config" => mac_mgmt_common::PushEvent::SyncConfig,
            "sync_skills" => mac_mgmt_common::PushEvent::SyncSkills,
            "sync_mcp_servers" => mac_mgmt_common::PushEvent::SyncMcpServers,
            "sync_ssh_keys" => mac_mgmt_common::PushEvent::SyncSshKeys,
            "self_update" => mac_mgmt_common::PushEvent::SelfUpdate,
            "sync_nixpkgs" => mac_mgmt_common::PushEvent::SyncNixpkgs,
            "sync_packages" => mac_mgmt_common::PushEvent::SyncPackages,
            "request_assessment" => mac_mgmt_common::PushEvent::RequestAssessment,
            other => return Ok(ToolOutput::Text(format!(
                "Unknown event '{other}'. Valid: sync_config, sync_skills, sync_mcp_servers, sync_ssh_keys, self_update, sync_nixpkgs, sync_packages, request_assessment"
            ))),
        };
        if let Some(push_fn) = &ctx.push_fn {
            push_fn(ctx.cluster_id, event);
            Ok(ToolOutput::Text(format!("Push event '{}' sent to cluster.", params.event)))
        } else {
            Ok(ToolOutput::Text("Push not available.".to_string()))
        }
    }
}

// ── Version / heartbeat info tools ────────────────────────────────────

settings_tool! {
    name: "get_version_info",
    struct_name: GetVersionInfoTool,
    description: "Get version information for the target instance: running daemon version and git commit from the heartbeat, plus available daemon versions from the server's version table.",
    handler: |ctx| {
        match ctx.instance_data.get_version_info(&ctx.instance_id).await {
            Ok(info) => {
                let mut out = String::new();
                match &info.heartbeat {
                    Some(h) => {
                        let age = (chrono::Utc::now() - h.reported_at).num_seconds();
                        out.push_str(&format!("Running version: {} (heartbeat {}s ago)\n", h.version, age));
                        if let Some(sha) = &h.git_sha {
                            out.push_str(&format!("Git commit: {sha}\n"));
                        }
                    }
                    None => out.push_str("No heartbeat data found.\n"),
                }

                if info.daemon_versions.is_empty() {
                    out.push_str("\nNo daemon versions registered on server.\n");
                } else {
                    out.push_str("\nAvailable daemon versions:\n");
                    for r in &info.daemon_versions {
                        out.push_str(&format!(
                            "  {} ({}) — {}{}\n",
                            r.version,
                            r.system,
                            r.created_at.format("%Y-%m-%d %H:%M"),
                            r.store_path.as_deref().map(|p| format!(" [{p}]")).unwrap_or_default(),
                        ));
                    }
                }

                Ok(ToolOutput::Text(out))
            }
            Err(e) => Ok(ToolOutput::Text(format!("Error querying version info: {e}"))),
        }
    }
}

settings_tool! {
    name: "get_heartbeat",
    struct_name: GetHeartbeatTool,
    description: "Get the full heartbeat data for the target instance: version, hostname, services, tunnels, file tunnels, shell tunnels, sample, services_extended, relay info, and timing.",
    handler: |ctx| {
        match ctx.instance_data.get_heartbeat_json(&ctx.instance_id).await {
            Ok(Some(json)) => {
                Ok(ToolOutput::Text(serde_json::to_string_pretty(&json).unwrap_or_default()))
            }
            Ok(None) => Ok(ToolOutput::Text("No heartbeat data found for this instance.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error querying heartbeat: {e}"))),
        }
    }
}

// ── Cluster / instance query tools ─────────────────────────────────────

settings_tool! {
    name: "get_cluster_instances",
    struct_name: GetClusterInstancesTool,
    description: "List all instances in the cluster with their status, version, hostname, and last heartbeat time.",
    handler: |ctx| {
        match ctx.instance_data.get_cluster_instances(ctx.cluster_id).await {
            Ok(rows) if rows.is_empty() => {
                Ok(ToolOutput::Text("No instances found in this cluster.".to_string()))
            }
            Ok(rows) => {
                let mut out = format!("{} instance(s):\n\n", rows.len());
                for r in &rows {
                    let age = (chrono::Utc::now() - r.reported_at).num_seconds();
                    let prefix = &r.instance_id[..r.instance_id.len().min(12)];
                    let host = r.hostname.as_deref().unwrap_or("?");
                    let online = age < 120;

                    // Count healthy/unhealthy services
                    let (healthy, total) = r.services_extended
                        .as_ref()
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            let h = arr.iter().filter(|s| s.get("healthy").and_then(|v| v.as_bool()) == Some(true)).count();
                            (h, arr.len())
                        })
                        .unwrap_or((0, 0));

                    let status = if !online { "OFFLINE" } else if healthy == total { "HEALTHY" } else { "DEGRADED" };
                    let is_self = r.instance_id == ctx.instance_id;
                    let marker = if is_self { " (this instance)" } else { "" };

                    out.push_str(&format!(
                        "  {prefix} — {status} v{} {host} ({age}s ago) services={healthy}/{total}{marker}\n",
                        r.version
                    ));
                }
                Ok(ToolOutput::Text(out))
            }
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

settings_tool! {
    name: "get_service_state",
    struct_name: GetServiceStateTool,
    description: "Get detailed per-service state from the latest heartbeat: health, probe results, timing. Use this to decide whether to restart, wait, or escalate.",
    handler: |ctx| {
        match ctx.instance_data.get_service_state(&ctx.instance_id).await {
            Ok(Some(r)) => {
                let age = (chrono::Utc::now() - r.reported_at).num_seconds();
                let mut out = format!("Heartbeat age: {age}s\n\n");

                out.push_str("Services (basic):\n");
                out.push_str(&serde_json::to_string_pretty(&r.services).unwrap_or_default());

                if let Some(ext) = r.services_extended {
                    out.push_str("\n\nServices (extended probes):\n");
                    out.push_str(&serde_json::to_string_pretty(&ext).unwrap_or_default());
                }

                Ok(ToolOutput::Text(out))
            }
            Ok(None) => Ok(ToolOutput::Text("No heartbeat data found.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

settings_tool! {
    name: "nix_check_upgrades",
    struct_name: NixCheckUpgradesTool,
    description: "Check which nix packages have available upgrades by running `nix-profile-list` on the target instance and comparing installed vs available. Returns the raw profile listing.",
    handler: |ctx| {
        // Use the existing nix-profile-list shell command via the instance access trait
        match ctx.instance.shell_exec("nix-profile-list", None).await {
            Ok(output) => {
                let mut text = String::new();
                for line in &output.lines {
                    text.push_str(&format!("{}\n", line.data));
                }
                if let Some(code) = output.exit_code {
                    if code != 0 {
                        text.push_str(&format!("\n[exit code: {code}]"));
                    }
                }
                Ok(ToolOutput::Text(text))
            }
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

/// Create all settings tools for a session.
///
/// When `diagnosis_only` is true, mutating tools (patch_config, set_config,
/// add/remove skill/mcp, send_push, request_assessment) are omitted.
pub fn all_settings_tools(ctx: ToolContext, diagnosis_only: bool) -> Vec<Box<dyn Tool>> {
    let mut tools: Vec<Box<dyn Tool>> = vec![
        WaitTool::new(ctx.clone()),
        GetConfigTool::new(ctx.clone()),
        ListSkillsTool::new(ctx.clone()),
        ListMcpServersTool::new(ctx.clone()),
        GetVersionInfoTool::new(ctx.clone()),
        GetHeartbeatTool::new(ctx.clone()),
        GetClusterInstancesTool::new(ctx.clone()),
        GetServiceStateTool::new(ctx.clone()),
        NixCheckUpgradesTool::new(ctx.clone()),
    ];

    if !diagnosis_only {
        tools.push(RequestAssessmentTool::new(ctx.clone()));
        tools.push(PatchConfigTool::new(ctx.clone()));
        tools.push(SetConfigTool::new(ctx.clone()));
        tools.push(AddSkillTool::new(ctx.clone()));
        tools.push(RemoveSkillTool::new(ctx.clone()));
        tools.push(AddMcpServerTool::new(ctx.clone()));
        tools.push(RemoveMcpServerTool::new(ctx.clone()));
        tools.push(SendPushTool::new(ctx));
    }

    tools
}

/// JSON Merge Patch (RFC 7386)
fn json_merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    if let serde_json::Value::Object(patch_obj) = patch {
        if let serde_json::Value::Object(target_obj) = target {
            for (key, value) in patch_obj {
                if value.is_null() {
                    target_obj.remove(key);
                } else {
                    let entry = target_obj
                        .entry(key.clone())
                        .or_insert(serde_json::Value::Null);
                    json_merge_patch(entry, value);
                }
            }
        } else {
            *target = patch.clone();
        }
    } else {
        *target = patch.clone();
    }
}
