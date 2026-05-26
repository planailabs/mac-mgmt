//! MCP server exposing healer tools over Streamable HTTP.
//!
//! Mounted at `/mcp/healer` on the axum router. Authenticated via admin API
//! tokens in the `Authorization: Bearer <token>` header.
//!
//! The server exposes meta-tools (list_instances, create_session, end_session,
//! staff ping management) that are always available, plus the full set of healer
//! tools after `create_session` is called for a target instance.

use std::borrow::Cow;
use std::sync::Arc;

use mac_mgmt_healer::instance_access::DynClusterAccess;
use mac_mgmt_healer::instance_data::DynInstanceData;
use mac_mgmt_healer::relay_client::{RelayClient, RelayClusterAccess, RelayInstanceAccess};
use mac_mgmt_healer::store::HealerStore;
use mac_mgmt_healer::store::pg::PgHealerStore;
use mac_mgmt_healer::tools::{PushFn, ToolContext, ToolRisk};
use rmcp::handler::server::tool::{ToolCallContext, ToolRouter};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{Peer, RoleServer, ServerHandler, tool, tool_router};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use swiftide::chat_completion::{Tool as SwiftideTool, ToolCall, ToolOutput};
use swiftide::traits::ToolFeedback;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Alias for rmcp's error type used throughout ServerHandler methods.
type McpError = rmcp::model::ErrorData;

// ── Types ─────────────────────────────────────────────────────────────

/// Per-MCP-session state for the healer MCP server.
pub struct HealerMcpServer {
    pool: PgPool,
    store: Arc<PgHealerStore>,
    instance_data: DynInstanceData,
    push_fn: Option<PushFn>,
    /// Swiftide tools — populated after create_session.
    tools: RwLock<Vec<(Box<dyn SwiftideTool>, ToolRisk)>>,
    /// Cached rmcp tool descriptors for the swiftide tools.
    tool_descriptors: RwLock<Vec<rmcp::model::Tool>>,
    /// Active session context.
    session_ctx: RwLock<Option<SessionContext>>,
    /// MCP peer handle — stored during initialize so we can send
    /// `tools/list_changed` notifications after create/end session.
    peer: RwLock<Option<Peer<RoleServer>>>,
    /// Meta-tool router (always available).
    meta_router: ToolRouter<Self>,
}

struct SessionContext {
    session_id: Uuid,
    cluster_id: Uuid,
    instance_id: String,
    cluster_name: String,
    hostname: String,
    services_extended: Vec<mac_mgmt_common::ServiceExtState>,
    sample_summary: String,
    file_tunnels: Vec<String>,
    shell_commands: Vec<String>,
    other_instances: Vec<mac_mgmt_healer::agent::InstanceInfo>,
    metrics_url: Option<String>,
}

impl HealerMcpServer {
    pub fn new(
        pool: PgPool,
        store: Arc<PgHealerStore>,
        instance_data: DynInstanceData,
        push_fn: Option<PushFn>,
    ) -> Self {
        Self {
            pool,
            store,
            instance_data,
            push_fn,
            tools: RwLock::new(Vec::new()),
            tool_descriptors: RwLock::new(Vec::new()),
            session_ctx: RwLock::new(None),
            peer: RwLock::new(None),
            meta_router: Self::tool_router(),
        }
    }
}

// ── Auth ──────────────────────────────────────────────────────────────

async fn validate_admin_token(pool: &PgPool, token: &str) -> bool {
    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(token.as_bytes()));
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(\
            SELECT 1 FROM tokens \
            WHERE token_hash = $1 AND kind = 'admin' AND NOT revoked \
            AND (expires_at IS NULL OR expires_at > now())\
        )",
    )
    .bind(&hash)
    .fetch_one(pool)
    .await
    .unwrap_or(false)
}

// ── Swiftide → rmcp bridge ───────────────────────────────────────────

