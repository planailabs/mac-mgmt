//! Endpoint registry: declare CRUD once, serve it as REST + MCP + OpenAPI.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, RawQuery};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodFilter, MethodRouter};
use axum::{Json, Router};
use futures::future::BoxFuture;
use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::auth::{Authenticator, bearer_from_header};
use crate::error::ApiError;
use crate::principal::Principal;

/// MCP tool risk → HTTP verb hint and tool annotations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    ReadOnly,
    Mutating,
    Destructive,
}

/// Whether a custom action operates on a single item (`/{id}/verb`) or the
/// collection (`/verb`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnItem {
    Yes,
    No,
}

/// The CRUD action an endpoint implements. Drives the REST verb+path, the MCP
/// tool-name suffix, and the default risk.
#[derive(Clone, Debug)]
pub enum Action {
    List,
    Get,
    Create,
    Update,
    Delete,
    Custom {
        verb: &'static str,
        risk: Risk,
        on_item: bool,
    },
}

impl Action {
    fn suffix(&self) -> &str {
        match self {
            Action::List => "list",
            Action::Get => "get",
            Action::Create => "create",
            Action::Update => "update",
            Action::Delete => "delete",
            Action::Custom { verb, .. } => verb,
        }
    }

    fn method(&self) -> http::Method {
        match self {
            Action::List | Action::Get => http::Method::GET,
            Action::Create => http::Method::POST,
            Action::Update => http::Method::PATCH,
            Action::Delete => http::Method::DELETE,
            Action::Custom { .. } => http::Method::POST,
        }
    }

    fn on_item(&self) -> bool {
        match self {
            Action::Get | Action::Update | Action::Delete => true,
            Action::List | Action::Create => false,
            Action::Custom { on_item, .. } => *on_item,
        }
    }

    fn has_body(&self) -> bool {
        matches!(
            self.method(),
            http::Method::POST | http::Method::PATCH | http::Method::PUT
        )
    }

    fn risk(&self) -> Risk {
        match self {
            Action::List | Action::Get => Risk::ReadOnly,
            Action::Create | Action::Update => Risk::Mutating,
            Action::Delete => Risk::Destructive,
            Action::Custom { risk, .. } => *risk,
        }
    }
}

type CallFn<S> =
    Box<dyn Fn(S, Arc<Principal>, Value) -> BoxFuture<'static, Result<Value, ApiError>> + Send + Sync>;

/// A single registered endpoint, erased to JSON in/out.
pub struct ErasedEndpoint<S> {
    pub resource: &'static str,
    pub tool: &'static str,
    pub tag: &'static str,
    pub action: Action,
    pub description: String,
    pub input_schema: JsonObject,
    pub output_schema: JsonObject,
    pub(crate) call: CallFn<S>,
}

impl<S> ErasedEndpoint<S> {
    /// MCP tool name, e.g. `domain_list`, `domain_deploy_cloudflare`.
    pub fn tool_name(&self) -> String {
        format!("{}_{}", self.tool, self.action.suffix())
    }

    pub fn risk(&self) -> Risk {
        self.action.risk()
    }

    pub fn http_method(&self) -> http::Method {
        self.action.method()
    }

    /// REST path, e.g. `/api/v1/domains`, `/api/v1/domains/{id}`,
    /// `/api/v1/domains/{id}/deploy_cloudflare`.
    pub fn path(&self) -> String {
        let mut p = format!("/api/v1/{}", self.resource);
        if self.action.on_item() {
            p.push_str("/{id}");
        }
        if let Action::Custom { verb, .. } = self.action {
            p.push('/');
            p.push_str(verb);
        }
        p
    }
}

/// A set of endpoints plus the authenticator that guards them. Build a
/// [`Registry`] once, then derive an HTTP router, an MCP service, and an
/// OpenAPI document from it.
pub struct Registry<S> {
    pub(crate) endpoints: Vec<Arc<ErasedEndpoint<S>>>,
    pub(crate) auth: Arc<dyn Authenticator>,
    pub(crate) instructions: Option<String>,
    title: String,
    version: String,
}

