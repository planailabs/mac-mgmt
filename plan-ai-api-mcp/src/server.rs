//! MCP `ServerHandler` over Streamable HTTP, generated from a [`Registry`].
//!
//! Modeled on `server/src/mcp_healer.rs`: auth happens in `initialize` (Bearer
//! token read from the HTTP request parts), the resolved principal is stored on
//! the per-session server instance, and `call_tool` dispatches to the erased
//! endpoint by tool name.

use std::borrow::Cow;
use std::sync::Arc;

use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::{RoleServer, ServerHandler};
use serde_json::{Value, json};
use tokio::sync::RwLock;

use crate::error::ApiError;
use crate::principal::Principal;
use crate::registry::{ErasedEndpoint, Registry, Risk};

type McpError = rmcp::model::ErrorData;

/// One MCP session's view of the API. Created per session by the service
/// factory; holds the resolved principal after `initialize`.
pub struct ApiMcpServer<S> {
    endpoints: Arc<Vec<Arc<ErasedEndpoint<S>>>>,
    tools: Arc<Vec<Tool>>,
    auth: Arc<dyn crate::auth::Authenticator>,
    state: S,
    instructions: Arc<Option<String>>,
    principal: RwLock<Option<Arc<Principal>>>,
}

impl<S: Clone + Send + Sync + 'static> Registry<S> {
    /// Build the MCP Streamable-HTTP service. Mount it with
    /// `.route_service("/mcp", svc.clone()).route_service("/mcp/", svc)`.
    pub fn mcp_service(&self, state: S) -> StreamableHttpService<ApiMcpServer<S>> {
        let endpoints = Arc::new(self.endpoints.clone());
        let tools: Arc<Vec<Tool>> =
            Arc::new(self.endpoints.iter().map(|e| endpoint_tool(e)).collect());
        let auth = self.auth.clone();
        let instructions = Arc::new(self.instructions.clone());

        StreamableHttpService::new(
            move || {
                Ok(ApiMcpServer {
                    endpoints: endpoints.clone(),
                    tools: tools.clone(),
                    auth: auth.clone(),
                    state: state.clone(),
                    instructions: instructions.clone(),
                    principal: RwLock::new(None),
                })
            },
            Arc::new(LocalSessionManager::default()),
            rmcp::transport::streamable_http_server::StreamableHttpServerConfig {
                stateful_mode: true,
                ..Default::default()
            },
        )
    }
}

fn endpoint_tool<S>(ep: &ErasedEndpoint<S>) -> Tool {
    let annotations = match ep.risk() {
        Risk::ReadOnly => ToolAnnotations::new().read_only(true).destructive(false),
        Risk::Mutating => ToolAnnotations::new().read_only(false).destructive(false),
        Risk::Destructive => ToolAnnotations::new().read_only(false).destructive(true),
    };
    Tool {
        name: Cow::Owned(ep.tool_name()),
        title: None,
        description: Some(Cow::Owned(ep.description.clone())),
        input_schema: Arc::new(ep.input_schema.clone()),
        output_schema: Some(Arc::new(ep.output_schema.clone())),
        annotations: Some(annotations),
        execution: None,
        icons: None,
        meta: None,
    }
}

impl<S: Clone + Send + Sync + 'static> ApiMcpServer<S> {
    fn server_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: self.instructions.as_ref().clone(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

impl<S: Clone + Send + Sync + 'static> ServerHandler for ApiMcpServer<S> {
    fn get_info(&self) -> ServerInfo {
        self.server_info()
    }

    fn initialize(
        &self,
        _request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<InitializeResult, McpError>> + Send + '_ {
        async move {
            let parts = context.extensions.get::<http::request::Parts>();
            let token = parts.and_then(|p| {
                p.headers
                    .get(http::header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.strip_prefix("Bearer "))
                    .filter(|t| !t.is_empty())
            });

            let token = token.ok_or_else(|| {
                ApiError::unauthorized("missing Authorization: Bearer <token> header").into_mcp()
            })?;

            let principal = self
                .auth
                .authenticate(token)
                .await
                .map_err(|e| e.into_mcp())?;
            *self.principal.write().await = Some(Arc::new(principal));

            let info = self.server_info();
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
            Ok(ListToolsResult {
                tools: self.tools.as_ref().clone(),
                next_cursor: None,
                meta: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            let principal = match self.principal.read().await.clone() {
                Some(p) => p,
                None => {
                    return Err(ApiError::unauthorized("not authenticated").into_mcp());
                }
            };

            let name = request.name.as_ref();
            let ep = self.endpoints.iter().find(|e| e.tool_name() == name);
            let ep = match ep {
                Some(ep) => ep.clone(),
                None => {
                    return Err(McpError::new(
                        ErrorCode::INVALID_PARAMS,
                        format!("unknown tool: {name}"),
                        None,
                    ));
                }
            };

            let args = request
                .arguments
                .map(Value::Object)
                .unwrap_or_else(|| json!({}));

            match (ep.call)(self.state.clone(), principal, args).await {
                Ok(value) => {
                    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
                    Ok(CallToolResult::success(vec![Content::text(text)]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
            }
        }
    }
}