fn swiftide_spec_to_rmcp_tool(
    spec: &swiftide::chat_completion::ToolSpec,
    risk: ToolRisk,
) -> rmcp::model::Tool {
    // Convert schemars Schema → JsonObject for input_schema
    let input_schema: JsonObject = if let Some(schema) = &spec.parameters_schema {
        let value = serde_json::to_value(schema).unwrap_or_default();
        match value {
            serde_json::Value::Object(mut map) => {
                // Strip $schema key — MCP doesn't expect it
                map.remove("$schema");
                map
            }
            _ => {
                let mut m = serde_json::Map::new();
                m.insert("type".into(), "object".into());
                m
            }
        }
    } else {
        let mut m = serde_json::Map::new();
        m.insert("type".into(), "object".into());
        m
    };

    let annotations = match risk {
        ToolRisk::ReadOnly => Some(ToolAnnotations::new().read_only(true).destructive(false)),
        ToolRisk::SessionLocal => Some(
            ToolAnnotations::new()
                .read_only(false)
                .destructive(false)
                .idempotent(true),
        ),
        ToolRisk::Mutating => Some(ToolAnnotations::new().read_only(false).destructive(false)),
        ToolRisk::Destructive => Some(ToolAnnotations::new().read_only(false).destructive(true)),
    };

    rmcp::model::Tool {
        name: Cow::Owned(spec.name.clone()),
        title: None,
        description: Some(Cow::Owned(spec.description.clone())),
        input_schema: Arc::new(input_schema),
        output_schema: None,
        annotations,
        execution: None,
        icons: None,
        meta: None,
    }
}

/// No-op AgentContext for invoking swiftide tools.
///
/// All healer tools receive `_agent_context` (unused parameter) so this is safe.
/// If a tool ever starts using the context, it will panic here — which is
/// intentional, as it means the tool needs to be adapted for MCP use.
struct NoOpAgentContext;

#[async_trait::async_trait]
impl swiftide::traits::AgentContext for NoOpAgentContext {
    async fn next_completion(
        &self,
    ) -> anyhow::Result<Option<Vec<swiftide::chat_completion::ChatMessage>>> {
        unimplemented!("NoOpAgentContext: tool should not call next_completion")
    }
    async fn current_new_messages(
        &self,
    ) -> anyhow::Result<Vec<swiftide::chat_completion::ChatMessage>> {
        unimplemented!("NoOpAgentContext: tool should not call current_new_messages")
    }
    async fn add_messages(
        &self,
        _item: Vec<swiftide::chat_completion::ChatMessage>,
    ) -> anyhow::Result<()> {
        unimplemented!("NoOpAgentContext: tool should not call add_messages")
    }
    async fn add_message(
        &self,
        _item: swiftide::chat_completion::ChatMessage,
    ) -> anyhow::Result<()> {
        unimplemented!("NoOpAgentContext: tool should not call add_message")
    }
    #[allow(deprecated)]
    async fn exec_cmd(
        &self,
        _cmd: &swiftide::traits::Command,
    ) -> Result<swiftide::traits::CommandOutput, swiftide::traits::CommandError> {
        unimplemented!("NoOpAgentContext: tool should not call exec_cmd")
    }
    fn executor(&self) -> &Arc<dyn swiftide::traits::ToolExecutor> {
        unimplemented!("NoOpAgentContext: tool should not call executor")
    }
    async fn history(&self) -> anyhow::Result<Vec<swiftide::chat_completion::ChatMessage>> {
        unimplemented!("NoOpAgentContext: tool should not call history")
    }
    async fn replace_history(
        &self,
        _items: Vec<swiftide::chat_completion::ChatMessage>,
    ) -> anyhow::Result<()> {
        unimplemented!("NoOpAgentContext: tool should not call replace_history")
    }
    async fn redrive(&self) -> anyhow::Result<()> {
        unimplemented!("NoOpAgentContext: tool should not call redrive")
    }
    async fn has_received_feedback(&self, _tool_call: &ToolCall) -> Option<ToolFeedback> {
        None
    }
    async fn feedback_received(
        &self,
        _tool_call: &ToolCall,
        _feedback: &ToolFeedback,
    ) -> anyhow::Result<()> {
        unimplemented!("NoOpAgentContext: tool should not call feedback_received")
    }
}

