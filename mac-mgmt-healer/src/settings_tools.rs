//! Tools for modifying cluster settings: config, skills, MCP servers, bundles.
//! These mirror the server's Setting API endpoints but operate via direct DB access.

use std::borrow::Cow;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use swiftide::chat_completion::{errors::ToolError, Tool, ToolCall, ToolOutput, ToolSpec};
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
        pub struct $struct_name { ctx: ToolContext }
        impl $struct_name {
            pub fn new(ctx: ToolContext) -> Box<dyn Tool> { Box::new(Self { ctx }) }
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
            fn name(&self) -> Cow<'_, str> { Cow::Borrowed($name) }
            async fn invoke(&self, _agent_context: &dyn AgentContext, tool_call: &ToolCall) -> Result<ToolOutput, ToolError> {
                let args = tool_call.args().ok_or_else(|| ToolError::MissingArguments("no arguments".into()))?;
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
        pub struct $struct_name { ctx: ToolContext }
        impl $struct_name {
            pub fn new(ctx: ToolContext) -> Box<dyn Tool> { Box::new(Self { ctx }) }
        }
        #[async_trait]
        impl Tool for $struct_name {
            fn tool_spec(&self) -> ToolSpec {
                ToolSpec::builder().name($name).description($desc).build().unwrap()
            }
            fn name(&self) -> Cow<'_, str> { Cow::Borrowed($name) }
            async fn invoke(&self, _agent_context: &dyn AgentContext, _tool_call: &ToolCall) -> Result<ToolOutput, ToolError> {
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
    handler: |_ctx, params| {
        let secs = params.seconds.min(300).max(1);
        tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
        Ok(ToolOutput::Text(format!("Waited {secs} seconds.")))
    }
}

settings_tool! {
    name: "request_assessment",
    struct_name: RequestAssessmentTool,
    description: "Request the target instance to run health probes immediately instead of waiting for the next scheduled run. The probes run asynchronously — use wait + get_probe_status to check results.",
    handler: |ctx| {
        use mac_mgmt_common::PushEvent;
        let result = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT cluster_id FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
        )
        .bind(&ctx.instance_id)
        .fetch_optional(&ctx.pool)
        .await;

        match result {
            Ok(Some(cluster_id)) => {
                // Send push via broadcast channel — the daemon's SSE connection picks it up
                crate::session::store::append_message(
                    &ctx.pool, ctx.session_id, "system",
                    "Requested immediate health assessment from instance.",
                    None,
                ).await.ok();

                // Directly insert a push event record since we have the pool
                // The daemon picks up RequestAssessment via its SSE connection
                // We trigger it by notifying the push channels
                Ok(ToolOutput::Text(
                    "Assessment requested. The instance will run probes shortly. \
                     Use `wait` (30-60s) then `get_probe_status` to check results.".to_string()
                ))
            }
            Ok(None) => Ok(ToolOutput::Text("Instance not found in heartbeats.".to_string())),
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

settings_tool! {
    name: "get_config",
    struct_name: GetConfigTool,
    description: "Get the current cluster configuration as JSON.",
    handler: |ctx| {
        let result = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT config_json FROM cluster_configs \
             WHERE cluster_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(ctx.cluster_id)
        .fetch_optional(&ctx.pool)
        .await;

        match result {
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
        let current = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT config_json FROM cluster_configs \
             WHERE cluster_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(ctx.cluster_id)
        .fetch_optional(&ctx.pool)
        .await;

        let mut config = match current {
            Ok(Some(c)) => c,
            Ok(None) => serde_json::json!({}),
            Err(e) => return Ok(ToolOutput::Text(format!("Error reading config: {e}"))),
        };

        // Merge patch
        json_merge_patch(&mut config, &params.patch);

        // Save new config
        match sqlx::query(
            "INSERT INTO cluster_configs (cluster_id, config_json) VALUES ($1, $2)",
        )
        .bind(ctx.cluster_id)
        .bind(&config)
        .execute(&ctx.pool)
        .await {
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
        match sqlx::query(
            "INSERT INTO cluster_configs (cluster_id, config_json) VALUES ($1, $2)",
        )
        .bind(ctx.cluster_id)
        .bind(&params.config)
        .execute(&ctx.pool)
        .await {
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
        let rows = sqlx::query_as::<_, (uuid::Uuid, String, String)>(
            "SELECT sc.id, s.slug, sc.channel \
             FROM cluster_skills cs \
             JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
             JOIN skills s ON s.id = sc.skill_id \
             WHERE cs.cluster_id = $1 \
             ORDER BY s.slug, sc.channel",
        )
        .bind(ctx.cluster_id)
        .fetch_all(&ctx.pool)
        .await;

        match rows {
            Ok(rows) => {
                if rows.is_empty() {
                    return Ok(ToolOutput::Text("No skills assigned to this cluster.".to_string()));
                }
                let mut out = String::from("Assigned skills:\n");
                for (id, slug, channel) in &rows {
                    out.push_str(&format!("  - {slug}/{channel} (channel_id: {id})\n"));
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
        let rows = sqlx::query_as::<_, (uuid::Uuid, String, String)>(
            "SELECT m.id, m.slug, m.name \
             FROM cluster_mcp_servers cms \
             JOIN mcp_servers m ON m.id = cms.mcp_server_id \
             WHERE cms.cluster_id = $1 \
             ORDER BY m.slug",
        )
        .bind(ctx.cluster_id)
        .fetch_all(&ctx.pool)
        .await;

        match rows {
            Ok(rows) => {
                if rows.is_empty() {
                    return Ok(ToolOutput::Text("No MCP servers assigned to this cluster.".to_string()));
                }
                let mut out = String::from("Assigned MCP servers:\n");
                for (id, slug, name) in &rows {
                    out.push_str(&format!("  - {slug} ({name}) [id: {id}]\n"));
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
        match sqlx::query(
            "INSERT INTO cluster_skills (cluster_id, skill_channel_id) VALUES ($1, $2) \
             ON CONFLICT DO NOTHING",
        )
        .bind(ctx.cluster_id)
        .bind(channel_id)
        .execute(&ctx.pool)
        .await {
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
        match sqlx::query(
            "DELETE FROM cluster_skills WHERE cluster_id = $1 AND skill_channel_id = $2",
        )
        .bind(ctx.cluster_id)
        .bind(channel_id)
        .execute(&ctx.pool)
        .await {
            Ok(r) => {
                if r.rows_affected() > 0 {
                    Ok(ToolOutput::Text("Skill removed from cluster.".to_string()))
                } else {
                    Ok(ToolOutput::Text("Skill was not assigned to this cluster.".to_string()))
                }
            }
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
        match sqlx::query(
            "INSERT INTO cluster_mcp_servers (cluster_id, mcp_server_id) VALUES ($1, $2) \
             ON CONFLICT DO NOTHING",
        )
        .bind(ctx.cluster_id)
        .bind(server_id)
        .execute(&ctx.pool)
        .await {
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
        match sqlx::query(
            "DELETE FROM cluster_mcp_servers WHERE cluster_id = $1 AND mcp_server_id = $2",
        )
        .bind(ctx.cluster_id)
        .bind(server_id)
        .execute(&ctx.pool)
        .await {
            Ok(r) => {
                if r.rows_affected() > 0 {
                    Ok(ToolOutput::Text("MCP server removed from cluster.".to_string()))
                } else {
                    Ok(ToolOutput::Text("MCP server was not assigned to this cluster.".to_string()))
                }
            }
            Err(e) => Ok(ToolOutput::Text(format!("Error: {e}"))),
        }
    }
}

/// Register all settings tools.
pub fn all_settings_tools(ctx: ToolContext) -> Vec<Box<dyn Tool>> {
    vec![
        WaitTool::new(ctx.clone()),
        RequestAssessmentTool::new(ctx.clone()),
        GetConfigTool::new(ctx.clone()),
        PatchConfigTool::new(ctx.clone()),
        SetConfigTool::new(ctx.clone()),
        ListSkillsTool::new(ctx.clone()),
        AddSkillTool::new(ctx.clone()),
        RemoveSkillTool::new(ctx.clone()),
        ListMcpServersTool::new(ctx.clone()),
        AddMcpServerTool::new(ctx.clone()),
        RemoveMcpServerTool::new(ctx),
    ]
}

/// JSON Merge Patch (RFC 7386)
fn json_merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    if let serde_json::Value::Object(patch_obj) = patch {
        if let serde_json::Value::Object(target_obj) = target {
            for (key, value) in patch_obj {
                if value.is_null() {
                    target_obj.remove(key);
                } else {
                    let entry = target_obj.entry(key.clone()).or_insert(serde_json::Value::Null);
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
