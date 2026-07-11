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

// MCP `structuredContent` must be a JSON *object*, but endpoints may return
// arrays, strings, uuids or unit. Those get a `{"result": …}` envelope at the
// MCP layer only — declared schema and returned value alike, so they always
// match. REST responses and the OpenAPI document keep the raw shape.

fn schema_is_object(schema: &JsonObject) -> bool {
    schema.get("type").and_then(Value::as_str) == Some("object")
}

fn envelope_schema(raw: &JsonObject) -> JsonObject {
    let mut inner = raw.clone();
    // `$defs` refs ("#/$defs/…") are rooted at the schema document, so they
    // must stay top-level when the payload schema moves under `properties`.
    let defs = inner.remove("$defs");
    let mut out = JsonObject::new();
    out.insert("type".into(), json!("object"));
    out.insert(
        "properties".into(),
        json!({ "result": Value::Object(inner) }),
    );
    out.insert("required".into(), json!(["result"]));
    if let Some(defs) = defs {
        out.insert("$defs".into(), defs);
    }
    out
}

fn mcp_output_schema(raw: &JsonObject) -> JsonObject {
    if schema_is_object(raw) {
        raw.clone()
    } else {
        envelope_schema(raw)
    }
}

fn mcp_structured_output(raw_schema: &JsonObject, value: Value) -> Value {
    if schema_is_object(raw_schema) {
        value
    } else {
        json!({ "result": value })
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
        output_schema: Some(Arc::new(mcp_output_schema(&ep.output_schema))),
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
                    let structured = mcp_structured_output(&ep.output_schema, value);
                    let text = serde_json::to_string_pretty(&structured)
                        .unwrap_or_else(|_| structured.to_string());
                    Ok(CallToolResult {
                        content: vec![Content::text(text)],
                        structured_content: Some(structured),
                        is_error: Some(false),
                        meta: None,
                    })
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(v: Value) -> JsonObject {
        match v {
            Value::Object(m) => m,
            _ => panic!("not an object"),
        }
    }

    #[test]
    fn object_output_passes_through() {
        let schema = obj(json!({ "type": "object", "properties": { "a": { "type": "string" } } }));
        assert_eq!(mcp_output_schema(&schema), schema);
        assert_eq!(
            mcp_structured_output(&schema, json!({ "a": "x" })),
            json!({ "a": "x" })
        );
    }

    #[test]
    fn array_output_is_wrapped_with_defs_kept_top_level() {
        let schema = obj(json!({
            "type": "array",
            "items": { "$ref": "#/$defs/Row" },
            "$defs": { "Row": { "type": "object" } }
        }));
        let wrapped = mcp_output_schema(&schema);
        assert_eq!(wrapped.get("type"), Some(&json!("object")));
        assert_eq!(wrapped.get("required"), Some(&json!(["result"])));
        assert_eq!(
            wrapped["properties"]["result"],
            json!({ "type": "array", "items": { "$ref": "#/$defs/Row" } })
        );
        assert_eq!(wrapped["$defs"], json!({ "Row": { "type": "object" } }));
        assert_eq!(
            mcp_structured_output(&schema, json!([1, 2])),
            json!({ "result": [1, 2] })
        );
    }

    #[test]
    fn unit_and_string_outputs_are_wrapped() {
        let null_schema = obj(json!({ "type": "null" }));
        assert_eq!(
            mcp_structured_output(&null_schema, Value::Null),
            json!({ "result": null })
        );
        let str_schema = obj(json!({ "type": "string" }));
        assert_eq!(
            mcp_output_schema(&str_schema)["properties"]["result"],
            json!({ "type": "string" })
        );
    }
}