impl<S: Clone + Send + Sync + 'static> Registry<S> {
    pub fn new(auth: Arc<dyn Authenticator>) -> Self {
        Self {
            endpoints: Vec::new(),
            auth,
            instructions: None,
            title: "API".into(),
            version: "0.1.0".into(),
        }
    }

    /// Set the OpenAPI `info.title` / `info.version`.
    pub fn info(mut self, title: impl Into<String>, version: impl Into<String>) -> Self {
        self.title = title.into();
        self.version = version.into();
        self
    }

    /// Set the MCP server `instructions` string shown to clients.
    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    /// Begin declaring endpoints for one resource. `resource` is the plural REST
    /// path segment (`"domains"`), `tool` the singular MCP tool prefix
    /// (`"domain"`), `tag` the OpenAPI group (`"Domains"`).
    pub fn resource(
        &mut self,
        resource: &'static str,
        tool: &'static str,
        tag: &'static str,
    ) -> ResourceBuilder<'_, S> {
        ResourceBuilder {
            reg: self,
            resource,
            tool,
            tag,
        }
    }

    fn push<I, O, F, Fut>(
        &mut self,
        resource: &'static str,
        tool: &'static str,
        tag: &'static str,
        action: Action,
        description: String,
        handler: F,
    ) where
        I: DeserializeOwned + JsonSchema + Send + 'static,
        O: Serialize + JsonSchema + 'static,
        F: Fn(S, Arc<Principal>, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, ApiError>> + Send + 'static,
    {
        self.endpoints.push(Arc::new(ErasedEndpoint {
            resource,
            tool,
            tag,
            action,
            description,
            input_schema: schema_object::<I>(),
            output_schema: schema_object::<O>(),
            call: erase(handler),
        }));
    }

    // ── HTTP ──────────────────────────────────────────────────────────

    /// Build the axum router: REST routes for every endpoint plus
    /// `/api/v1` (index), `/api/v1/openapi.json`, and `/api/v1/docs`.
    pub fn http_router(&self, state: S) -> Router {
        let mut by_path: BTreeMap<String, MethodRouter> = BTreeMap::new();

        for ep in &self.endpoints {
            let path = ep.path();
            let filter = method_filter(&ep.http_method());
            let mr = by_path.remove(&path).unwrap_or_else(MethodRouter::new);
            let mr = mount_endpoint(mr, filter, ep.clone(), self.auth.clone(), state.clone());
            by_path.insert(path, mr);
        }

        let mut router = Router::new();
        for (path, mr) in by_path {
            router = router.route(&path, mr);
        }

        let index = self.index_json();
        router = router.route(
            "/api/v1",
            axum::routing::get(move || {
                let index = index.clone();
                async move { Json(index) }
            }),
        );

        // OpenAPI + Swagger UI via utoipa: our schemars-generated document is the
        // source of truth; utoipa serves it (at /api/v1/openapi.json) and renders
        // the bundled Swagger UI (at /api/v1/docs). Parsing our generated JSON into
        // utoipa's typed OpenApi is deterministic from the registered types, so a
        // failure here is a build-time bug we want surfaced loudly.
        let openapi: utoipa::openapi::OpenApi = serde_json::from_value(self.openapi_json())
            .expect("generated OpenAPI must parse into utoipa::openapi::OpenApi");
        router.merge(
            utoipa_swagger_ui::SwaggerUi::new("/api/v1/docs")
                .url("/api/v1/openapi.json", openapi),
        )
    }

    /// The MCP tool names this registry exposes, in registration order.
    pub fn tool_names(&self) -> Vec<String> {
        self.endpoints.iter().map(|e| e.tool_name()).collect()
    }

    fn index_json(&self) -> Value {
        let items: Vec<Value> = self
            .endpoints
            .iter()
            .map(|ep| {
                json!({
                    "tool": ep.tool_name(),
                    "method": ep.http_method().as_str(),
                    "path": ep.path(),
                    "risk": risk_str(ep.risk()),
                    "description": ep.description,
                })
            })
            .collect();
        json!({ "endpoints": items })
    }

    // ── OpenAPI 3.1 ───────────────────────────────────────────────────

    /// Generate an OpenAPI 3.1 document from the registered endpoints.
    pub fn openapi_json(&self) -> Value {
        let mut components_schemas: Map<String, Value> = Map::new();
        let mut paths: Map<String, Value> = Map::new();

        for ep in &self.endpoints {
            let input = hoist(&ep.input_schema, &mut components_schemas);
            let output = hoist(&ep.output_schema, &mut components_schemas);

            let mut op = Map::new();
            op.insert("operationId".into(), json!(ep.tool_name()));
            op.insert("summary".into(), json!(ep.description));
            op.insert("tags".into(), json!([ep.tag]));

            let mut params: Vec<Value> = Vec::new();
            if ep.action.on_item() {
                params.push(json!({
                    "name": "id", "in": "path", "required": true,
                    "schema": { "type": "string", "format": "uuid" }
                }));
            }
            // ReadOnly GET list: expose input fields as query parameters.
            if matches!(ep.http_method(), http::Method::GET) && !ep.action.on_item() {
                if let Some(Value::Object(props)) = input.get("properties") {
                    let required: Vec<&str> = input
                        .get("required")
                        .and_then(|r| r.as_array())
                        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                        .unwrap_or_default();
                    for (name, schema) in props {
                        params.push(json!({
                            "name": name, "in": "query",
                            "required": required.contains(&name.as_str()),
                            "schema": schema,
                        }));
                    }
                }
            }
            if !params.is_empty() {
                op.insert("parameters".into(), json!(params));
            }

            if ep.action.has_body() {
                op.insert(
                    "requestBody".into(),
                    json!({
                        "required": true,
                        "content": { "application/json": { "schema": input } }
                    }),
                );
            }

            op.insert(
                "responses".into(),
                json!({
                    "200": {
                        "description": "OK",
                        "content": { "application/json": { "schema": output } }
                    },
                    "400": { "description": "Bad request" },
                    "401": { "description": "Unauthorized" },
                    "403": { "description": "Forbidden" },
                    "404": { "description": "Not found" }
                }),
            );

            let method = ep.http_method().as_str().to_lowercase();
            let path_item = paths
                .entry(ep.path())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(m) = path_item {
                m.insert(method, Value::Object(op));
            }
        }

        json!({
            "openapi": "3.1.0",
            "info": { "title": self.title, "version": self.version },
            "security": [{ "bearerAuth": [] }],
            "components": {
                "securitySchemes": {
                    "bearerAuth": { "type": "http", "scheme": "bearer" }
                },
                "schemas": components_schemas,
            },
            "paths": paths,
        })
    }
}

