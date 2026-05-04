use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use crate::backend::Backend;
use crate::types::*;

#[derive(Clone)]
pub struct MemvaultServer {
    client: Arc<dyn Backend>,
    default_tags: Vec<String>,
    default_visibility: String,
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
}

impl MemvaultServer {
    pub fn new(client: Arc<dyn Backend>, default_tags: Vec<String>, default_visibility: String) -> Self {
        Self {
            client,
            default_tags,
            default_visibility,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler]
impl ServerHandler for MemvaultServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Memvault MCP server — store, retrieve, search, and link memories in a \
                 local-first p2p knowledge base. Use memvault_put to store documents, \
                 memvault_search to find them, and the graph tools to build a knowledge graph."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

fn parse_tags(tags: &[String]) -> Vec<(String, String)> {
    tags.iter()
        .filter_map(|t| {
            let mut parts = t.splitn(2, ':');
            let scope = parts.next()?.to_string();
            let label = parts.next().unwrap_or("").to_string();
            Some((scope, label))
        })
        .collect()
}

#[tool_router]
impl MemvaultServer {
    #[tool(
        name = "memvault_put",
        description = "Store a memory (document with optional title and tags). Returns the hex-encoded CID and doc ID."
    )]
    async fn put(&self, Parameters(params): Parameters<PutParams>) -> String {
        let mut frontmatter = serde_json::Map::new();
        if let Some(title) = &params.title {
            frontmatter.insert(
                "title".to_string(),
                serde_json::Value::String(title.clone()),
            );
        }

        let tags_input = if params.tags.is_empty() {
            &self.default_tags
        } else {
            &params.tags
        };
        let tags = parse_tags(tags_input);
        let vis = params.visibility.as_deref().or(Some(self.default_visibility.as_str()));

        match self
            .client
            .put_doc(
                &params.text,
                serde_json::Value::Object(frontmatter),
                tags,
                vis,
            )
            .await
        {
            Ok(resp) => {
                serde_json::json!({
                    "cid": resp.get("cid").and_then(|v| v.as_str()).unwrap_or(""),
                    "id": resp.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                    "status": "stored"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_get",
        description = "Retrieve a memory by its hex-encoded doc ID. Returns the document text and metadata."
    )]
    async fn get(&self, Parameters(params): Parameters<GetParams>) -> String {
        match self.client.get_doc(&params.cid).await {
            Ok(Some(doc)) => {
                serde_json::json!({
                    "doc_id": params.cid,
                    "title": doc.get("frontmatter")
                        .and_then(|f| f.get("title"))
                        .and_then(|t| t.as_str()),
                    "body": doc.get("body").and_then(|b| b.as_str()).unwrap_or(""),
                })
                .to_string()
            }
            Ok(None) => format!("error: document not found for id {}", params.cid),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_search",
        description = "Search memories by text query. Returns matching documents with relevance scores and snippets. Optionally filter by tag (scope:label format)."
    )]
    async fn search(&self, Parameters(params): Parameters<SearchParams>) -> String {
        let limit = params.limit.unwrap_or(10);
        let tag_filter = params.tag_filter.as_deref();

        match self.client.search(&params.query, limit, tag_filter).await {
            Ok(hits) => hits.to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_list",
        description = "List recent memories. Optionally filter by tag scope and label."
    )]
    async fn list(&self, Parameters(params): Parameters<ListParams>) -> String {
        let limit = params.limit.unwrap_or(20);
        let tag_ns = params.tag_scope.as_deref();
        let tag_val = params.tag_label.as_deref();

        match self.client.list_docs(tag_ns, tag_val, limit).await {
            Ok(docs) => {
                let results = docs.as_array().cloned().unwrap_or_default();
                serde_json::json!({
                    "count": results.len(),
                    "documents": results,
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_attach",
        description = "Attach a local file to memvault by its absolute path. Returns the manifest CID."
    )]
    async fn attach(&self, Parameters(params): Parameters<AttachParams>) -> String {
        let path = std::path::Path::new(&params.path);

        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => return format!("error: cannot read {}: {e}", params.path),
        };

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed");

        let mime_type = params.content_type.as_deref().unwrap_or_else(|| {
            mime_guess::from_path(path)
                .first_raw()
                .unwrap_or("application/octet-stream")
        });

        match self
            .client
            .attach_file(&data, filename, mime_type)
            .await
        {
            Ok(resp) => {
                serde_json::json!({
                    "manifest_cid": resp.get("cid").and_then(|v| v.as_str()).unwrap_or(""),
                    "filename": filename,
                    "size": data.len(),
                    "mime_type": mime_type,
                    "status": "attached"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_read_range",
        description = "Read a byte range from an attachment. Returns base64-encoded bytes for the range [start, end)."
    )]
    async fn read_range(&self, Parameters(params): Parameters<ReadRangeParams>) -> String {
        match self.client.read_attachment_range(&params.manifest_cid, params.start, params.end).await {
            Ok(data) => {
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                serde_json::json!({
                    "data_base64": encoded,
                    "size": data.len(),
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_pin",
        description = "Pin an attachment to prevent garbage collection."
    )]
    async fn pin(&self, Parameters(params): Parameters<PinParams>) -> String {
        match self.client.pin_attachment(&params.manifest_cid).await {
            Ok(()) => serde_json::json!({
                "manifest_cid": params.manifest_cid,
                "status": "pinned"
            })
            .to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_unpin",
        description = "Unpin an attachment, allowing garbage collection."
    )]
    async fn unpin(&self, Parameters(params): Parameters<UnpinParams>) -> String {
        match self.client.unpin_attachment(&params.manifest_cid).await {
            Ok(()) => serde_json::json!({
                "manifest_cid": params.manifest_cid,
                "status": "unpinned"
            })
            .to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_extract_text",
        description = "Extract text content from an attachment (supports PDF, DOCX, HTML, Markdown, plain text)."
    )]
    async fn extract_text(&self, Parameters(params): Parameters<ExtractTextParams>) -> String {
        match self.client.extract_text(&params.manifest_cid).await {
            Ok(Some(text)) => serde_json::json!({
                "manifest_cid": params.manifest_cid,
                "text": text,
            })
            .to_string(),
            Ok(None) => serde_json::json!({
                "manifest_cid": params.manifest_cid,
                "text": null,
                "note": "unsupported format or extraction failed"
            })
            .to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_attachment_info",
        description = "Get the manifest metadata for an attachment (filename, MIME type, size, layout, etc.)."
    )]
    async fn attachment_info(&self, Parameters(params): Parameters<AttachmentInfoParams>) -> String {
        match self
            .client
            .get_attachment_manifest(&params.manifest_cid)
            .await
        {
            Ok(Some(manifest)) => manifest.to_string(),
            Ok(None) => format!("error: manifest not found for cid {}", params.manifest_cid),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_graph_add",
        description = "Add an entity to the knowledge graph. Returns the hex-encoded entity ID. Property values are stored as-is (strings)."
    )]
    async fn graph_add(&self, Parameters(params): Parameters<GraphAddParams>) -> String {
        let vis = params.visibility.as_deref().or(Some(self.default_visibility.as_str()));
        // Convert HashMap<String,String> to JSON object preserving string values.
        let props = serde_json::json!(params.props);

        match self
            .client
            .add_entity(&params.kind, props, vis)
            .await
        {
            Ok(resp) => {
                serde_json::json!({
                    "entity_id": resp.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                    "status": "created"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_graph_link",
        description = "Add a directed edge between two entities in the knowledge graph. \
                       Source and target are hex-encoded entity IDs. Use memvault_link for cross-type linking."
    )]
    async fn graph_link(&self, Parameters(params): Parameters<GraphLinkParams>) -> String {
        let source = format!("entity:{}", params.source_id);
        let target = format!("entity:{}", params.target_id);
        match self
            .client
            .add_link(&source, &target, &params.relation, params.weight)
            .await
        {
            Ok(resp) => {
                serde_json::json!({
                    "edge_id": resp.get("edge_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "status": "linked"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_graph_query",
        description = "List all edges for a given entity. Returns connected nodes."
    )]
    async fn graph_query(&self, Parameters(params): Parameters<GraphQueryParams>) -> String {
        let node = format!("entity:{}", params.from_id);
        match self.client.edges_of(&node).await {
            Ok(edges) => {
                let results = edges.as_array().cloned().unwrap_or_default();
                serde_json::json!({
                    "count": results.len(),
                    "edges": results,
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_link",
        description = "Create a directed link between any two nodes (entities, documents, or attachments). \
                       Nodes are specified as 'entity:<hex>', 'doc:<hex>', or 'attachment:<hex>'. Returns the edge ID."
    )]
    async fn link(&self, Parameters(params): Parameters<LinkParams>) -> String {
        match self.client.add_link(&params.source, &params.target, &params.relation, params.weight).await {
            Ok(resp) => {
                serde_json::json!({
                    "edge_id": resp.get("edge_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "status": "linked"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_edges",
        description = "List all edges (incoming and outgoing) for any node. \
                       Node is specified as 'entity:<hex>', 'doc:<hex>', or 'attachment:<hex>'."
    )]
    async fn edges(&self, Parameters(params): Parameters<EdgesOfParams>) -> String {
        match self.client.edges_of(&params.node).await {
            Ok(edges) => {
                let results = edges.as_array().cloned().unwrap_or_default();
                serde_json::json!({
                    "count": results.len(),
                    "edges": results,
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_unlink",
        description = "Remove a link (edge) by its hex-encoded edge ID. Requires the source node."
    )]
    async fn unlink(&self, Parameters(params): Parameters<UnlinkParams>) -> String {
        match self.client.delete_link(&params.edge_id, &params.source).await {
            Ok(_) => serde_json::json!({ "edge_id": params.edge_id, "status": "removed" }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_list_all",
        description = "List all nodes (documents, entities, and attachments). Optionally filter by a saved view name."
    )]
    async fn list_all(&self, Parameters(params): Parameters<ListAllParams>) -> String {
        let limit = params.limit.unwrap_or(100);
        match self.client.list_all(params.view.as_deref(), limit).await {
            Ok(nodes) => {
                let results = nodes.as_array().cloned().unwrap_or_default();
                serde_json::json!({
                    "count": results.len(),
                    "nodes": results,
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_tag",
        description = "Add tags to an existing item (entity, document, or attachment). Tags in 'scope:label' format."
    )]
    async fn tag(&self, Parameters(params): Parameters<TagParams>) -> String {
        let tags = parse_tags(&params.tags);
        match self.client.add_tags(&params.node, tags).await {
            Ok(resp) => resp.to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_untag",
        description = "Remove tags from an existing item. Tags in 'scope:label' format."
    )]
    async fn untag(&self, Parameters(params): Parameters<UntagParams>) -> String {
        let tags = parse_tags(&params.tags);
        match self.client.remove_tags(&params.node, tags).await {
            Ok(resp) => resp.to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_get_tags",
        description = "Get the effective tags for an item."
    )]
    async fn get_tags(&self, Parameters(params): Parameters<GetTagsParams>) -> String {
        match self.client.get_tags(&params.node).await {
            Ok(resp) => resp.to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_view_list",
        description = "List all saved views. Views are named tag filter sets that scope content."
    )]
    async fn view_list(&self) -> String {
        match self.client.list_views().await {
            Ok(views) => views.to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_view_create",
        description = "Create a saved view. Items must have ALL the specified tags to appear. Tags in 'scope:label' format."
    )]
    async fn view_create(&self, Parameters(params): Parameters<ViewCreateParams>) -> String {
        let tags = parse_tags(&params.tags);
        match self.client.create_view(&params.name, tags).await {
            Ok(resp) => resp.to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_view_update",
        description = "Update a view's required tags."
    )]
    async fn view_update(&self, Parameters(params): Parameters<ViewUpdateParams>) -> String {
        let tags = parse_tags(&params.tags);
        match self.client.update_view(&params.name, tags).await {
            Ok(resp) => resp.to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_view_delete",
        description = "Delete a saved view by name."
    )]
    async fn view_delete(&self, Parameters(params): Parameters<ViewDeleteParams>) -> String {
        match self.client.delete_view(&params.name).await {
            Ok(resp) => resp.to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_get_entity",
        description = "Get a single entity by its hex-encoded ID. Returns kind, properties, and outgoing edges."
    )]
    async fn get_entity(&self, Parameters(params): Parameters<GetEntityParams>) -> String {
        match self.client.get_entity(&params.id).await {
            Ok(Some(entity)) => entity.to_string(),
            Ok(None) => format!("error: entity not found: {}", params.id),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_list_entities",
        description = "List knowledge graph entities with their kind and label."
    )]
    async fn list_entities(&self, Parameters(params): Parameters<ListEntitiesParams>) -> String {
        let limit = params.limit.unwrap_or(50);
        match self.client.list_entities(limit).await {
            Ok(entities) => {
                let arr = entities.as_array().cloned().unwrap_or_default();
                serde_json::json!({ "count": arr.len(), "entities": arr }).to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_traverse",
        description = "Traverse the knowledge graph from any node. Returns connected nodes up to max_depth. \
                       Start node specified as 'entity:<hex>', 'doc:<hex>', or 'attachment:<hex>'."
    )]
    async fn traverse(&self, Parameters(params): Parameters<TraverseParams>) -> String {
        let max_depth = params.max_depth.unwrap_or(2);
        match self.client.traverse_from(&params.from, params.relation.as_deref(), max_depth).await {
            Ok(hits) => {
                let arr = hits.as_array().cloned().unwrap_or_default();
                serde_json::json!({ "count": arr.len(), "hits": arr }).to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_audit",
        description = "Query the audit log. Optionally filter by operation kind (DocCreate, EntityCreate, AttachFile, EdgeAdd, Retract)."
    )]
    async fn audit(&self, Parameters(params): Parameters<AuditParams>) -> String {
        let limit = params.limit.unwrap_or(50);
        match self.client.audit(limit, params.op_kind.as_deref()).await {
            Ok(records) => {
                let arr = records.as_array().cloned().unwrap_or_default();
                serde_json::json!({ "count": arr.len(), "records": arr }).to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_doc_history",
        description = "View the operation history for a document by its hex-encoded ID."
    )]
    async fn doc_history(&self, Parameters(params): Parameters<DocHistoryParams>) -> String {
        match self.client.history_of(&params.doc_id).await {
            Ok(history) => {
                let arr = history.as_array().cloned().unwrap_or_default();
                serde_json::json!({ "count": arr.len(), "history": arr }).to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_retract",
        description = "Retract (soft-delete) any node by its ID ('entity:<hex>', 'doc:<hex>', 'attachment:<hex>'). \
                       Removes it from search, graph, and all views."
    )]
    async fn retract(&self, Parameters(params): Parameters<RetractParams>) -> String {
        // Support both node_id format and raw CID for backward compat.
        let node_id = &params.cid;
        if node_id.contains(':') {
            // Node ID format
            match self.client.retract_node(node_id, &params.reason).await {
                Ok(resp) => resp.to_string(),
                Err(e) => format!("error: {e}"),
            }
        } else {
            // Legacy CID format
            match self.client.retract(node_id, &params.reason).await {
                Ok(resp) => {
                    serde_json::json!({
                        "tombstone_cid": resp.get("cid").and_then(|v| v.as_str()).unwrap_or(""),
                        "status": "retracted"
                    })
                    .to_string()
                }
                Err(e) => format!("error: {e}"),
            }
        }
    }

    #[tool(
        name = "memvault_status",
        description = "Get the status of the local memvault node (block count, doc count, peer count, uptime)."
    )]
    async fn status(&self) -> String {
        match self.client.status().await {
            Ok(s) => s.to_string(),
            Err(e) => format!("error: {e}"),
        }
    }
}