async fn invoke_swiftide_tool(
    tool: &dyn SwiftideTool,
    arguments: Option<&JsonObject>,
) -> Result<CallToolResult, McpError> {
    let args_str = arguments
        .map(|a| serde_json::to_string(a))
        .transpose()
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let tool_call = ToolCall::builder()
        .id(Uuid::new_v4().to_string())
        .name(tool.name().to_string())
        .args(args_str.unwrap_or_else(|| "{}".to_string()))
        .build()
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let noop_ctx = NoOpAgentContext;
    match tool.invoke(&noop_ctx, &tool_call).await {
        Ok(output) => match output {
            ToolOutput::Text(text) => Ok(CallToolResult::success(vec![Content::text(text)])),
            ToolOutput::Fail(text) => Ok(CallToolResult::error(vec![Content::text(text)])),
            ToolOutput::Stop(val) => {
                let text = val
                    .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
                    .unwrap_or_else(|| "Session stopped".to_string());
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            ToolOutput::AgentFailed(val) => {
                let text = val
                    .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
                    .unwrap_or_else(|| "Agent failed".to_string());
                Ok(CallToolResult::error(vec![Content::text(text)]))
            }
            ToolOutput::FeedbackRequired(val) => {
                let text = val
                    .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
                    .unwrap_or_else(|| "Feedback required".to_string());
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            _ => Ok(CallToolResult::success(vec![Content::text(
                "Tool returned unknown output type",
            )])),
        },
        Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
    }
}

// ── Meta-tool parameter types ─────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListInstancesParams {
    /// Optional cluster ID to filter by.
    #[serde(default)]
    cluster_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateSessionParams {
    /// Instance ID to target.
    instance_id: String,
    /// Cluster ID the instance belongs to.
    cluster_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListStaffPingsParams {
    /// Filter by cluster ID.
    #[serde(default)]
    cluster_id: Option<String>,
    /// Filter by instance ID.
    #[serde(default)]
    instance_id: Option<String>,
    /// Filter by resolved status (true/false).
    #[serde(default)]
    resolved: Option<bool>,
    /// Filter by category (e.g. "hardware", "network", "disk_space").
    #[serde(default)]
    category: Option<String>,
    /// Max results (default 200, max 1000).
    #[serde(default)]
    limit: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetStaffPingParams {
    /// Staff ping ID.
    ping_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ResolveStaffPingParams {
    /// Staff ping ID to resolve.
    ping_id: String,
    /// Who resolved it (defaults to "admin-mcp").
    #[serde(default)]
    resolved_by: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UnresolveStaffPingParams {
    /// Staff ping ID to unresolve.
    ping_id: String,
}

// ── Meta-tool implementations ─────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct InstanceRow {
    instance_id: String,
    cluster_id: Uuid,
    hostname: Option<String>,
    relay_proxy_url: Option<String>,
    reported_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow)]
struct StaffPingSqlRow {
    id: Uuid,
    session_id: Uuid,
    cluster_id: Uuid,
    instance_id: String,
    category: String,
    message: String,
    resolved: bool,
    resolved_by: Option<String>,
    resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
struct StaffPingOutput {
    id: String,
    session_id: String,
    cluster_id: String,
    instance_id: String,
    category: String,
    message: String,
    resolved: bool,
    resolved_by: Option<String>,
    resolved_at: Option<String>,
    created_at: String,
}

impl From<StaffPingSqlRow> for StaffPingOutput {
    fn from(r: StaffPingSqlRow) -> Self {
        Self {
            id: r.id.to_string(),
            session_id: r.session_id.to_string(),
            cluster_id: r.cluster_id.to_string(),
            instance_id: r.instance_id,
            category: r.category,
            message: r.message,
            resolved: r.resolved,
            resolved_by: r.resolved_by,
            resolved_at: r.resolved_at.map(|t| t.to_rfc3339()),
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

// Heartbeat row for create_session context
#[derive(sqlx::FromRow)]
struct HeartbeatContextRow {
    relay_proxy_url: Option<String>,
    services_extended: Option<serde_json::Value>,
    file_tunnels: Option<serde_json::Value>,
    shell_tunnels: Option<serde_json::Value>,
    sample: Option<serde_json::Value>,
    hostname: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ClusterNameRow {
    name: String,
}

#[derive(sqlx::FromRow)]
struct PeerInstanceRow {
    instance_id: String,
    hostname: Option<String>,
}

// ── Session context builder ──────────────────────────────────────────

impl HealerMcpServer {
    /// Build the full session context from a heartbeat, mint a proxy token,
    /// load all healer tools, and populate `self.session_ctx` / `self.tools` /
    /// `self.tool_descriptors`. Returns the number of tools loaded.
    ///
    /// Used by both `create_session` and warm-session restore on `initialize`.
    async fn build_session_context(
        &self,
        cluster_id: Uuid,
        instance_id: &str,
        session_id: Uuid,
    ) -> Result<usize, String> {
        // Look up heartbeat data
        let hb = sqlx::query_as::<_, HeartbeatContextRow>(
            "SELECT relay_proxy_url, services_extended, file_tunnels, shell_tunnels, sample, hostname \
             FROM daemon_heartbeats WHERE cluster_id = $1 AND instance_id = $2",
        )
        .bind(cluster_id)
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Error querying heartbeats: {e}"))?
        .ok_or_else(|| "Error: instance not found in heartbeats".to_string())?;

        let relay_url = hb
            .relay_proxy_url
            .as_ref()
            .ok_or_else(|| "Error: instance has no relay proxy URL".to_string())?
            .clone();

        let cluster_name =
            sqlx::query_as::<_, ClusterNameRow>("SELECT name FROM clusters WHERE id = $1")
                .bind(cluster_id)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten()
                .map(|r| r.name)
                .unwrap_or_else(|| cluster_id.to_string());

        // Mint proxy token
        let healer_scopes: &[&str] = &["files:read", "files:write", "shell:exec", "logs:read"];
        let (proxy_token, _) = self
            .store
            .mint_proxy_token_scoped(cluster_id, None, Some(healer_scopes))
            .await
            .map_err(|e| format!("Error minting proxy token: {e}"))?;

        // Build relay access
        let relay_client = Arc::new(RelayClient::new(relay_url.clone(), proxy_token));
        let instance_prefix: String = instance_id.chars().take(12).collect();
        let instance_access: mac_mgmt_healer::instance_access::DynInstanceAccess = Arc::new(
            RelayInstanceAccess::new(relay_client.clone(), instance_prefix.clone()),
        );
        let cluster_access: Option<DynClusterAccess> =
            Some(Arc::new(RelayClusterAccess::new(relay_client)));

        // Parse services, tunnels, sample from heartbeat
        let services_extended: Vec<mac_mgmt_common::ServiceExtState> = hb
            .services_extended
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let file_tunnels_full = hb
            .file_tunnels
            .clone()
            .unwrap_or(serde_json::Value::Array(vec![]));
        let file_tunnel_names: Vec<String> = match &file_tunnels_full {
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect(),
            _ => vec![],
        };

        let shell_tunnels_full = hb
            .shell_tunnels
            .clone()
            .unwrap_or(serde_json::Value::Array(vec![]));
        let shell_command_names: Vec<String> = match &shell_tunnels_full {
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect(),
            _ => vec![],
        };

        let sample_summary = hb
            .sample
            .as_ref()
            .map(|s| mac_mgmt_healer::agent::format_sample_summary(s))
            .unwrap_or_default();

        // Get peer instances
        let peer_instances: Vec<mac_mgmt_healer::agent::InstanceInfo> =
            sqlx::query_as::<_, PeerInstanceRow>(
                "SELECT instance_id, hostname FROM daemon_heartbeats \
                 WHERE cluster_id = $1 AND instance_id != $2 \
                 AND reported_at > now() - interval '5 minutes'",
            )
            .bind(cluster_id)
            .bind(instance_id)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| mac_mgmt_healer::agent::InstanceInfo {
                instance_prefix: r.instance_id.chars().take(12).collect(),
                hostname: r.hostname.unwrap_or_else(|| "unknown".to_string()),
                healthy: true,
            })
            .collect();

        let cluster_instance_prefixes: Vec<String> = peer_instances
            .iter()
            .map(|i| i.instance_prefix.clone())
            .collect();

        // Build ToolContext
        let (events_tx, _) = tokio::sync::broadcast::channel(64);
        let tool_ctx = ToolContext {
            instance: instance_access,
            instance_data: self.instance_data.clone(),
            cluster: cluster_access,
            target_instance: instance_prefix,
            cluster_instances: cluster_instance_prefixes,
            file_tunnels: file_tunnel_names.clone(),
            file_tunnels_full,
            shell_commands: shell_command_names.clone(),
            shell_commands_full: shell_tunnels_full,
            store: self.store.clone(),
            session_id,
            cluster_id,
            instance_id: instance_id.to_string(),
            push_fn: self.push_fn.clone(),
            events_tx,
            metrics_url: Some(format!("{}/metrics", relay_url)),
            auto_approve: true,
            approval_notify: Arc::new(tokio::sync::Notify::new()),
        };

        // Build all tools
        let healer_tools = mac_mgmt_healer::tools::all_tools_with_risk(tool_ctx.clone(), false);
        let settings_tools =
            mac_mgmt_healer::settings_tools::all_settings_tools_with_risk(tool_ctx, false);

        let mut all_tools = healer_tools;
        all_tools.extend(settings_tools);

        let descriptors: Vec<rmcp::model::Tool> = all_tools
            .iter()
            .map(|(tool, risk)| swiftide_spec_to_rmcp_tool(&tool.tool_spec(), *risk))
            .collect();

        let tool_count = all_tools.len();

        // Store session context
        *self.session_ctx.write().await = Some(SessionContext {
            session_id,
            cluster_id,
            instance_id: instance_id.to_string(),
            cluster_name,
            hostname: hb.hostname.unwrap_or_else(|| "unknown".to_string()),
            services_extended,
            sample_summary,
            file_tunnels: file_tunnel_names,
            shell_commands: shell_command_names,
            other_instances: peer_instances,
            metrics_url: Some(format!("{}/metrics", relay_url)),
        });
        *self.tools.write().await = all_tools;
        *self.tool_descriptors.write().await = descriptors;

        Ok(tool_count)
    }
}

#[tool_router]
impl HealerMcpServer {
    #[tool(
        name = "list_instances",
        description = "List fleet instances with their status. Returns instance_id, cluster_id, hostname, relay availability, and last heartbeat time. Use cluster_id to filter."
    )]
    async fn list_instances(&self, Parameters(params): Parameters<ListInstancesParams>) -> String {
        let rows = if let Some(cid) = &params.cluster_id {
            let cid: Uuid = match cid.parse() {
                Ok(id) => id,
                Err(_) => return "Error: invalid cluster_id UUID".to_string(),
            };
            sqlx::query_as::<_, InstanceRow>(
                "SELECT instance_id, cluster_id, hostname, relay_proxy_url, reported_at \
                 FROM daemon_heartbeats WHERE cluster_id = $1 \
                 ORDER BY reported_at DESC LIMIT 200",
            )
            .bind(cid)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, InstanceRow>(
                "SELECT instance_id, cluster_id, hostname, relay_proxy_url, reported_at \
                 FROM daemon_heartbeats \
                 ORDER BY reported_at DESC LIMIT 200",
            )
            .fetch_all(&self.pool)
            .await
        };

        match rows {
            Ok(rows) => {
                let mut out = format!("{} instances:\n\n", rows.len());
                for r in &rows {
                    let online = if r.relay_proxy_url.is_some() {
                        "relay: yes"
                    } else {
                        "relay: no"
                    };
                    let age = chrono::Utc::now() - r.reported_at;
                    let age_str = if age.num_minutes() < 5 {
                        "online".to_string()
                    } else {
                        format!("{}m ago", age.num_minutes())
                    };
                    out.push_str(&format!(
                        "- {} ({}) cluster={} {} last_seen={}\n",
                        r.instance_id,
                        r.hostname.as_deref().unwrap_or("?"),
                        r.cluster_id,
                        online,
                        age_str,
                    ));
                }
                out
            }
            Err(e) => format!("Error listing instances: {e}"),
        }
    }

    #[tool(
        name = "create_session",
        description = "Create a healer session targeting a specific instance. This populates the tool list with ~40 healer tools for diagnosing and remediating the instance. Call list_instances first to find available targets."
    )]
    async fn create_session(&self, Parameters(params): Parameters<CreateSessionParams>) -> String {
        // If a warm session is already active for the same instance, return it.
        // For a different instance, end the old session first.
        {
            let guard = self.session_ctx.read().await;
            if let Some(ctx) = guard.as_ref() {
                if ctx.instance_id == params.instance_id {
                    let tool_count = self.tools.read().await.len();
                    return format!(
                        "Session created.\n- session_id: {}\n- instance: {}\n- cluster: {}\n- tools available: {tool_count}\n\n\
                         Call get_system_prompt to load the healer instructions.",
                        ctx.session_id, ctx.instance_id, ctx.cluster_id,
                    );
                }
            }
        }
        // End any existing session (different instance or stale).
        if let Some(old_ctx) = self.session_ctx.write().await.take() {
            let _ = self
                .store
                .transition_state(
                    old_ctx.session_id,
                    &mac_mgmt_healer::session::models::SessionState::Done,
                    &serde_json::json!({}),
                )
                .await;
            self.tools.write().await.clear();
            self.tool_descriptors.write().await.clear();
        }

        let cluster_id: Uuid = match params.cluster_id.parse() {
            Ok(id) => id,
            Err(_) => return "Error: invalid cluster_id UUID".to_string(),
        };

        // Create a healer session in the DB
        let session_id = match self
            .store
            .create_session(
                cluster_id,
                &params.instance_id,
                "admin-mcp",
                &serde_json::json!([]),
                &serde_json::json!({}),
                None,
                None,
                Some("mcp-session"),
            )
            .await
        {
            Ok(id) => id,
            Err(e) => return format!("Error creating session: {e}"),
        };

        // Build the full session context (heartbeat, proxy token, tools).
        match self
            .build_session_context(cluster_id, &params.instance_id, session_id)
            .await
        {
            Ok(tool_count) => {
                // Notify the client that the tool list has changed.
                if let Some(peer) = self.peer.read().await.as_ref() {
                    let _ = peer.notify_tool_list_changed().await;
                }
                format!(
                    "Session created.\n- session_id: {session_id}\n- instance: {}\n- cluster: {}\n- tools available: {tool_count}\n\n\
                     Call get_system_prompt to load the healer instructions.",
                    params.instance_id, params.cluster_id,
                )
            }
            Err(e) => {
                // Clean up the DB session on failure.
                let _ = self
                    .store
                    .fail_session(session_id, &e, &serde_json::json!({}))
                    .await;
                e
            }
        }
    }

    #[tool(
        name = "end_session",
        description = "End the active healer session and clear healer tools."
    )]
    async fn end_session(&self) -> String {
        let ctx = self.session_ctx.write().await.take();
        self.tools.write().await.clear();
        self.tool_descriptors.write().await.clear();

        // Notify the client that the tool list has changed.
        if let Some(peer) = self.peer.read().await.as_ref() {
            let _ = peer.notify_tool_list_changed().await;
        }

        match ctx {
            Some(ctx) => format!(
                "Session {} ended for instance {}.",
                ctx.session_id, ctx.instance_id
            ),
            None => "No active session.".to_string(),
        }
    }

    #[tool(
        name = "get_session_info",
        description = "Get information about the active healer session."
    )]
    async fn get_session_info(&self) -> String {
        let ctx = self.session_ctx.read().await;
        let tool_count = self.tools.read().await.len();
        match &*ctx {
            Some(ctx) => format!(
                "Active session:\n- session_id: {}\n- instance: {}\n- cluster: {} ({})\n- hostname: {}\n- tools: {tool_count}",
                ctx.session_id, ctx.instance_id, ctx.cluster_id, ctx.cluster_name, ctx.hostname,
            ),
            None => "No active session. Call create_session first.".to_string(),
        }
    }

    #[tool(
        name = "get_system_prompt",
        description = "Get the healer agent system prompt for the active session. Load this after create_session to understand how to diagnose and remediate the target instance."
    )]
    async fn get_system_prompt(&self) -> String {
        let ctx = self.session_ctx.read().await;
        match &*ctx {
            Some(ctx) => mac_mgmt_healer::agent::build_system_prompt_external(
                &ctx.cluster_name,
                &ctx.cluster_id.to_string(),
                &ctx.instance_id,
                &ctx.hostname,
                &ctx.other_instances,
                &ctx.services_extended,
                &ctx.sample_summary,
                &ctx.file_tunnels,
                &ctx.shell_commands,
                None, // resume_context
                "",   // metrics_summary (fetched on demand via get_metrics tool)
            ),
            None => "No active session. Call create_session first.".to_string(),
        }
    }

    // ── Staff ping tools ──────────────────────────────────────────────

    #[tool(
        name = "list_staff_pings",
        description = "List staff pings across all clusters and instances. Filter by cluster_id, instance_id, resolved status, or category. Defaults to showing unresolved pings."
    )]
    async fn list_staff_pings(
        &self,
        Parameters(params): Parameters<ListStaffPingsParams>,
    ) -> String {
        let limit = params.limit.unwrap_or(200).min(1000);

        // Build dynamic query
        let mut conditions: Vec<String> = Vec::new();
        let mut param_idx = 0u32;

        if params.cluster_id.is_some() {
            param_idx += 1;
            conditions.push(format!("cluster_id = ${param_idx}"));
        }
        if params.instance_id.is_some() {
            param_idx += 1;
            conditions.push(format!("instance_id = ${param_idx}"));
        }
        if params.resolved.is_some() {
            param_idx += 1;
            conditions.push(format!("resolved = ${param_idx}"));
        }
        if params.category.is_some() {
            param_idx += 1;
            conditions.push(format!("category = ${param_idx}"));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        param_idx += 1;
        let sql = format!(
            "SELECT id, session_id, cluster_id, instance_id, category, message, \
                    resolved, resolved_by, resolved_at, created_at \
             FROM healer_staff_pings {where_clause} \
             ORDER BY resolved ASC, created_at DESC \
             LIMIT ${param_idx}"
        );

        let mut query = sqlx::query_as::<_, StaffPingSqlRow>(&sql);

        if let Some(cid) = &params.cluster_id {
            match cid.parse::<Uuid>() {
                Ok(id) => query = query.bind(id),
                Err(_) => return "Error: invalid cluster_id UUID".to_string(),
            }
        }
        if let Some(iid) = &params.instance_id {
            query = query.bind(iid.clone());
        }
        if let Some(r) = params.resolved {
            query = query.bind(r);
        }
        if let Some(cat) = &params.category {
            query = query.bind(cat.clone());
        }
        query = query.bind(limit);

        match query.fetch_all(&self.pool).await {
            Ok(rows) => {
                let pings: Vec<StaffPingOutput> = rows.into_iter().map(Into::into).collect();
                serde_json::to_string_pretty(&pings)
                    .unwrap_or_else(|e| format!("Error serializing: {e}"))
            }
            Err(e) => format!("Error listing staff pings: {e}"),
        }
    }

    #[tool(
        name = "get_staff_ping",
        description = "Get a single staff ping by ID with full details."
    )]
    async fn get_staff_ping(&self, Parameters(params): Parameters<GetStaffPingParams>) -> String {
        let pid: Uuid = match params.ping_id.parse() {
            Ok(id) => id,
            Err(_) => return "Error: invalid ping_id UUID".to_string(),
        };

        match sqlx::query_as::<_, StaffPingSqlRow>(
            "SELECT id, session_id, cluster_id, instance_id, category, message, \
                    resolved, resolved_by, resolved_at, created_at \
             FROM healer_staff_pings WHERE id = $1",
        )
        .bind(pid)
        .fetch_optional(&self.pool)
        .await
        {
            Ok(Some(row)) => {
                let ping: StaffPingOutput = row.into();
                serde_json::to_string_pretty(&ping)
                    .unwrap_or_else(|e| format!("Error serializing: {e}"))
            }
            Ok(None) => "Staff ping not found.".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        name = "resolve_staff_ping",
        description = "Mark a staff ping as resolved. Idempotent — resolving an already-resolved ping is a no-op."
    )]
    async fn resolve_staff_ping(
        &self,
        Parameters(params): Parameters<ResolveStaffPingParams>,
    ) -> String {
        let pid: Uuid = match params.ping_id.parse() {
            Ok(id) => id,
            Err(_) => return "Error: invalid ping_id UUID".to_string(),
        };
        let resolved_by = params.resolved_by.as_deref().unwrap_or("admin-mcp");

        match sqlx::query(
            "UPDATE healer_staff_pings \
             SET resolved = true, resolved_by = $1, resolved_at = now() \
             WHERE id = $2 AND NOT resolved",
        )
        .bind(resolved_by)
        .bind(pid)
        .execute(&self.pool)
        .await
        {
            Ok(result) => {
                if result.rows_affected() > 0 {
                    format!("Staff ping {pid} resolved.")
                } else {
                    format!("Staff ping {pid} was already resolved or not found.")
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        name = "unresolve_staff_ping",
        description = "Reopen a resolved staff ping."
    )]
    async fn unresolve_staff_ping(
        &self,
        Parameters(params): Parameters<UnresolveStaffPingParams>,
    ) -> String {
        let pid: Uuid = match params.ping_id.parse() {
            Ok(id) => id,
            Err(_) => return "Error: invalid ping_id UUID".to_string(),
        };

        match sqlx::query(
            "UPDATE healer_staff_pings \
             SET resolved = false, resolved_by = NULL, resolved_at = NULL \
             WHERE id = $1 AND resolved",
        )
        .bind(pid)
        .execute(&self.pool)
        .await
        {
            Ok(result) => {
                if result.rows_affected() > 0 {
                    format!("Staff ping {pid} reopened.")
                } else {
                    format!("Staff ping {pid} was not resolved or not found.")
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }
}

// ── ServerHandler impl ────────────────────────────────────────────────

impl ServerHandler for HealerMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "MCP server for mac-mgmt fleet healing. Use list_instances and create_session \
                 to target an instance, then get_system_prompt for behavioral instructions. \
                 Staff ping tools are always available for triage."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
            ..Default::default()
        }
    }

    fn initialize(
        &self,
        _request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<InitializeResult, McpError>> + Send + '_ {
        async move {
            // Extract HTTP parts for auth
            let parts = context.extensions.get::<http::request::Parts>();
            let token = parts.and_then(|p| {
                p.headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.strip_prefix("Bearer "))
            });

            let token = match token {
                Some(t) => t,
                None => {
                    return Err(McpError::new(
                        ErrorCode::INVALID_REQUEST,
                        "Missing Authorization: Bearer <token> header",
                        None,
                    ));
                }
            };

            if !validate_admin_token(&self.pool, token).await {
                return Err(McpError::new(
                    ErrorCode::INVALID_REQUEST,
                    "Invalid or expired admin token",
                    None,
                ));
            }

            // Store peer handle so we can send tools/list_changed later.
            *self.peer.write().await = Some(context.peer.clone());

            // Restore warm MCP session if one exists in the database.
            // This makes healer tools available from the first tools/list
            // call, avoiding the need for a tools/list_changed notification.
            if let Ok(Some(sess)) = self.store.find_mcp_session().await {
                tracing::info!(
                    session_id = %sess.id,
                    instance = %sess.instance_id,
                    "restoring warm MCP session"
                );
                match self
                    .build_session_context(sess.cluster_id, &sess.instance_id, sess.id)
                    .await
                {
                    Ok(n) => tracing::info!(tools = n, "warm session restored"),
                    Err(e) => {
                        tracing::warn!("warm session restore failed (instance offline?): {e}")
                    }
                }
            }

            let info = self.get_info();
            Ok(InitializeResult {
                protocol_version: ProtocolVersion::LATEST,
                capabilities: info.capabilities,
                server_info: info.server_info,
                instructions: info.instructions,
            })
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async move {
            // Meta-tools are always available
            let mut tools = self.meta_router.list_all();

            // Add healer tools if session is active
            let healer_tools = self.tool_descriptors.read().await;
            tools.extend(healer_tools.iter().cloned());

            Ok(ListToolsResult {
                tools,
                next_cursor: None,
                meta: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            let tool_name = request.name.as_ref();

            // Check if it's a meta-tool
            if self.meta_router.has_route(tool_name) {
                let ctx = ToolCallContext::new(self, request, context);
                return self
                    .meta_router
                    .call(ctx)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None));
            }

            // Otherwise, try healer tools
            let tools = self.tools.read().await;
            let tool = tools.iter().find(|(t, _)| t.name() == tool_name);

            match tool {
                Some((tool, _risk)) => {
                    invoke_swiftide_tool(tool.as_ref(), request.arguments.as_ref()).await
                }
                None => Err(McpError::new(
                    ErrorCode::INVALID_PARAMS,
                    format!("Unknown tool: {tool_name}"),
                    None,
                )),
            }
        }
    }

    fn get_tool(&self, name: &str) -> Option<rmcp::model::Tool> {
        // Check meta-tools first
        if let Some(t) = self.meta_router.get(name) {
            return Some(t.clone());
        }

        // Check healer tools (blocking read — this is a sync method)
        // Use try_read to avoid blocking; if locked, return None
        if let Ok(descriptors) = self.tool_descriptors.try_read() {
            return descriptors.iter().find(|t| t.name == name).cloned();
        }

        None
    }
}

// ── Service factory ───────────────────────────────────────────────────

pub fn build_mcp_service(
    pool: PgPool,
    store: Arc<PgHealerStore>,
    instance_data: DynInstanceData,
    push_fn: Option<PushFn>,
) -> rmcp::transport::streamable_http_server::StreamableHttpService<HealerMcpServer> {
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    };

    StreamableHttpService::new(
        move || {
            Ok(HealerMcpServer::new(
                pool.clone(),
                store.clone(),
                instance_data.clone(),
                push_fn.clone(),
            ))
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig {
            stateful_mode: true,
            ..StreamableHttpServerConfig::default()
        },
    )
}