/// Fluent per-resource endpoint declaration.
pub struct ResourceBuilder<'a, S> {
    reg: &'a mut Registry<S>,
    resource: &'static str,
    tool: &'static str,
    tag: &'static str,
}

impl<'a, S: Clone + Send + Sync + 'static> ResourceBuilder<'a, S> {
    fn add<I, O, F, Fut>(&mut self, action: Action, description: &str, handler: F) -> &mut Self
    where
        I: DeserializeOwned + JsonSchema + Send + 'static,
        O: Serialize + JsonSchema + 'static,
        F: Fn(S, Arc<Principal>, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, ApiError>> + Send + 'static,
    {
        self.reg.push(
            self.resource,
            self.tool,
            self.tag,
            action,
            description.to_string(),
            handler,
        );
        self
    }

    pub fn list<I, O, F, Fut>(&mut self, description: &str, handler: F) -> &mut Self
    where
        I: DeserializeOwned + JsonSchema + Send + 'static,
        O: Serialize + JsonSchema + 'static,
        F: Fn(S, Arc<Principal>, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, ApiError>> + Send + 'static,
    {
        self.add(Action::List, description, handler)
    }

    pub fn get<I, O, F, Fut>(&mut self, description: &str, handler: F) -> &mut Self
    where
        I: DeserializeOwned + JsonSchema + Send + 'static,
        O: Serialize + JsonSchema + 'static,
        F: Fn(S, Arc<Principal>, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, ApiError>> + Send + 'static,
    {
        self.add(Action::Get, description, handler)
    }

    pub fn create<I, O, F, Fut>(&mut self, description: &str, handler: F) -> &mut Self
    where
        I: DeserializeOwned + JsonSchema + Send + 'static,
        O: Serialize + JsonSchema + 'static,
        F: Fn(S, Arc<Principal>, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, ApiError>> + Send + 'static,
    {
        self.add(Action::Create, description, handler)
    }

    pub fn update<I, O, F, Fut>(&mut self, description: &str, handler: F) -> &mut Self
    where
        I: DeserializeOwned + JsonSchema + Send + 'static,
        O: Serialize + JsonSchema + 'static,
        F: Fn(S, Arc<Principal>, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, ApiError>> + Send + 'static,
    {
        self.add(Action::Update, description, handler)
    }

    pub fn delete<I, O, F, Fut>(&mut self, description: &str, handler: F) -> &mut Self
    where
        I: DeserializeOwned + JsonSchema + Send + 'static,
        O: Serialize + JsonSchema + 'static,
        F: Fn(S, Arc<Principal>, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, ApiError>> + Send + 'static,
    {
        self.add(Action::Delete, description, handler)
    }

    pub fn custom<I, O, F, Fut>(
        &mut self,
        verb: &'static str,
        risk: Risk,
        on_item: OnItem,
        description: &str,
        handler: F,
    ) -> &mut Self
    where
        I: DeserializeOwned + JsonSchema + Send + 'static,
        O: Serialize + JsonSchema + 'static,
        F: Fn(S, Arc<Principal>, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, ApiError>> + Send + 'static,
    {
        self.add(
            Action::Custom {
                verb,
                risk,
                on_item: matches!(on_item, OnItem::Yes),
            },
            description,
            handler,
        )
    }
}

// ── Erasure & schema helpers ──────────────────────────────────────────

fn erase<S, I, O, F, Fut>(handler: F) -> CallFn<S>
where
    S: Send + 'static,
    I: DeserializeOwned + Send + 'static,
    O: Serialize + 'static,
    F: Fn(S, Arc<Principal>, I) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<O, ApiError>> + Send + 'static,
{
    let handler = Arc::new(handler);
    Box::new(move |state, principal, value| {
        let handler = handler.clone();
        Box::pin(async move {
            let input: I = serde_json::from_value(value)
                .map_err(|e| ApiError::bad_request(format!("invalid input: {e}")))?;
            let output = handler(state, principal, input).await?;
            serde_json::to_value(output)
                .map_err(|e| ApiError::internal(format!("serialize output: {e}")))
        })
    })
}

fn schema_object<T: JsonSchema>() -> JsonObject {
    let schema = schemars::schema_for!(T);
    let mut value = serde_json::to_value(schema).unwrap_or_else(|_| json!({ "type": "object" }));
    if let Value::Object(ref mut m) = value {
        m.remove("$schema");
    }
    match value {
        Value::Object(m) => m,
        _ => {
            let mut m = Map::new();
            m.insert("type".into(), json!("object"));
            m
        }
    }
}

// ── Request dispatch ──────────────────────────────────────────────────

async fn dispatch<S>(
    ep: Arc<ErasedEndpoint<S>>,
    auth: Arc<dyn Authenticator>,
    state: S,
    headers: HeaderMap,
    input: Value,
) -> Response
where
    S: Clone + Send + Sync + 'static,
{
    let token = match bearer_from_header(&headers) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };
    let principal = match auth.authenticate(token).await {
        Ok(p) => Arc::new(p),
        Err(e) => return e.into_response(),
    };
    match (ep.call)(state, principal, input).await {
        Ok(value) => Json(value).into_response(),
        Err(e) => e.into_response(),
    }
}

/// Attach the right handler shape (list / item / body / item+body) for `ep`.
fn mount_endpoint<S>(
    mr: MethodRouter,
    filter: MethodFilter,
    ep: Arc<ErasedEndpoint<S>>,
    auth: Arc<dyn Authenticator>,
    state: S,
) -> MethodRouter
where
    S: Clone + Send + Sync + 'static,
{
    let on_item = ep.action.on_item();
    let has_body = ep.action.has_body();

    match (on_item, has_body) {
        // Collection read (GET list): input from query string.
        (false, false) => mr.on(filter, {
            move |headers: HeaderMap, RawQuery(q): RawQuery| {
                let (ep, auth, state) = (ep.clone(), auth.clone(), state.clone());
                async move { dispatch(ep, auth, state, headers, query_to_json(q)).await }
            }
        }),
        // Collection write (POST create / custom): input from body.
        (false, true) => mr.on(filter, {
            move |headers: HeaderMap, body: Bytes| {
                let (ep, auth, state) = (ep.clone(), auth.clone(), state.clone());
                async move {
                    match json_body(&body) {
                        Ok(input) => dispatch(ep, auth, state, headers, input).await,
                        Err(e) => e.into_response(),
                    }
                }
            }
        }),
        // Item read/delete (GET/DELETE with {id}): input = { id }.
        (true, false) => mr.on(filter, {
            move |headers: HeaderMap, Path(id): Path<String>| {
                let (ep, auth, state) = (ep.clone(), auth.clone(), state.clone());
                async move {
                    dispatch(ep, auth, state, headers, json!({ "id": id })).await
                }
            }
        }),
        // Item write (PATCH/custom with {id}): input = body merged with { id }.
        (true, true) => mr.on(filter, {
            move |headers: HeaderMap, Path(id): Path<String>, body: Bytes| {
                let (ep, auth, state) = (ep.clone(), auth.clone(), state.clone());
                async move {
                    match json_body(&body) {
                        Ok(input) => {
                            dispatch(ep, auth, state, headers, with_id(input, id)).await
                        }
                        Err(e) => e.into_response(),
                    }
                }
            }
        }),
    }
}

fn json_body(bytes: &Bytes) -> Result<Value, ApiError> {
    if bytes.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice(bytes).map_err(|e| ApiError::bad_request(format!("invalid JSON body: {e}")))
}

fn with_id(value: Value, id: String) -> Value {
    match value {
        Value::Object(mut m) => {
            m.insert("id".into(), json!(id));
            Value::Object(m)
        }
        _ => json!({ "id": id }),
    }
}

/// Parse a query string into a JSON object, coercing scalar strings to
/// numbers/bools so typed inputs (e.g. `limit: i64`) deserialize.
fn query_to_json(raw: Option<String>) -> Value {
    let mut map = Map::new();
    if let Some(q) = raw {
        if let Ok(pairs) = serde_urlencoded::from_str::<Vec<(String, String)>>(&q) {
            for (k, v) in pairs {
                map.insert(k, coerce(&v));
            }
        }
    }
    Value::Object(map)
}

fn coerce(s: &str) -> Value {
    if let Ok(i) = s.parse::<i64>() {
        return json!(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return json!(f);
    }
    match s {
        "true" => json!(true),
        "false" => json!(false),
        other => json!(other),
    }
}

fn method_filter(m: &http::Method) -> MethodFilter {
    match *m {
        http::Method::GET => MethodFilter::GET,
        http::Method::POST => MethodFilter::POST,
        http::Method::PATCH => MethodFilter::PATCH,
        http::Method::PUT => MethodFilter::PUT,
        http::Method::DELETE => MethodFilter::DELETE,
        _ => MethodFilter::POST,
    }
}

pub(crate) fn risk_str(r: Risk) -> &'static str {
    match r {
        Risk::ReadOnly => "read_only",
        Risk::Mutating => "mutating",
        Risk::Destructive => "destructive",
    }
}

// ── OpenAPI $defs hoisting ────────────────────────────────────────────

/// Move a schema's `$defs` into a shared `components/schemas` map and rewrite
/// `#/$defs/*` refs to `#/components/schemas/*`, returning the inlined schema.
fn hoist(schema: &JsonObject, components: &mut Map<String, Value>) -> Value {
    let mut value = Value::Object(schema.clone());
    if let Value::Object(ref mut m) = value {
        if let Some(Value::Object(defs)) = m.remove("$defs") {
            for (name, mut def) in defs {
                rewrite_refs(&mut def);
                components.entry(name).or_insert(def);
            }
        }
    }
    rewrite_refs(&mut value);
    value
}

fn rewrite_refs(v: &mut Value) {
    match v {
        Value::Object(m) => {
            if let Some(Value::String(r)) = m.get_mut("$ref") {
                if let Some(rest) = r.strip_prefix("#/$defs/") {
                    *r = format!("#/components/schemas/{rest}");
                }
            }
            for (_, val) in m.iter_mut() {
                rewrite_refs(val);
            }
        }
        Value::Array(a) => {
            for x in a {
                rewrite_refs(x);
            }
        }
        _ => {}
    }
}

